using System.IO;
using System.Windows.Input;
using AwesomeAssertions;
using LiteDB;
using NSubstitute;
using OmniSpot.Configuration;
using OmniSpot.Configuration.Storage;
using OmniSpot.Stats.Storage;
using OmniSpot.WindowDiscovery;
using Xunit;

namespace OmniSpot.Tests.Configuration;

public sealed class DatabaseUpgradeTests : IDisposable
{
    private readonly DirectoryInfo _directory = Directory.CreateTempSubdirectory("omnispot-upgrade-test-");

    [Fact]
    public void SettingsUpgrade_ImportsExistingDataOnce_AndPreservesOriginal()
    {
        var legacyPath = Path.Combine(_directory.FullName, "settings.db");
        var targetPath = Path.Combine(_directory.FullName, "settings.sqlite3");
        var config = RustConfigurationBackend.GetDefaults() with
        {
            StatsEnabled = true,
            Applications = [new ApplicationConfiguration(Key.N, @"C:\Windows\notepad.exe", CycleMode.Hide, false)],
            SpotlightCustomDirectories = [@"C:\Research\Résumé"],
        };
        using (var legacy = new LiteDatabase(legacyPath))
        {
            legacy.GetCollection<SettingsDocument>("settings")
                .Insert(SettingsDocument.FromConfiguration(config, 1));
            legacy.GetCollection<AppRegistryDocument>(AppRegistryDocument.CollectionName)
                .Insert(new AppRegistryDocument { ProcessName = "example.exe", DisplayName = "Example" });
        }
        var original = File.ReadAllBytes(legacyPath);
        var packages = Substitute.For<IPackagedAppsService>();
        var target = new RustDatabase(targetPath);

        LegacyDatabaseImport.Settings(target, legacyPath, packages);
        target.LoadSettings().Configuration.Should().BeEquivalentTo(config);
        target.Read<AppRegistryDocument>(AppRegistryDocument.CollectionName).Should()
            .ContainSingle().Which.DisplayName.Should().Be("Example");
        target.SaveSettings(config with { StatsEnabled = false });

        var reopened = new RustDatabase(targetPath);
        LegacyDatabaseImport.Settings(reopened, legacyPath, packages);
        reopened.LoadSettings(sanitize: false).Configuration.StatsEnabled.Should().BeFalse();
        File.ReadAllBytes(legacyPath).Should().Equal(original);
    }

    [Fact]
    public void FailedInitialization_RollsBackAllRows_AndCanRetry()
    {
        var target = new RustDatabase(Path.Combine(_directory.FullName, "settings.sqlite3"));
        Action invalid = () => target.Initialize([new("settings", "1", 1), new("settings", "1", 2)]);
        invalid.Should().Throw<IOException>();
        target.IsReady().Should().BeFalse();

        target.Initialize([new("settings", "1", 3)]);
        target.Read<int>("settings").Should().Equal(3);
    }

    [Fact]
    public void StatsUpgrade_ConvertsLegacyDates_AndResetSurvivesReopen()
    {
        var provider = new StatsDbProvider(_directory.FullName);
        provider.OpenExisting().Should().BeNull();
        File.Exists(Path.Combine(_directory.FullName, "stats.sqlite3")).Should().BeFalse();

        var legacyPath = Path.Combine(_directory.FullName, "stats.db");
        var local = new DateTime(2025, 9, 8, 12, 0, 0, DateTimeKind.Local);
        const string collection = DailyBucketDocument.CollectionName;
        using (var legacy = new LiteDatabase(legacyPath))
        {
            legacy.GetCollection(collection).Insert(new BsonDocument
            {
                ["_id"] = local,
                ["TotalSwitches"] = 42,
            });
            legacy.GetCollection(collection).Insert(new BsonDocument { ["_id"] = "0001-01-01" });
        }
        var original = File.ReadAllBytes(legacyPath);

        var target = provider.OpenExisting()!;
        var bucket = target.Read<DailyBucketDocument>(collection).Should().ContainSingle().Which;
        bucket.Date.Should().Be(new DateOnly(2025, 9, 8));
        bucket.TotalSwitches.Should().Be(42);
        target.ReadStatsHistory(new DateOnly(2025, 9, 8)).Should().BeEmpty();
        target.ReadStatsHistory(new DateOnly(2025, 9, 9)).Should().ContainSingle();
        target.Clear(collection);

        new StatsDbProvider(_directory.FullName).OpenExisting()!
            .Read<DailyBucketDocument>(collection).Should().BeEmpty();
        File.ReadAllBytes(legacyPath).Should().Equal(original);
    }

    public void Dispose() => _directory.Delete(recursive: true);
}
