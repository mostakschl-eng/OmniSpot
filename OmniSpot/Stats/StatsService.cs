using Microsoft.Extensions.Logging;
using OmniSpot.Stats.Migrations;
using OmniSpot.Stats.Storage;
using AppConfig = OmniSpot.Configuration.Configuration;

namespace OmniSpot.Stats;

/// <summary>Managed lifecycle and LiteDB adapter for the Rust stats runtime.</summary>
internal sealed class StatsService(
    SessionStats sessionStats,
    AppRegistryCache registryCache,
    StatsDbProvider dbProvider,
    StatsMigrationRunner statsMigrationRunner,
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
            if (!statsMigrationRunner.RunPending())
            {
                logger.LogError("Stats DB migrations failed — stats collection not started");
                return;
            }

            LoadTodaysBucket();
            rustStatsBackend.SetFlushCallback(() => Flush("Rust five-minute timer"));
            _storagePrepared = true;
        }

        if (!statsEnabled) logger.LogInformation("Stats collection is disabled");
        if (rustStatsBackend.ApplyConfiguration(statsEnabled))
        {
            // Rust drains and joins its worker before the LiteDB snapshot is persisted.
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
            registryCache.PersistNativeEntries();
            using var database = dbProvider.Get();
            var collection = database.GetCollection<DailyBucketDocument>(DailyBucketDocument.CollectionName);
            var snapshots = sessionStats.GetSnapshots();
            foreach (var snapshot in snapshots)
            {
                if (snapshot.Date == default)
                {
                    logger.LogWarning("Skipping stats flush for a snapshot without a date (reason={Reason})", reason);
                    continue;
                }
                collection.Upsert(snapshot);
                logger.LogDebug("Stats flushed for {Date} (reason={Reason})", snapshot.Date.ToString("yyyy-MM-dd"), reason);
            }
            if (snapshots.Count > 1) sessionStats.ClearRollover();
        }
        catch (Exception ex)
        {
            logger.LogError(ex, "Error flushing stats to database");
        }
        finally { _flushLock.Release(); }
    }

    private void LoadTodaysBucket()
    {
        try
        {
            if (!dbProvider.Exists()) return;
            using var database = dbProvider.Get();
            var today = DateOnly.FromDateTime(DateTime.Today);
            var collection = database.GetCollection<DailyBucketDocument>(DailyBucketDocument.CollectionName);
            var existing = collection.FindOne(bucket => bucket.Date == today);
            if (existing is not null)
            {
                sessionStats.LoadFrom(existing);
                logger.LogDebug("Loaded existing stats for {Date}", today.ToString("yyyy-MM-dd"));
            }
        }
        catch (Exception ex) { logger.LogError(ex, "Error loading today's stats bucket from database"); }
    }
}
