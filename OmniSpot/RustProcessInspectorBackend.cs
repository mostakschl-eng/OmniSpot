using System.Runtime.InteropServices;
using System.IO;
using System.Text;
using System.Text.Json;

namespace OmniSpot;

/// <summary>FFI adapter for native process inspection.</summary>
internal sealed class RustProcessInspectorBackend
{
    private const string LibraryName = "omnispot_spotlight_native";
    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web);

    public bool NeedsElevation(uint processId) => Invoke<bool>("needsElevation", processId: processId);
    public bool IsWindowsExecutable(string path) => Invoke<bool>("isWindowsExecutable", path: path);
    public string GetProcessDisplayName(string path) => Invoke<string>("displayName", path: path) ?? Path.GetFileNameWithoutExtension(path);
    public Task<bool> WaitForPotentialElevation(string path, int timeoutMs) =>
        Task.Run(() => Invoke<bool>("waitForPotentialElevation", path: path, timeoutMs: timeoutMs));

    private static T? Invoke<T>(string operation, string? path = null, uint? processId = null, int? timeoutMs = null)
    {
        var request = JsonSerializer.SerializeToUtf8Bytes(new { operation, path, processId, timeoutMs }, JsonOptions);
        var pointer = NativeInvoke(request, (nuint)request.Length, out var length);
        if (pointer == IntPtr.Zero) throw new InvalidOperationException($"Rust process-inspection operation '{operation}' failed.");
        try
        {
            var json = Marshal.PtrToStringUTF8(pointer, checked((int)length))!;
            return JsonSerializer.Deserialize<T>(json, JsonOptions);
        }
        finally { NativeFree(pointer, length); }
    }

    [DllImport(LibraryName, EntryPoint = "omnispot_process_inspector", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr NativeInvoke(byte[] request, nuint requestLength, out nuint outputLength);
    [DllImport(LibraryName, EntryPoint = "omnispot_spotlight_free", CallingConvention = CallingConvention.Cdecl)]
    private static extern void NativeFree(IntPtr pointer, nuint length);
}
