namespace OmniSpot.Stats;

internal interface IAppRegistryCache
{
    string GetDisplayName(string processName, string? processPath = null);
}