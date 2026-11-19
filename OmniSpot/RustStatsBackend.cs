using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using OmniSpot.Stats.Storage;

namespace OmniSpot.Stats;

/// <summary>FFI adapter for Rust-owned stats event calculations and session buckets.</summary>
internal sealed class RustStatsBackend
{
    private const string LibraryName = "omnispot_spotlight_native";
    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web);
    private static NativeStatsChangedCallback? _changedCallback;
    private static NativeStatsChangedCallback? _flushCallback;

    public void SetChangedCallback(Action callback)
    {
        _changedCallback = () =>
        {
            try { callback(); }
            catch { /* Managed UI subscribers must not unwind through the native worker thread. */ }
        };
        Ensure(NativeSetChangedCallback(_changedCallback));
    }

    public void SetFlushCallback(Action callback)
    {
        _flushCallback = () =>
        {
            try { callback(); }
            catch { /* Persistence failures are logged by StatsService; never unwind through Rust. */ }
        };
        Ensure(NativeSetFlushCallback(_flushCallback));
    }

    public void ProcessSwitch(SwitchEvent e, string triggerKey)
    {
        var request = new
        {
            processName = e.ProcessName,
            totalChoices = e.TotalChoices,
            modifierDownTick = e.ModifierDownTick,
            letterDownTick = e.LetterDownTick,
            previousLetterUpTick = e.PreviousLetterUpTick,
            isDynamic = e.IsDynamic,
            triggerKey
        };
        var bytes = Serialize(request);
        Ensure(NativeProcessSwitch(bytes, (nuint)bytes.Length));
    }

    public void ProcessPeek(PeekEvent e)
    {
        var bytes = Serialize(new
        {
            processName = e.TargetProcessName,
            armTick = e.ArmTick,
            finishTick = e.FinishTick,
            isDynamic = e.IsDynamic
        });
        Ensure(NativeProcessPeek(bytes, (nuint)bytes.Length));
    }

    public void ProcessAltTab(int navCount) => Ensure(NativeProcessAltTab(navCount));

    public bool IsRunning => NativeServiceIsRunning() != 0;

    public bool ApplyConfiguration(bool statsEnabled)
    {
        var result = NativeServiceUpdate(statsEnabled ? 1u : 0u);
        if (result == 0) throw new InvalidOperationException("Rust stats configuration update failed.");
        return result == 2;
    }

    public bool Enqueue(StatsEvent statsEvent)
    {
        object request = statsEvent switch
        {
            SwitchEvent e => new
            {
                kind = "switch",
                processName = e.ProcessName,
                processPath = e.ProcessPath,
                totalChoices = e.TotalChoices,
                modifierDownTick = e.ModifierDownTick,
                letterDownTick = e.LetterDownTick,
                previousLetterUpTick = e.PreviousLetterUpTick,
                isDynamic = e.IsDynamic,
                triggerKey = e.TriggerKey.ToString()
            },
            PeekEvent e => new
            {
                kind = "peek",
                processName = e.TargetProcessName,
                processPath = e.TargetProcessPath,
                armTick = e.ArmTick,
                finishTick = e.FinishTick,
                isDynamic = e.IsDynamic
            },
            AltTabEvent e => new { kind = "altTab", navCount = e.NavCount },
            _ => throw new ArgumentOutOfRangeException(nameof(statsEvent))
        };
        var bytes = JsonSerializer.SerializeToUtf8Bytes(request, request.GetType(), JsonOptions);
        var result = NativeEnqueue(bytes, (nuint)bytes.Length);
        if (result == 2) throw new InvalidOperationException("Rust stats queue rejected an invalid event payload.");
        return result == 1;
    }

    public IReadOnlyList<DailyBucketDocument> GetSnapshots()
    {
        var pointer = NativeGetSnapshots(out var length);
        if (pointer == IntPtr.Zero) throw new InvalidOperationException("Rust stats snapshot query failed.");
        try
        {
            var json = Marshal.PtrToStringUTF8(pointer, checked((int)length))!;
            return JsonSerializer.Deserialize<List<DailyBucketDocument>>(json, JsonOptions)
                   ?? throw new InvalidOperationException("Rust returned no stats snapshots.");
        }
        finally { NativeFree(pointer, length); }
    }

    public void LoadToday(DailyBucketDocument document)
    {
        var bytes = Serialize(document);
        Ensure(NativeLoadToday(bytes, (nuint)bytes.Length));
    }
    public void ClearRollover() => Ensure(NativeClearRollover());
    public void ClearSession() => Ensure(NativeClearSession());

    public IReadOnlyList<DailyBucketDocument> GetHistoricBuckets(IEnumerable<DailyBucketDocument> buckets, DateOnly today)
    {
        var bytes = Serialize(new { buckets, today = today.ToString("yyyy-MM-dd") });
        var pointer = NativeHistoricBuckets(bytes, (nuint)bytes.Length, out var length);
        if (pointer == IntPtr.Zero) throw new InvalidOperationException("Rust historic stats query failed.");
        try
        {
            var json = Marshal.PtrToStringUTF8(pointer, checked((int)length))!;
            return JsonSerializer.Deserialize<List<DailyBucketDocument>>(json, JsonOptions) ?? [];
        }
        finally { NativeFree(pointer, length); }
    }

    public string GetLocalDateKey(DateTime utcDateTime, TimeZoneInfo zone)
    {
        var utc = DateTime.SpecifyKind(utcDateTime, DateTimeKind.Utc);
        var request = Serialize(new { utcTicks = utc.Ticks, offsetTicks = zone.GetUtcOffset(utc).Ticks });
        var pointer = NativeLocalDateKey(request, (nuint)request.Length, out var length);
        if (pointer == IntPtr.Zero) throw new InvalidOperationException("Rust local stats date conversion failed.");
        try
        {
            return JsonSerializer.Deserialize<string>(Marshal.PtrToStringUTF8(pointer, checked((int)length))!, JsonOptions)
                   ?? throw new InvalidOperationException("Rust returned no local date key.");
        }
        finally { NativeFree(pointer, length); }
    }

    public StatsMetrics Calculate(
        IReadOnlyList<DailyBucketDocument> allBuckets,
        DailyBucketDocument today,
        List<(string ProcessName, System.Windows.Input.Key Key)> configuredStaticApps,
        AppRegistryCache registryCache)
    {
        var names = allBuckets.Append(today)
            .SelectMany(bucket => bucket.StaticAppUsage.Keys.Concat(bucket.DynamicAppUsage.Keys))
            .Concat(configuredStaticApps.Select(app => app.ProcessName))
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .ToDictionary(name => name, name => registryCache.GetDisplayName(name), StringComparer.OrdinalIgnoreCase);
        var request = new
        {
            allBuckets,
            today,
            configuredStaticApps = configuredStaticApps.Select(app => new
            {
                processName = app.ProcessName,
                key = app.Key.ToString()
            }),
            displayNames = names
        };
        var bytes = Serialize(request);
        var pointer = NativeCalculate(bytes, (nuint)bytes.Length, out var length);
        if (pointer == IntPtr.Zero) throw new InvalidOperationException("Rust stats calculation failed.");
        try
        {
            var json = Marshal.PtrToStringUTF8(pointer, checked((int)length))!;
            return JsonSerializer.Deserialize<StatsMetrics>(json, JsonOptions)
                   ?? throw new InvalidOperationException("Rust returned no stats metrics.");
        }
        finally { NativeFree(pointer, length); }
    }

    private static byte[] Serialize<T>(T value) => JsonSerializer.SerializeToUtf8Bytes(value, JsonOptions);

    private static void Ensure(uint result)
    {
        if (result == 0) throw new InvalidOperationException("Rust stats operation failed.");
    }

    [DllImport(LibraryName, EntryPoint = "omnispot_stats_process_switch", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeProcessSwitch(byte[] request, nuint length);
    [DllImport(LibraryName, EntryPoint = "omnispot_stats_process_peek", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeProcessPeek(byte[] request, nuint length);
    [DllImport(LibraryName, EntryPoint = "omnispot_stats_process_alt_tab", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeProcessAltTab(int navCount);
    [DllImport(LibraryName, EntryPoint = "omnispot_stats_get_snapshots", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr NativeGetSnapshots(out nuint length);
    [DllImport(LibraryName, EntryPoint = "omnispot_stats_load_today", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeLoadToday(byte[] request, nuint length);
    [DllImport(LibraryName, EntryPoint = "omnispot_stats_clear_rollover", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeClearRollover();
    [DllImport(LibraryName, EntryPoint = "omnispot_stats_clear_session", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeClearSession();
    [DllImport(LibraryName, EntryPoint = "omnispot_stats_calculate", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr NativeCalculate(byte[] request, nuint requestLength, out nuint length);
    [DllImport(LibraryName, EntryPoint = "omnispot_stats_historic_buckets", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr NativeHistoricBuckets(byte[] request, nuint requestLength, out nuint length);
    [DllImport(LibraryName, EntryPoint = "omnispot_stats_local_date_key", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr NativeLocalDateKey(byte[] request, nuint requestLength, out nuint length);
    [DllImport(LibraryName, EntryPoint = "omnispot_stats_service_is_running", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeServiceIsRunning();
    [DllImport(LibraryName, EntryPoint = "omnispot_stats_service_update", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeServiceUpdate(uint enabled);
    [DllImport(LibraryName, EntryPoint = "omnispot_stats_enqueue", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeEnqueue(byte[] request, nuint requestLength);
    [DllImport(LibraryName, EntryPoint = "omnispot_stats_set_changed_callback", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeSetChangedCallback(NativeStatsChangedCallback? callback);
    [DllImport(LibraryName, EntryPoint = "omnispot_stats_set_flush_callback", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeSetFlushCallback(NativeStatsChangedCallback? callback);

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate void NativeStatsChangedCallback();
    [DllImport(LibraryName, EntryPoint = "omnispot_spotlight_free", CallingConvention = CallingConvention.Cdecl)]
    private static extern void NativeFree(IntPtr pointer, nuint length);
}
