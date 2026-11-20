using System.Runtime.InteropServices;
using System.Text.Json;

namespace OmniSpot;

/// <summary>Runs legacy migration version selection and ordering in Rust.</summary>
internal static class RustMigrationBackend
{
    private const string LibraryName = "omnispot_spotlight_native";

    public static IReadOnlyList<int> GetPendingVersions(IEnumerable<int> available, IEnumerable<int> executed)
    {
        var request = JsonSerializer.SerializeToUtf8Bytes(new { availableVersions = available, executedVersions = executed },
            new JsonSerializerOptions(JsonSerializerDefaults.Web));
        var pointer = NativePendingVersions(request, (nuint)request.Length, out var length);
        if (pointer == IntPtr.Zero) throw new InvalidOperationException("Rust migration version selection failed.");
        try
        {
            var json = Marshal.PtrToStringUTF8(pointer, checked((int)length))!;
            return JsonSerializer.Deserialize<int[]>(json) ?? [];
        }
        finally { NativeFree(pointer, length); }
    }

    public static bool IsDefaultStatsBucketKey(string key)
    {
        var request = JsonSerializer.SerializeToUtf8Bytes(key);
        return NativeIsDefaultStatsBucketKey(request, (nuint)request.Length) != 0;
    }

    [DllImport(LibraryName, EntryPoint = "omnispot_migrations_pending_versions", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr NativePendingVersions(byte[] request, nuint requestLength, out nuint length);

    [DllImport(LibraryName, EntryPoint = "omnispot_stats_is_default_bucket_key", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeIsDefaultStatsBucketKey(byte[] request, nuint requestLength);

    [DllImport(LibraryName, EntryPoint = "omnispot_spotlight_free", CallingConvention = CallingConvention.Cdecl)]
    private static extern void NativeFree(IntPtr pointer, nuint length);
}
