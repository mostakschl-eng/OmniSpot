using OmniSpot.Stats.Storage;

namespace OmniSpot.Stats;

/// <summary>Managed read/write adapter for the Rust-owned session stats state.</summary>
internal sealed class SessionStats
{
    private readonly RustStatsBackend _rustStatsBackend;

    public SessionStats(RustStatsBackend rustStatsBackend)
    {
        _rustStatsBackend = rustStatsBackend;
        _rustStatsBackend.SetChangedCallback(() => DataChanged?.Invoke());
    }

    public event Action? DataChanged;

    public void LoadFrom(DailyBucketDocument document) => _rustStatsBackend.LoadToday(document);

    public void Clear() => _rustStatsBackend.ClearSession();

    public void ClearRollover() => _rustStatsBackend.ClearRollover();

    public IReadOnlyList<DailyBucketDocument> GetSnapshots() => _rustStatsBackend.GetSnapshots();
}
