using LiteDB;
using Microsoft.Extensions.Logging;

namespace OmniSpot.Spotlight;

internal class SpotlightUsageStore(LiteDatabase db, ILogger<SpotlightUsageStore> logger) : ISpotlightUsageStore
{
    private const string CollectionName = "spotlight_usage";
    private readonly object _lock = new();
    private Dictionary<string, int>? _counts;

    public int GetBoost(string target)
    {
        lock (_lock)
        {
            EnsureLoaded();
            return Math.Min(6, _counts!.GetValueOrDefault(target)) * 2;
        }
    }

    public IReadOnlyList<string> GetFrequentTargets(int maxCount)
    {
        lock (_lock)
        {
            EnsureLoaded();
            return _counts!
                .OrderByDescending(entry => entry.Value)
                .Take(maxCount)
                .Select(entry => entry.Key)
                .ToList();
        }
    }

    public void RecordLaunch(string target)
    {
        lock (_lock)
        {
            EnsureLoaded();
            var key = target.ToUpperInvariant();
            var count = _counts!.GetValueOrDefault(key) + 1;
            _counts![key] = count;

            try
            {
                db.GetCollection<BsonDocument>(CollectionName).Upsert(new BsonDocument
                {
                    ["_id"] = key,
                    ["count"] = count
                });
            }
            catch (Exception ex)
            {
                logger.LogWarning(ex, "Failed to save Spotlight usage for {Target}", target);
            }
        }
    }

    private void EnsureLoaded()
    {
        if (_counts is not null)
        {
            return;
        }

        _counts = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);
        try
        {
            foreach (var item in db.GetCollection<BsonDocument>(CollectionName).FindAll())
            {
                _counts[item["_id"].AsString] = item["count"].AsInt32;
            }
        }
        catch (Exception ex)
        {
            logger.LogWarning(ex, "Failed to load Spotlight usage");
        }
    }
}