using Microsoft.Extensions.Logging;
using OmniSpot.Stats.Storage;

namespace OmniSpot.Stats;

internal class StatsRepository(StatsDbProvider dbProvider, RustStatsBackend rustStatsBackend, ILogger<StatsRepository> logger)
{
    /// <summary>Returns historic buckets before today in descending date order.</summary>
    public IReadOnlyList<DailyBucketDocument> GetAllHistoricBuckets()
    {
        var today = DateOnly.FromDateTime(DateTime.Today);
        try
        {
            using var db = dbProvider.Get();
            var collection = db.GetCollection<DailyBucketDocument>(DailyBucketDocument.CollectionName);
            return rustStatsBackend.GetHistoricBuckets(collection.FindAll().ToList(), today);
        }
        catch (Exception ex)
        {
            logger.LogError(ex, "Failed to load historical stats from database");
            return [];
        }
    }
}
