namespace OmniSpot.WindowDiscovery;

public interface IProcessPathExtractor
{
    string? GetProcessImagePath(uint processId);
}