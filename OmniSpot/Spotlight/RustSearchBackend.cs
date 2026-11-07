using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using System.IO;

namespace OmniSpot.Spotlight;

internal sealed class RustSearchBackend
{
    private const string LibraryName = "omnispot_spotlight_native";
    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web);
    private static readonly Lazy<bool> NativeAvailable = new(IsNativeAvailable);
    private readonly Lazy<Task> _indexInitialization;
    private readonly string _persistencePath;

    public RustSearchBackend()
    {
        _indexInitialization = new Lazy<Task>(InitializeIndexAsync, LazyThreadSafetyMode.ExecutionAndPublication);
        _persistencePath = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "OmniSpot",
            "spotlight-index.json");
    }

    public bool IsAvailable => NativeAvailable.Value;

    public void WarmUp()
    {
        if (!IsAvailable)
        {
            return;
        }

        _ = _indexInitialization.Value;
    }

    private Task InitializeIndexAsync()
    {
        return Task.Run(() =>
        {
            var roots = new[]
            {
                Environment.GetFolderPath(Environment.SpecialFolder.StartMenu),
                Environment.GetFolderPath(Environment.SpecialFolder.CommonStartMenu),
                Environment.GetFolderPath(Environment.SpecialFolder.Desktop),
                Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments),
                Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
                Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles),
                Environment.GetFolderPath(Environment.SpecialFolder.CommonProgramFiles)
            }.Where(path => !string.IsNullOrWhiteSpace(path)).Distinct(StringComparer.OrdinalIgnoreCase).ToArray();

            var configBytes = Encoding.UTF8.GetBytes(JsonSerializer.Serialize(new RustIndexConfig(roots, _persistencePath), JsonOptions));
            NativeInitialize(configBytes, (UIntPtr)configBytes.Length);
        });
    }

    public async Task RecordLaunchAsync(string target, CancellationToken cancellationToken = default)
    {
        WarmUp();
        await _indexInitialization.Value.WaitAsync(cancellationToken);
        var requestBytes = Encoding.UTF8.GetBytes(JsonSerializer.Serialize(
            new RustRecordLaunchRequest(target, _persistencePath), JsonOptions));
        NativeRecordLaunch(requestBytes, (UIntPtr)requestBytes.Length);
    }

    public async Task<IReadOnlyList<SearchResult>> SearchIndexedAsync(
        string query,
        int maxResults,
        SearchResultType? resultType = null,
        CancellationToken cancellationToken = default)
    {
        WarmUp();
        await _indexInitialization.Value.WaitAsync(cancellationToken);
        cancellationToken.ThrowIfCancellationRequested();
        if (!IsAvailable)
        {
            return [];
        }

        var requestBytes = Encoding.UTF8.GetBytes(JsonSerializer.Serialize(new RustIndexSearchRequest(query, maxResults), JsonOptions));
        UIntPtr outputLength = UIntPtr.Zero;
        var outputPointer = NativeIndexSearch(requestBytes, (UIntPtr)requestBytes.Length, ref outputLength);
        var managedLength = checked((int)outputLength.ToUInt64());
        if (outputPointer == IntPtr.Zero || managedLength <= 0)
        {
            return [];
        }

        try
        {
            cancellationToken.ThrowIfCancellationRequested();
            var outputBytes = new byte[managedLength];
            Marshal.Copy(outputPointer, outputBytes, 0, managedLength);
            var nativeResults = JsonSerializer.Deserialize<RustIndexedResult[]>(outputBytes, JsonOptions) ?? [];
            return nativeResults
                .Where(result => resultType is null ||
                                 string.Equals(result.ResultType, resultType.Value.ToString(), StringComparison.OrdinalIgnoreCase))
                .Select(result => new SearchResult(
                result.Title,
                result.Subtitle,
                result.Target,
                Enum.TryParse<SearchResultType>(result.ResultType, true, out var parsedType)
                    ? parsedType
                    : SearchResultType.File,
                null,
                result.Score)).ToList();
        }
        finally
        {
            NativeFree(outputPointer, outputLength);
        }
    }

    private static bool IsNativeAvailable()
    {
        if (!NativeLibrary.TryLoad(LibraryName, typeof(RustSearchBackend).Assembly, null, out var handle))
        {
            return false;
        }

        NativeLibrary.Free(handle);
        return true;
    }

    [DllImport(LibraryName, EntryPoint = "omnispot_spotlight_free", CallingConvention = CallingConvention.Cdecl)]
    private static extern void NativeFree(IntPtr pointer, UIntPtr length);

    [DllImport(LibraryName, EntryPoint = "omnispot_spotlight_initialize", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeInitialize(byte[] config, UIntPtr configLength);

    [DllImport(LibraryName, EntryPoint = "omnispot_spotlight_search_index", CallingConvention = CallingConvention.Cdecl)]
    private static extern IntPtr NativeIndexSearch(byte[] request, UIntPtr requestLength, ref UIntPtr outputLength);

    [DllImport(LibraryName, EntryPoint = "omnispot_spotlight_record_launch", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeRecordLaunch(byte[] request, UIntPtr requestLength);

    private sealed record RustIndexConfig(string[] Roots, string PersistencePath);

    private sealed record RustIndexSearchRequest(string Query, int MaxResults);

    private sealed record RustIndexedResult(
        string Title,
        string Subtitle,
        string Target,
        string ResultType,
        int Score);

    private sealed record RustRecordLaunchRequest(string Target, string PersistencePath);
}
