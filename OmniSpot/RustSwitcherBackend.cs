using OmniSpot.Configuration;
using OmniSpot.WindowDiscovery;
using System.Globalization;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using Windows.Win32.Foundation;

namespace OmniSpot;

/// <summary>Marshals window-switching requests to the native Rust switcher.</summary>
internal static class RustSwitcherBackend
{
    private const string LibraryName = "omnispot_spotlight_native";
    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web);

    public static void Open(ApplicationConfiguration app)
    {
        var request = new OpenRequest(ToNative(app), CultureInfo.CurrentCulture.Name);
        var response = Call<OpenRequest, NativeResponse<bool>>(request, NativeOpen);
        Ensure(response);
    }

    public static void ActivateWindow(ApplicationWindow window)
    {
        var request = new ActivateRequest(ToInt64(window.Handle).ToString(CultureInfo.InvariantCulture), window.NeedsElevation);
        var bytes = Serialize(request);
        if (NativeActivate(bytes, (UIntPtr)bytes.Length) == 0)
            throw new InvalidOperationException("Rust failed to activate the window.");
    }

    private static NativeApplication ToNative(ApplicationConfiguration app) => new(
        (uint)app.Key, app.ProcessPath, app.CycleMode.ToString(), app.StartIfNotRunning,
        app.Type == ApplicationType.Packaged ? "Packaged" : "Win32", app.Aumid);

    private static TResponse Call<TRequest, TResponse>(TRequest request,
        NativeJsonCall call)
    {
        var bytes = Serialize(request);
        UIntPtr length = UIntPtr.Zero;
        var pointer = call(bytes, (UIntPtr)bytes.Length, ref length);
        if (pointer == IntPtr.Zero) throw new InvalidOperationException("Rust switcher returned no response.");
        try
        {
            var output = new byte[checked((int)length.ToUInt64())];
            Marshal.Copy(pointer, output, 0, output.Length);
            return JsonSerializer.Deserialize<TResponse>(output, JsonOptions)
                ?? throw new InvalidOperationException("Rust switcher returned an invalid response.");
        }
        finally { NativeFree(pointer, length); }
    }

    [DllImport(LibraryName, EntryPoint = "omnispot_switcher_open", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr NativeOpen(byte[] request, UIntPtr requestLength, ref UIntPtr outputLength);

    private static byte[] Serialize<T>(T value) => Encoding.UTF8.GetBytes(JsonSerializer.Serialize(value, JsonOptions));
    internal static unsafe long ToInt64(HWND handle) => (long)handle.Value;

    private static void Ensure<T>(NativeResponse<T> response)
    {
        if (!response.Ok) throw new InvalidOperationException(response.Error ?? "Rust switcher operation failed.");
    }

    private delegate IntPtr NativeJsonCall(byte[] request, UIntPtr requestLength, ref UIntPtr outputLength);

    [DllImport(LibraryName, EntryPoint = "omnispot_switcher_activate", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeActivate(byte[] request, UIntPtr requestLength);
    [DllImport(LibraryName, EntryPoint = "omnispot_spotlight_free", CallingConvention = CallingConvention.Cdecl)]
    private static extern void NativeFree(IntPtr pointer, UIntPtr length);

    private sealed record OpenRequest(NativeApplication Application, string Culture);
    private sealed record ActivateRequest(string Target, bool NeedsElevation);
    private sealed record NativeApplication(uint Key, string ProcessPath, string CycleMode,
        bool StartIfNotRunning, string ApplicationType, string? Aumid);
    private sealed record NativeResponse<T>(bool Ok, T? Value, string? Error);
}
