using OmniSpot.Stats.Storage;

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
