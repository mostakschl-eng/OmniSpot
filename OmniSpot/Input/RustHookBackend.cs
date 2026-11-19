using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using OmniSpot.Configuration;
using AppConfig = OmniSpot.Configuration.Configuration;

namespace OmniSpot.Input;

/// <summary>FFI boundary for the native keyboard hook engine. WPF effects are dispatched back to the host.</summary>
internal sealed class RustHookBackend
{
    private const string LibraryName = "omnispot_spotlight_native";
    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web);
    private NativeHookCallback? _callback;

    public void RegisterHost(Func<uint, byte[], uint> dispatch)
    {
        _callback = (eventId, payload, length) =>
        {
            try
            {
                var bytes = new byte[checked((int)length.ToUInt64())];
                if (bytes.Length > 0) Marshal.Copy(payload, bytes, 0, bytes.Length);
                return dispatch(eventId, bytes);
            }
            catch { return 0; }
        };
        Ensure(NativeSetCallback(_callback));
    }

    public void Start(AppConfig config)
    {
        var bytes = Serialize(config);
        Ensure(NativeStart(bytes, (nuint)bytes.Length));
    }
    public void UpdateConfiguration(AppConfig config)
    {
        var bytes = Serialize(config);
        Ensure(NativeUpdateConfiguration(bytes, (nuint)bytes.Length));
    }
    public void Stop() => Ensure(NativeStop());
    public void SetOverlayPreviewEnabled(bool enabled) => Ensure(NativeSetOverlayPreview(enabled ? 1u : 0u));
    public void ShowOverlayIfEnabled() => Ensure(NativeShowOverlayIfEnabled());
    public void PotentialElevationDetected() => Ensure(NativePotentialElevationDetected());

    public bool Process(uint key, bool isDown, uint flags, string culture)
    {
        var request = JsonSerializer.SerializeToUtf8Bytes(new HookEventRequest(key, isDown, flags, culture), JsonOptions);
        Ensure(NativeKeyboardEvent(request, (nuint)request.Length, out var suppress));
        return suppress != 0;
    }

    private static byte[] Serialize(AppConfig config) => JsonSerializer.SerializeToUtf8Bytes(new NativeConfiguration(
        (uint)config.Modifier,
        config.Applications.Select(app => new NativeApplication((uint)app.Key, app.ProcessPath, app.CycleMode.ToString(),
            app.StartIfNotRunning, app.Type.ToString(), app.Aumid)).ToArray(),
        config.PulseBorderEnabled, config.Theme.ToString(), config.OverlayEnabled, config.OverlayShowDelayMs,
        config.OverlayKeepOpenWhileModifierHeld, config.PeekEnabled, config.DynamicModeEnabled, config.StatsEnabled,
        config.SpotlightEnabled, (uint)config.SpotlightModifier, config.SpotlightCustomDirectories?.ToArray()), JsonOptions);

    private static void Ensure(uint result)
    {
        if (result == 0) throw new InvalidOperationException("Rust keyboard hook operation failed.");
    }

    [DllImport(LibraryName, EntryPoint = "omnispot_hook_set_callback", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeSetCallback(NativeHookCallback? callback);
    [DllImport(LibraryName, EntryPoint = "omnispot_hook_start", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeStart(byte[] request, nuint length);
    [DllImport(LibraryName, EntryPoint = "omnispot_hook_update_configuration", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeUpdateConfiguration(byte[] request, nuint length);
    [DllImport(LibraryName, EntryPoint = "omnispot_hook_stop", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeStop();
    [DllImport(LibraryName, EntryPoint = "omnispot_hook_keyboard_event", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeKeyboardEvent(byte[] request, nuint length, out uint suppress);
    [DllImport(LibraryName, EntryPoint = "omnispot_hook_set_overlay_preview", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeSetOverlayPreview(uint enabled);
    [DllImport(LibraryName, EntryPoint = "omnispot_hook_show_overlay_if_enabled", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeShowOverlayIfEnabled();
    [DllImport(LibraryName, EntryPoint = "omnispot_hook_potential_elevation_detected", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativePotentialElevationDetected();

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate uint NativeHookCallback(uint eventId, IntPtr payload, UIntPtr length);

    private sealed record HookEventRequest(uint Key, bool IsKeyDown, uint Flags, string Culture);
    private sealed record NativeApplication(uint Key, string ProcessPath, string CycleMode, bool StartIfNotRunning,
        string ApplicationType, string? Aumid);
    private sealed record NativeConfiguration(uint Modifier, NativeApplication[] Applications, bool PulseBorderEnabled,
        string Theme, bool OverlayEnabled, int OverlayShowDelayMs, bool OverlayKeepOpenWhileModifierHeld,
        bool PeekEnabled, bool DynamicModeEnabled, bool StatsEnabled, bool SpotlightEnabled,
        uint SpotlightModifier, string[]? SpotlightCustomDirectories);
}
