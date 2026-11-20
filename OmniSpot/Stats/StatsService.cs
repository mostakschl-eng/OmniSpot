using Microsoft.Extensions.Logging;
using OmniSpot.Stats.Storage;
using AppConfig = OmniSpot.Configuration.Configuration;

namespace OmniSpot.Stats;

/// <summary>Managed lifecycle and persistence adapter for the Rust stats runtime.</summary>
internal sealed class StatsService(
    SessionStats sessionStats,
    AppRegistryCache registryCache,
    StatsDbProvider dbProvider,
    RustStatsBackend rustStatsBackend,
    ILogger<StatsService> logger) : IDisposable
{
    private readonly SemaphoreSlim _flushLock = new(1, 1);
    private bool _storagePrepared;

    private bool IsRunning => rustStatsBackend.IsRunning;

    public void Start(bool statsEnabled)
    {
        ApplyConfiguration(statsEnabled, "app startup");
    }

    public void UpdateConfiguration(AppConfig config)
    {
        ApplyConfiguration(config.StatsEnabled, "stats disabled");
    }

    public void Enqueue(StatsEvent statsEvent)
    {
        if (!IsRunning) return;
        if (!rustStatsBackend.Enqueue(statsEvent))
            logger.LogWarning("Rust stats queue rejected event {Type}", statsEvent.GetType().Name);
    }

    public void Dispose()
    {
        if (rustStatsBackend.ApplyConfiguration(false)) PersistSnapshots("app exit");
        _flushLock.Dispose();
    }

    private void ApplyConfiguration(bool statsEnabled, string reason)
    {
        if (statsEnabled && !_storagePrepared)
        {
            try { LoadTodaysBucket(); }
            catch (Exception ex)
            {
                logger.LogError(ex, "Failed to prepare the stats database");
                return;
            }
            rustStatsBackend.SetFlushCallback(() => Flush("Rust five-minute timer"));
            _storagePrepared = true;
        }

        if (!statsEnabled) logger.LogInformation("Stats collection is disabled");
        if (rustStatsBackend.ApplyConfiguration(statsEnabled))
        {
            // Rust drains and joins its worker before the stats snapshot is persisted.
            PersistSnapshots(reason);
            logger.LogInformation("Stats collection stopped");
        }
        else if (statsEnabled)
        {
            logger.LogInformation("Stats collection running in Rust");
        }
    }

    public void Flush(string reason)
    {
        if (!IsRunning) return;
        PersistSnapshots(reason);
    }

    private void PersistSnapshots(string reason)
    {
        _flushLock.Wait();
        try
        {
            var count = dbProvider.OpenOrCreate().FlushStats();
            logger.LogDebug("Flushed {Count} stats buckets (reason={Reason})", count, reason);
            registryCache.PersistNativeEntries();
        }
        catch (Exception ex)
        {
            logger.LogError(ex, "Error flushing stats to database");
        }
        finally { _flushLock.Release(); }
    }

    public void Reset()
    {
        _flushLock.Wait();
        try
        {
            if (dbProvider.OpenExisting() is { } database) database.ResetStats();
            else sessionStats.Clear();
        }
        finally { _flushLock.Release(); }
    }

    private void LoadTodaysBucket()
    {
        dbProvider.OpenExisting()?.RestoreStats(DateOnly.FromDateTime(DateTime.Today));
    }
}
