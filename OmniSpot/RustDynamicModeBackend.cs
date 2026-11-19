using OmniSpot.Configuration;
using OmniSpot.WindowDiscovery;
using System.Runtime.InteropServices;
using System.IO;
using System.Text;
using System.Text.Json;

namespace OmniSpot;

/// <summary>Runs dynamic app filtering and key assignment in the native Rust backend.</summary>
internal static class RustDynamicModeBackend
{
    private const string LibraryName = "omnispot_spotlight_native";
    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web);

    public static IReadOnlyList<ApplicationConfiguration> Build(
        IReadOnlyList<ApplicationConfiguration> staticApps,
        IReadOnlyList<ApplicationWindow> windows,
        IAppNameResolver resolver,
        IReadOnlyList<string> installedPackages,
        uint? key)
    {
        var paths = windows.Select(w => w.ProcessImagePath).Distinct(StringComparer.OrdinalIgnoreCase).ToArray();
        var resolved = paths.Select(path => new ResolvedKey(path,
            resolver.GetDynamicKey(Path.GetFileName(path), path) is { } assigned ? (uint)assigned : null)).ToArray();
        var request = new Request(windows.Select(ToNative).ToArray(), staticApps.Select(ToNative).ToArray(), resolved,
            installedPackages.ToArray(), key);
        var bytes = Encoding.UTF8.GetBytes(JsonSerializer.Serialize(request, JsonOptions));
        UIntPtr length = UIntPtr.Zero;
        var pointer = NativeBuild(bytes, (UIntPtr)bytes.Length, ref length);
        if (pointer == IntPtr.Zero) throw new InvalidOperationException("Rust dynamic mode returned no response.");
        try
        {
            var resultBytes = new byte[checked((int)length.ToUInt64())];
            Marshal.Copy(pointer, resultBytes, 0, resultBytes.Length);
            var result = JsonSerializer.Deserialize<NativeApplication[]>(resultBytes, JsonOptions) ?? [];
            return result.Select(app => new ApplicationConfiguration((System.Windows.Input.Key)app.Key,
                app.ProcessPath, Enum.Parse<CycleMode>(app.CycleMode), app.StartIfNotRunning,
                Enum.Parse<ApplicationType>(app.ApplicationType), app.Aumid)).ToArray();
        }
        finally { NativeFree(pointer, length); }
    }

    public static uint? ResolveDynamicKey(string displayName)
    {
        var bytes = Encoding.UTF8.GetBytes(JsonSerializer.Serialize(displayName));
        var key = NativeResolveKey(bytes, (UIntPtr)bytes.Length);
        return key == 0 ? null : key;
    }

    private static NativeWindow ToNative(ApplicationWindow w) => new(
        RustSwitcherBackend.ToInt64(w.Handle), w.Title, w.ProcessId, w.ProcessImagePath, (uint)w.State,
        (int)w.Position.X, (int)w.Position.Y, (int)w.Size.Width, (int)w.Size.Height,
        (uint)w.Style, (uint)w.StyleEx, w.IsCloaked, w.NeedsElevation);

    private static NativeApplication ToNative(ApplicationConfiguration app) => new(
        (uint)app.Key, app.ProcessPath, app.CycleMode.ToString(), app.StartIfNotRunning,
        app.Type.ToString(), app.Aumid);

    [DllImport(LibraryName, EntryPoint = "omnispot_dynamic_get_apps", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr NativeBuild(byte[] request, UIntPtr requestLength, ref UIntPtr outputLength);
    [DllImport(LibraryName, EntryPoint = "omnispot_dynamic_resolve_key", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeResolveKey(byte[] request, UIntPtr requestLength);
    [DllImport(LibraryName, EntryPoint = "omnispot_spotlight_free", CallingConvention = CallingConvention.Cdecl)]
    private static extern void NativeFree(IntPtr pointer, UIntPtr length);

    private sealed record Request(NativeWindow[] Windows, NativeApplication[] Applications,
        ResolvedKey[] ResolvedKeys, string[] InstalledPackages, uint? Key);
    private sealed record ResolvedKey(string ProcessPath, uint? Key);
    private sealed record NativeApplication(uint Key, string ProcessPath, string CycleMode,
        bool StartIfNotRunning, string ApplicationType, string? Aumid);
    private sealed record NativeWindow(long Handle, string? Title, uint ProcessId, string ProcessImagePath,
        uint State, int X, int Y, int Width, int Height, uint Style, uint StyleEx, bool IsCloaked, bool NeedsElevation);
}
