namespace OmniSpot.WindowDiscovery;

internal interface IPackagedAppsService
{
    InstalledPackages GetInstalledPackages();
    PackagedAppInfo? GetByInstalledPath(string path, uint? processId);
    PackagedAppInfo? GetByAumid(string? aumid);
    PackagedAppInfo? GetByPackageFamilyName(string packageFamilyName);
    string? GetInstalledPathByAumid(string? aumid);
    string? GetDisplayName(string installPath);
    IReadOnlyList<(string Name, string Path, string? Aumid, string? IconPath)> GetSearchableApps();
}
