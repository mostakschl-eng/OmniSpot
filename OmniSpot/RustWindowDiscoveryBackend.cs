using OmniSpot.WindowDiscovery;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using Windows.Win32.Foundation;
using Windows.Win32.UI.WindowsAndMessaging;

namespace OmniSpot;

/// <summary>Thin managed DTO adapter for native Rust window discovery.</summary>
internal static class RustWindowDiscoveryBackend
{
    private const string LibraryName = "omnispot_spotlight_native";
    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web);

    public static List<ApplicationWindow> GetWindows() => ReadArray(NativeGetAll);
    public static List<ApplicationWindow> GetAllWindows() => ReadArray(NativeGetAllIncludingInvalid);
    public static List<ApplicationWindow> GetCachedWindows() => ReadArray(NativeGetCached);
    public static ApplicationWindow? GetCurrentWindow() => ReadOne(NativeGetCurrent);

    public static IReadOnlyList<RustRunningApplication> GetRunningApplications(
        IReadOnlyList<ApplicationWindow> windows,
        IEnumerable<string> excludedProcessNames,
        IEnumerable<string> installedPackages)
    {
        var request = new RunningAppsRequest(windows.Select(ToNative).ToArray(),
            excludedProcessNames.ToArray(), installedPackages.ToArray());
        var bytes = Encoding.UTF8.GetBytes(JsonSerializer.Serialize(request, JsonOptions));
        UIntPtr length = UIntPtr.Zero;
        var pointer = NativeGetRunningApps(bytes, (UIntPtr)bytes.Length, ref length);
        return ReadJson<RustRunningApplication[]>(pointer, length)!;
    }

    // A pair keeps the native allocation length attached until it is freed.
    private static (IntPtr Pointer, UIntPtr Length) Read(NativeRead call)
    {
        UIntPtr length = UIntPtr.Zero;
        var pointer = call(ref length);
        if (pointer == IntPtr.Zero) throw new InvalidOperationException("Rust window discovery returned no response.");
        return (pointer, length);
    }

    private static List<ApplicationWindow> ReadArray(NativeRead call)
    {
        var (pointer, length) = Read(call);
        return ReadJson<RustWindow[]>(pointer, length)!.Select(FromNative).ToList();
    }

    private static ApplicationWindow? ReadOne(NativeRead call)
    {
        var (pointer, length) = Read(call);
        var result = ReadJson<RustWindow>(pointer, length, allowNull: true);
        return result is null ? null : FromNative(result);
    }

    private static T? ReadJson<T>(IntPtr pointer, UIntPtr length, bool allowNull = false) where T : class
    {
        if (pointer == IntPtr.Zero) throw new InvalidOperationException("Rust window discovery returned no response.");
        try
        {
            var bytes = new byte[checked((int)length.ToUInt64())];
            Marshal.Copy(pointer, bytes, 0, bytes.Length);
            return DecodeJson<T>(bytes, allowNull);
        }
        finally { NativeFree(pointer, length); }
    }

    internal static T? DecodeJson<T>(ReadOnlySpan<byte> json, bool allowNull = false) where T : class
    {
        var result = JsonSerializer.Deserialize<T>(json, JsonOptions);
        if (result is null && !allowNull)
            throw new InvalidOperationException("Rust window discovery returned invalid data.");
        return result;
    }

    internal static RustWindow ToNative(ApplicationWindow w) => new(
        RustSwitcherBackend.ToInt64(w.Handle), w.Title, w.ProcessId, w.ProcessImagePath, (uint)w.State,
        (int)w.Position.X, (int)w.Position.Y, (int)w.Size.Width, (int)w.Size.Height,
        (uint)w.Style, (uint)w.StyleEx, w.IsCloaked, w.NeedsElevation);

    internal static ApplicationWindow FromNative(RustWindow w) => new(
        new HWND((nint)w.Handle), w.Title ?? string.Empty, w.ProcessId, w.ProcessImagePath,
        (SHOW_WINDOW_CMD)w.State, new System.Drawing.Point(w.X, w.Y), new System.Drawing.Size(w.Width, w.Height),
        (WINDOW_STYLE)w.Style, (WindowStyleEx)w.StyleEx, w.IsCloaked, w.NeedsElevation);

    [DllImport(LibraryName, EntryPoint = "omnispot_windows_get_all", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr NativeGetAll(ref UIntPtr outputLength);
    [DllImport(LibraryName, EntryPoint = "omnispot_windows_get_all_including_invalid", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr NativeGetAllIncludingInvalid(ref UIntPtr outputLength);
    [DllImport(LibraryName, EntryPoint = "omnispot_windows_get_cached", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr NativeGetCached(ref UIntPtr outputLength);
    [DllImport(LibraryName, EntryPoint = "omnispot_windows_get_current", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr NativeGetCurrent(ref UIntPtr outputLength);
    [DllImport(LibraryName, EntryPoint = "omnispot_running_apps", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr NativeGetRunningApps(byte[] request, UIntPtr requestLength, ref UIntPtr outputLength);
    [DllImport(LibraryName, EntryPoint = "omnispot_spotlight_free", CallingConvention = CallingConvention.Cdecl)]
    private static extern void NativeFree(IntPtr pointer, UIntPtr length);

    private delegate IntPtr NativeRead(ref UIntPtr outputLength);
    internal sealed record RustRunningApplication(uint ProcessId, string ProcessName, string ProcessImagePath, bool IsPackagedApp);
    private sealed record RunningAppsRequest(RustWindow[] Windows, string[] ExcludedProcessNames, string[] Packages);
    internal sealed record RustWindow(long Handle, string? Title, uint ProcessId, string ProcessImagePath,
        uint State, int X, int Y, int Width, int Height, uint Style, uint StyleEx, bool IsCloaked, bool NeedsElevation);
}
