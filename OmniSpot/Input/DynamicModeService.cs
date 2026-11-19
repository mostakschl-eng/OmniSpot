using OmniSpot.Configuration;
using OmniSpot.WindowDiscovery;
using System.Windows.Input;

namespace OmniSpot.Input;

/// <summary>Managed adapter supplying current app metadata to Rust dynamic mode logic.</summary>
internal sealed class DynamicModeService(
    IWindowEnumerator windowEnumerator,
    IAppNameResolver appNameResolver,
    IPackagedAppsService packagedAppsService)
{
    public IReadOnlyList<ApplicationConfiguration> GetAllDynamicApps(
        IReadOnlyList<ApplicationConfiguration> staticApps,
        IReadOnlyList<ApplicationWindow> windows)
    {
        var installed = packagedAppsService.GetInstalledPackages();
        return RustDynamicModeBackend.Build(staticApps, windows, appNameResolver, installed.Paths.ToArray(), null);
    }

    public IReadOnlyList<ApplicationConfiguration> GetAppsForKey(
        Key letter,
        IReadOnlyList<ApplicationConfiguration> staticApps)
    {
        if (staticApps.Any(app => app.Key == letter)) return [];
        return RustDynamicModeBackend.Build(staticApps, windowEnumerator.GetCachedWindows(), appNameResolver,
            Array.Empty<string>(), (uint)letter);
    }
}
