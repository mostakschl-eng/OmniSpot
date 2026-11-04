using LiteDB;

namespace OmniSpot.Configuration.Storage;

internal class MigrationRecord
{
    [BsonId]
    public int Version { get; init; }

    public DateTime ExecutedAt { get; init; }
}