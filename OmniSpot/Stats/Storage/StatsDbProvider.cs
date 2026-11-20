using System.IO;

namespace OmniSpot.Stats.Storage;

internal sealed class StatsDbProvider(string directory)
{
    private readonly RustDatabase _database = new(Path.Combine(directory, "stats.sqlite3"));
    private readonly string _legacyPath = Path.Combine(directory, "stats.db");

    public RustDatabase? OpenExisting()
    {
        if (_database.IsReady()) return _database;
        return LegacyDatabaseImport.Exists(_legacyPath) ? OpenOrCreate() : null;
    }

    public RustDatabase OpenOrCreate()
    {
        LegacyDatabaseImport.Stats(_database, _legacyPath);
        return _database;
    }
}
