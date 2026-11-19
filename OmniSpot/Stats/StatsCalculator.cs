using OmniSpot.Stats.Storage;
using System.Windows.Input;

namespace OmniSpot.Stats;

internal sealed record StatsMetrics(
    string LifeGained,
    int TeleportStreak,
    string MuscleMemoGrade,
    string MuscleMemoPersona,
    IReadOnlyList<AppUsageSummary> Podium,
    AppUsageSummary? MostPeekedApp,
    StaleShortcutInfo? FirstStaleShortcut,
    bool IsMaintenanceWarmup,
    bool IsSessionWarmup,
    int AltTabRelapsePct,
    int AltTabSoberStreak,
    int WastedKeystrokes,
    int TotalSwitchCount,
    FastestSwitchRecord? PersonalBestRecord,
    int TotalPeekCount,
    string AvgGlanceSec,
    string AvgLatency,
    int StaticPct,
    int TodaySwitchCount);

internal sealed record AppUsageSummary(
    string ProcessName,
    string DisplayName,
    int Switches,
    int Peeks,
    int TotalPeekTimeMs);

internal sealed record StaleShortcutInfo(string Letter, string ProcessName, string DisplayName);

/// <summary>Managed DTO adapter for Rust-owned stats metric calculations.</summary>
internal sealed class StatsCalculator(AppRegistryCache appRegistryCache, RustStatsBackend rustStatsBackend)
{
    public StatsMetrics Compute(
        IReadOnlyList<DailyBucketDocument> allBuckets,
        DailyBucketDocument today,
        List<(string ProcessName, Key Key)> configuredStaticApps) =>
        rustStatsBackend.Calculate(allBuckets, today, configuredStaticApps, appRegistryCache);
}
