using OmniSpot.Stats;
using System.Windows.Input;

namespace OmniSpot.WindowDiscovery;

internal class AppNameResolver(IAppRegistryCache appRegistryCache) : IAppNameResolver
{
    /// <summary>
    /// Resolves the display name for a process via <see cref="IAppRegistryCache"/> and returns
    /// the corresponding letter key, or null if the resolved name does not start with a letter.
    /// </summary>
    public Key? GetDynamicKey(string processName, string processPath)
    {
        var displayName = appRegistryCache.GetDisplayName(processName, processPath);

        return RustDynamicModeBackend.ResolveDynamicKey(displayName) is { } key
            ? (System.Windows.Input.Key)key
            : null;
    }
}
