using System.Runtime.InteropServices;
using System.Text.Json;
using OmniSpot.Stats.Storage;

namespace OmniSpot.Stats;

/// <summary>FFI adapter for Rust-owned app-name caching and fallback resolution.</summary>
internal sealed class RustAppRegistryBackend
{
    private const string LibraryName = "omnispot_spotlight_native";
    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web);

    public void Initialize(IEnumerable<AppRegistryDocument> entries)
    {
        var response = Invoke<bool>(new { operation = "initialize", entries = entries.Select(e => new
        {
            processName = e.ProcessName,
            displayName = e.DisplayName
        }).ToArray() });
        if (response != true) throw new InvalidOperationException("Rust app registry initialization failed.");
    }

    public AppRegistryResolution Resolve(string processName, string? processPath) =>
        Invoke<AppRegistryResolution>(new { operation = "resolve", processName, processPath })
        ?? throw new InvalidOperationException("Rust app registry returned no result.");

    public IReadOnlyList<AppRegistryDocument> GetEntries() =>
        (Invoke<RegistryEntry[]>(new { operation = "entries" }) ?? [])
        .Select(entry => new AppRegistryDocument { ProcessName = entry.ProcessName, DisplayName = entry.DisplayName })
        .ToArray();

    private static T? Invoke<T>(object request)
    {
        var bytes = JsonSerializer.SerializeToUtf8Bytes(request, JsonOptions);
        var pointer = NativeInvoke(bytes, (nuint)bytes.Length, out var length);
        if (pointer == IntPtr.Zero) throw new InvalidOperationException("Rust app registry operation failed.");
        try
        {
            var json = Marshal.PtrToStringUTF8(pointer, checked((int)length))!;
            return JsonSerializer.Deserialize<T>(json, JsonOptions);
        }
        finally { NativeFree(pointer, length); }
    }

    [DllImport(LibraryName, EntryPoint = "omnispot_app_registry", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr NativeInvoke(byte[] request, nuint requestLength, out nuint outputLength);
    [DllImport(LibraryName, EntryPoint = "omnispot_spotlight_free", CallingConvention = CallingConvention.Cdecl)]
    private static extern void NativeFree(IntPtr pointer, nuint length);

    private sealed record RegistryEntry(string ProcessName, string DisplayName);
}

internal sealed record AppRegistryResolution(string DisplayName, bool WasCached, bool ShouldPersist);
