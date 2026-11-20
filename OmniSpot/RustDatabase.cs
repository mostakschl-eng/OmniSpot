using System.IO;
using System.Runtime.InteropServices;
using System.Text.Json;

namespace OmniSpot;

internal sealed class RustDatabase(string path)
{
    private const string LibraryName = "omnispot_spotlight_native";
    private static readonly JsonSerializerOptions Json = new(JsonSerializerDefaults.Web);

    internal sealed record Row(string Collection, string Key, object Value);

    public bool IsReady() => Execute(new { path, operation = "ready" }).GetBoolean();

    public void Initialize(IEnumerable<Row> rows) =>
        Execute(new { path, operation = "initialize", rows });

    public T[] Read<T>(string collection, string? key = null) =>
        Execute(new { path, operation = "read", collection, key }).Deserialize<T[]>(Json)!;

    public void Write(IEnumerable<Row> rows, bool overwrite = true) =>
        Execute(new { path, operation = "write", rows, overwrite });

    public void Clear(string collection) =>
        Execute(new { path, operation = "clear", collection });

    public (Configuration.Configuration Configuration, string? ValidationMessage) LoadSettings(bool sanitize = true)
    {
        var result = Execute(new { path, operation = "loadSettings", sanitize });
        var configuration = result.GetProperty("configuration").Deserialize<RustConfigurationBackend.NativeConfiguration>(Json)!;
        return (RustConfigurationBackend.FromNative(configuration), result.GetProperty("validationMessage").GetString());
    }

    public void SaveSettings(Configuration.Configuration config) =>
        Execute(new { path, operation = "saveSettings", configuration = RustConfigurationBackend.ToNativeConfiguration(config) });

    public Stats.Storage.DailyBucketDocument[] ReadStatsHistory(DateOnly today) =>
        Execute(new { path, operation = "statsHistory", today }).Deserialize<Stats.Storage.DailyBucketDocument[]>(Json)!;

    public int FlushStats() => Execute(new { path, operation = "flushStats" }).GetInt32();

    public void RestoreStats(DateOnly today) => Execute(new { path, operation = "restoreStats", today });

    public void ResetStats() => Execute(new { path, operation = "resetStats" });

    private static JsonElement Execute(object request)
    {
        var bytes = JsonSerializer.SerializeToUtf8Bytes(request, Json);
        var pointer = NativeExecute(bytes, (nuint)bytes.Length, out var length);
        if (pointer == IntPtr.Zero) throw new IOException("Rust database returned no response.");
        try
        {
            using var response = JsonDocument.Parse(Marshal.PtrToStringUTF8(pointer, checked((int)length))!);
            if (response.RootElement.TryGetProperty("error", out var error))
                throw new IOException(error.GetString());
            return response.RootElement.GetProperty("value").Clone();
        }
        finally { NativeFree(pointer, length); }
    }

    [DllImport(LibraryName, EntryPoint = "omnispot_storage_execute", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr NativeExecute(byte[] request, nuint length, out nuint outputLength);

    [DllImport(LibraryName, EntryPoint = "omnispot_spotlight_free", CallingConvention = CallingConvention.Cdecl)]
    private static extern void NativeFree(IntPtr pointer, nuint length);
}
