namespace OmniSpot.WindowDiscovery;

/// <summary>Package path data returned by the native Rust package service.</summary>
internal sealed class InstalledPackages
{
    private readonly string[] _paths;

    public static readonly InstalledPackages Empty = new([]);

    public InstalledPackages(IEnumerable<string> paths) =>
        _paths = paths.Distinct(StringComparer.OrdinalIgnoreCase).ToArray();

    public int Count => _paths.Length;

    public IReadOnlyCollection<string> Paths => _paths;
}
