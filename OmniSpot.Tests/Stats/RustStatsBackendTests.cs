using System.IO;
using AwesomeAssertions;
using Microsoft.Extensions.Logging.Abstractions;
using NSubstitute;
using OmniSpot.Configuration;
using OmniSpot.Stats;
using OmniSpot.Stats.Storage;
using OmniSpot.WindowDiscovery;
using OmniSpot;
using System.Diagnostics;
using System.Windows.Input;
using Xunit;
using AppConfig = OmniSpot.Configuration.Configuration;

namespace OmniSpot.Tests.Stats;

[Collection("Rust app registry")]
public class RustStatsBackendTests
{
    [Theory]
    [InlineData("2024-01-01T00:30:00Z", -480, "2023-12-31")]
    [InlineData("2024-02-29T23:30:00Z", 120, "2024-03-01")]
    [InlineData("2024-02-29T12:00:00Z", 0, "2024-02-29")]
    public void LocalDateKey_UsesRustCalendarAndSuppliedTimeZone(string utcText, int offsetMinutes, string expected)
    {
        var utc = DateTime.Parse(utcText, null, System.Globalization.DateTimeStyles.AdjustToUniversal);
        var zone = TimeZoneInfo.CreateCustomTimeZone("test", TimeSpan.FromMinutes(offsetMinutes), "test", "test");

        new RustStatsBackend().GetLocalDateKey(utc, zone).Should().Be(expected);
    }

    [Fact]
    public void ProcessSwitch_UpdatesRustSessionBucket_AndRoundTripsThroughFfi()
    {
        var sut = new RustStatsBackend();
        var today = new DailyBucketDocument { Date = DateOnly.FromDateTime(DateTime.Today) };
        sut.LoadToday(today);
        var now = Stopwatch.GetTimestamp();

        sut.ProcessSwitch(new SwitchEvent(
            "rust-ffi-test.exe", null, "C:\\Apps\\rust-ffi-test.exe", 2,
            now, now + Stopwatch.Frequency / 10, null, false, Key.A), "A");

        var bucket = sut.GetSnapshots().Last();
        bucket.Date.Should().Be(DateOnly.FromDateTime(DateTime.Today));
        bucket.TotalSwitches.Should().Be(1);
        bucket.StaticAppUsage["rust-ffi-test.exe"].Switches.Should().Be(1);
        bucket.FastestSwitch.Should().NotBeNull();
        bucket.FastestSwitch!.Letter.Should().Be("A");
    }

    [Fact]
    public void Calculate_ReturnsMetricsFromRustBackend()
    {
        using var temp = new TestDatabase();
        var db = temp.Database;
        var windows = Substitute.For<IWindowEnumerator>();
        windows.GetWindows().Returns([]);
        var registry = new AppRegistryCache(db, windows, NullLogger<AppRegistryCache>.Instance,
            new RustAppRegistryBackend());
        var config = new AppConfig(Key.RightCtrl, [], false, AppThemeSetting.System,
            false, 0, false, false, false, false);
        registry.Prepopulate(config);
        var today = new DailyBucketDocument { Date = DateOnly.FromDateTime(DateTime.Today), TotalSwitches = 12 };

        var metrics = new RustStatsBackend().Calculate([], today, [], registry);

        metrics.TodaySwitchCount.Should().Be(12);
        metrics.TotalSwitchCount.Should().Be(12);
    }

    [Fact]
    public void ProcessInspector_UsesRustForExecutableTypeAndDisplayName()
    {
        var path = Environment.ProcessPath!;
        var rust = new RustProcessInspectorBackend();

        rust.IsWindowsExecutable(path).Should().BeTrue();
        rust.GetProcessDisplayName(path).Should().NotBeNullOrWhiteSpace();
    }

    [Fact]
    public void NativeStorage_FlushesRestoresAndResetsTheSession()
    {
        using var temp = new TestDatabase();
        var backend = new RustStatsBackend();
        var today = DateOnly.FromDateTime(DateTime.Today);
        backend.LoadToday(new DailyBucketDocument { Date = today, TotalSwitches = 23 });

        temp.Database.FlushStats().Should().Be(1);
        backend.ClearSession();
        temp.Database.RestoreStats(today);
        backend.GetSnapshots().Should().ContainSingle().Which.TotalSwitches.Should().Be(23);

        temp.Database.ResetStats();
        backend.GetSnapshots().Last().TotalSwitches.Should().Be(0);
        temp.Database.ReadStatsHistory(today.AddDays(1)).Should().BeEmpty();
    }

    [Fact]
    public void GetHistoricBuckets_FiltersTodayAndSortsNewestFirstInRust()
    {
        var today = DateOnly.FromDateTime(DateTime.Today);
        var buckets = new[]
        {
            new DailyBucketDocument { Date = today.AddDays(-2), TotalSwitches = 2 },
            new DailyBucketDocument { Date = today, TotalSwitches = 4 },
            new DailyBucketDocument { Date = today.AddDays(-1), TotalSwitches = 3 }
        };

        var result = new RustStatsBackend().GetHistoricBuckets(buckets, today);

        result.Select(bucket => bucket.Date).Should().Equal(today.AddDays(-1), today.AddDays(-2));
    }

    [Fact]
    public void AppRegistry_UsesRustCaseInsensitiveCache()
    {
        var backend = new RustAppRegistryBackend();
        backend.Initialize([new AppRegistryDocument { ProcessName = "Sample.exe", DisplayName = "Sample App" }]);

        var result = backend.Resolve("SAMPLE.EXE", null);

        result.DisplayName.Should().Be("Sample App");
        result.WasCached.Should().BeTrue();
        result.ShouldPersist.Should().BeFalse();
    }

    [Fact]
    public void StatsService_QueuesProcessesAndDrainsEventsInRust()
    {
        var backend = new RustStatsBackend();
        var session = new SessionStats(backend);
        var changed = false;
        session.DataChanged += () => changed = true;
        backend.LoadToday(new DailyBucketDocument { Date = DateOnly.FromDateTime(DateTime.Today) });
        backend.ApplyConfiguration(true).Should().BeFalse();
        backend.IsRunning.Should().BeTrue();
        var now = Stopwatch.GetTimestamp();
        var enqueued = backend.Enqueue(new SwitchEvent(
            "queued-rust-test.exe", null, "C:\\Apps\\queued-rust-test.exe", 2,
            now, now + Stopwatch.Frequency / 20, null, false, Key.Q));
        backend.ApplyConfiguration(false).Should().BeTrue();

        enqueued.Should().BeTrue();
        session.GetSnapshots().Last().StaticAppUsage["queued-rust-test.exe"].Switches.Should().Be(1);
        changed.Should().BeTrue();
        backend.IsRunning.Should().BeFalse();
    }
    private sealed class TestDatabase : IDisposable
    {
        private readonly DirectoryInfo _directory = Directory.CreateTempSubdirectory("omnispot-stats-test-");
        public RustDatabase Database { get; }

        public TestDatabase()
        {
            Database = new RustDatabase(Path.Combine(_directory.FullName, "settings.sqlite3"));
            Database.Initialize([]);
        }

        public void Dispose() => _directory.Delete(recursive: true);
    }
}
