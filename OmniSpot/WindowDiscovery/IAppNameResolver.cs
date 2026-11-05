using System.Windows.Input;

namespace OmniSpot.WindowDiscovery;

internal interface IAppNameResolver
{
    Key? GetDynamicKey(string processName, string processPath);
}