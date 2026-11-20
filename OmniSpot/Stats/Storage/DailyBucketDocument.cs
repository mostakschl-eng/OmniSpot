using LiteDB;

namespace OmniSpot.Stats.Storage;

internal sealed class FastestSwitchRecord
{
    public int DurationMs { get; init; }
    public required string AppName { get; init; }
    public required string Letter { get; init; }
    public DateTime Date { get; init; }
}

internal sealed class AppUsageStats(int switches = 0, int peeks = 0, int totalPeekTimeMs = 0, int totalSwitchTimeMs = 0)
{
    public int Switches { get; init; } = switches;
    public int Peeks { get; init; } = peeks;
    public int TotalPeekTimeMs { get; init; } = totalPeekTimeMs;
    public int TotalSwitchTimeMs { get; init; } = totalSwitchTimeMs;
}

/// <summary>Persisted daily bucket; stat mutations are owned by Rust.</summary>
internal sealed class DailyBucketDocument
{
    public const string CollectionName = "daily_buckets";

    [BsonId]
    public DateOnly Date { get; set; }

    public int TotalSwitches { get; set; }
    public int TotalTimeSavedMs { get; set; }
    public int TotalPeeks { get; set; }
    public Dictionary<string, AppUsageStats> StaticAppUsage { get; init; } = [];
    public Dictionary<string, AppUsageStats> DynamicAppUsage { get; init; } = [];
    public int AltTabSwitches { get; set; }
    public int AltTabKeystrokes { get; set; }
    public FastestSwitchRecord? FastestSwitch { get; set; }
}
