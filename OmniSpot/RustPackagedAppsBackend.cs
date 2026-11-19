using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using OmniSpot.WindowDiscovery;

namespace OmniSpot;

/// <summary>Thin FFI adapter for native package discovery and identity lookups.</summary>
internal sealed class RustPackagedAppsBackend
{
    private const string LibraryName = "omnispot_spotlight_native";
    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web);

    public InstalledPackages GetInstalledPackages() => new(Invoke<string[]>("installedPackages") ?? []);

    public IReadOnlyList<(string Name, string Path, string? Aumid, string? IconPath)> GetSearchableApps() =>
        (Invoke<SearchableApp[]>("searchableApps") ?? [])
        .Select(app => (app.Name, app.Path, (string?)app.Aumid, (string?)app.IconPath)).ToArray();

    public PackagedAppInfo? GetByInstalledPath(string path, uint? processId) =>
        Invoke<PackagedAppInfo>("byInstalledPath", path: path, processId: processId);

    public PackagedAppInfo? GetByAumid(string? aumid) => Invoke<PackagedAppInfo>("byAumid", aumid: aumid);

    public PackagedAppInfo? GetByPackageFamilyName(string familyName) =>
        Invoke<PackagedAppInfo>("byFamilyName", familyName: familyName);

    public string? GetInstalledPathByAumid(string? aumid) =>
        Invoke<string>("installedPathByAumid", aumid: aumid);

    public string? GetDisplayName(string path) => Invoke<string>("displayName", path: path);

    private static T? Invoke<T>(string operation, string? path = null, uint? processId = null,
        string? aumid = null, string? familyName = null)
    {
        var request = JsonSerializer.SerializeToUtf8Bytes(new { operation, path, processId, aumid, familyName }, JsonOptions);
        var pointer = NativeInvoke(request, (nuint)request.Length, out var length);
        if (pointer == IntPtr.Zero)
            throw new InvalidOperationException($"Rust packaged-app operation '{operation}' failed.");
        try
        {
            var json = Marshal.PtrToStringUTF8(pointer, checked((int)length))!;
            return JsonSerializer.Deserialize<T>(json, JsonOptions);
        }
        finally { NativeFree(pointer, length); }
    }

    [DllImport(LibraryName, EntryPoint = "omnispot_packaged_apps", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr NativeInvoke(byte[] request, nuint requestLength, out nuint outputLength);
    [DllImport(LibraryName, EntryPoint = "omnispot_spotlight_free", CallingConvention = CallingConvention.Cdecl)]
    private static extern void NativeFree(IntPtr pointer, nuint length);

    private sealed record SearchableApp(string Name, string Path, string Aumid, string IconPath);
}
