using LiteDB;
using OmniSpot.Stats.Storage;

namespace OmniSpot.Stats.Migrations;

internal class DateTimeToDateOnlyMigration(TimeProvider timeProvider) : IStatsMigration
{
    private readonly RustStatsBackend _rust = new();
    public int Version => 1;

    public void Up(ILiteDatabase db)
    {
        var col = db.GetCollection(DailyBucketDocument.CollectionName);
        var docs = col.FindAll().ToList();
        var zone = timeProvider.LocalTimeZone;

        foreach (var doc in docs)
        {
            var oldId = doc["_id"];

            if (oldId.Type != BsonType.DateTime)
            {
                continue;
            }

            // LiteDB reads DateTime as local time; the stored instant is interpreted as UTC.
            var utc = oldId.AsDateTime.ToUniversalTime();
            var dateKey = _rust.GetLocalDateKey(utc, zone);
            var newId = new BsonValue(dateKey);

            doc["_id"] = newId;

            col.Delete(oldId);
            col.Insert(doc);
        }
    }
}
