namespace OmniSpot.Spotlight;

internal interface ISpotlightUsageStore
{
    int GetBoost(string target);
    IReadOnlyList<string> GetFrequentTargets(int maxCount);
    void RecordLaunch(string target);
}