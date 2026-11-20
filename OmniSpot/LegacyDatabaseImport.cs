using System.Globalization;
using System.IO;
using LiteDB;
using Microsoft.Extensions.Logging.Abstractions;
using OmniSpot.Configuration.Migrations;
using OmniSpot.Configuration.Storage;
using OmniSpot.Stats.Migrations;
using OmniSpot.Stats.Storage;
using OmniSpot.WindowDiscovery;

namespace OmniSpot;

internal static class LegacyDatabaseImport
{
    public static void Settings(RustDatabase target, string path, IPackagedAppsService packagedApps)
    {
        if (target.IsReady()) return;
        Import(target, path, db =>
        {
            var migrations = new MigrationRunner(db, [new FixPackagedAppsMigration(packagedApps)],
                NullLogger<MigrationRunner>.Instance);
            if (!migrations.RunPending()) throw new IOException("Legacy settings migration failed.");

            var rows = db.GetCollection<AppRegistryDocument>(AppRegistryDocument.CollectionName).FindAll()
                .Select(entry => new RustDatabase.Row(AppRegistryDocument.CollectionName, entry.ProcessName, entry))
                .ToList();
            var settings = db.GetCollection<SettingsDocument>("settings").FindById(1);
            if (settings is not null) rows.Add(new("settings", "1", settings));
            return rows;
        });
    }

    public static void Stats(RustDatabase target, string path)
    {
        if (target.IsReady()) return;
        Import(target, path, db =>
        {
            var migrations = new StatsMigrationRunner(
                [new DateTimeToDateOnlyMigration(TimeProvider.System), new RemoveDefaultDateBucketMigration()],
                NullLogger<StatsMigrationRunner>.Instance);
            if (!migrations.RunPending(db)) throw new IOException("Legacy stats migration failed.");
            return db.GetCollection<DailyBucketDocument>(DailyBucketDocument.CollectionName).FindAll()
                .Select(bucket => new RustDatabase.Row(DailyBucketDocument.CollectionName,
                    bucket.Date.ToString("yyyy-MM-dd", CultureInfo.InvariantCulture), bucket)).ToArray();
        });
    }

    internal static bool Exists(string path)
    {
        try
        {
            File.GetAttributes(path);
            return true;
        }
        catch (FileNotFoundException) { return false; }
        catch (DirectoryNotFoundException) { return false; }
    }

    private static void Import(RustDatabase target, string path, Func<LiteDatabase, IEnumerable<RustDatabase.Row>> read)
    {
        if (!Exists(path))
        {
            target.Initialize([]);
            return;
        }

        var directory = Directory.CreateTempSubdirectory("omnispot-import-");
        try
        {
            var copy = Path.Combine(directory.FullName, Path.GetFileName(path));
            // Deny writable handles while copying the database and its recovery log.
            using (var source = File.Open(path, FileMode.Open, FileAccess.Read, FileShare.Read))
            {
                var log = Path.ChangeExtension(path, null) + "-log" + Path.GetExtension(path);
                if (Exists(log)) File.Copy(log, Path.Combine(directory.FullName, Path.GetFileName(log)));
                using var output = File.Create(copy);
                source.CopyTo(output);
            }

            var mapper = new BsonMapper { EnumAsInteger = true };
            mapper.RegisterType<DateOnly>(
                date => date.ToString("yyyy-MM-dd", CultureInfo.InvariantCulture),
                value => DateOnly.ParseExact(value.AsString, "yyyy-MM-dd", CultureInfo.InvariantCulture));
            using var legacy = new LiteDatabase(copy, mapper);
            target.Initialize(read(legacy));
        }
        finally { directory.Delete(recursive: true); }
    }
}
