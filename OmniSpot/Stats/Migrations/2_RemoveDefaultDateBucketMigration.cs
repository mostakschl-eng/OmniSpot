using LiteDB;
using OmniSpot.Stats.Storage;
using OmniSpot;

namespace OmniSpot.Stats.Migrations;

/// <summary>
/// Removes the bucket stored under the default date (0001-01-01), which could be flushed by an untouched
/// session snapshot before the first event of the day was recorded.
/// </summary>
internal class RemoveDefaultDateBucketMigration : IStatsMigration
{
    public int Version => 2;

    public void Up(ILiteDatabase db)
    {
        var col = db.GetCollection(DailyBucketDocument.CollectionName);
        var key = default(DateOnly).ToString("yyyy-MM-dd");
        if (RustMigrationBackend.IsDefaultStatsBucketKey(key))
        {
            col.Delete(new BsonValue(key));
        }
    }
}
