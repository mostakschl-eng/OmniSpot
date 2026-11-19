using OmniSpot.Stats;
using System.Windows.Media;

namespace OmniSpot.WindowDiscovery;

internal record RunningApplicationInfo(uint ProcessId, string ProcessName, string ProcessImagePath, string DisplayName, ImageSource? Icon, bool IsPackagedApp);

internal class RunningApplicationsService(
    IWindowEnumerator windowEnumerator,
    IconExtractor iconExtractor,
    IPackagedAppsService packagedAppsService,
    AppRegistryCache appRegistryCache)
{
    public IReadOnlyList<RunningApplicationInfo> GetRunningApplications(IEnumerable<string> excludedProcessNames,
        bool includeIcons = true)
    {
        var installedPackages = packagedAppsService.GetInstalledPackages();
        return RustWindowDiscoveryBackend.GetRunningApplications(
                windowEnumerator.GetWindows(), excludedProcessNames, installedPackages.Paths)
            .Select(item =>
            {
                var icon = !includeIcons ? null : item.IsPackagedApp
                    ? iconExtractor.GetByIconPath(packagedAppsService.GetByInstalledPath(item.ProcessImagePath, item.ProcessId)?.IconPath)
                    : iconExtractor.GetByProcessPath(item.ProcessImagePath);

                return new RunningApplicationInfo(
                    ProcessId: item.ProcessId,
                    ProcessName: item.ProcessName,
                    ProcessImagePath: item.ProcessImagePath,
                    DisplayName: appRegistryCache.GetDisplayName(item.ProcessName),
                    Icon: icon,
                    IsPackagedApp: item.IsPackagedApp);
            })
            .ToList();
    }
}
