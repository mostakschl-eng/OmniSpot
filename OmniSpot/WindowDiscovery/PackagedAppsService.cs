using OmniSpot;

namespace OmniSpot.WindowDiscovery;

/// <summary>Managed adapter for Rust-owned Windows package discovery.</summary>
internal sealed class PackagedAppsService(RustPackagedAppsBackend rust)
    : IPackagedAppsService
{
    public IReadOnlyList<(string Name, string Path, string? Aumid, string? IconPath)> GetSearchableApps() =>
        rust.GetSearchableApps();

    public InstalledPackages GetInstalledPackages() => rust.GetInstalledPackages();

    public string? GetDisplayName(string installPath) => rust.GetDisplayName(installPath);

    public PackagedAppInfo? GetByInstalledPath(string path, uint? processId) =>
        rust.GetByInstalledPath(path, processId);

    public string? GetInstalledPathByAumid(string? aumid) => rust.GetInstalledPathByAumid(aumid);

    public PackagedAppInfo? GetByAumid(string? aumid) => rust.GetByAumid(aumid);

    public PackagedAppInfo? GetByPackageFamilyName(string packageFamilyName) =>
        rust.GetByPackageFamilyName(packageFamilyName);
}

public sealed record PackagedAppInfo(string Aumid, string? IconPath);
