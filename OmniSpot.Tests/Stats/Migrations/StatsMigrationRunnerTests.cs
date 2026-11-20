using OmniSpot.Configuration.Storage;
using OmniSpot.Stats.Migrations;
using AwesomeAssertions;
using LiteDB;
using Microsoft.Extensions.Logging.Abstractions;
using Xunit;

namespace OmniSpot.Tests.Stats.Migrations;

public class StatsMigrationRunnerTests : IDisposable
{
    private readonly LiteDatabase _db = new(":memory:");

    public void Dispose() => _db.Dispose();

    private StatsMigrationRunner BuildRunner(params IStatsMigration[] migrations) =>
        new(migrations, NullLogger<StatsMigrationRunner>.Instance);

    // ── RunPending ─────────────────────────────────────────────────────────────

    [Fact]
    public void RunPending_ReturnsTrue_WhenNoMigrationsRegistered()
    {
        var sut = BuildRunner();

        var result = sut.RunPending(_db);

        result.Should().BeTrue();
    }

    [Fact]
    public void RunPending_RunsMigration_WhenNotYetExecuted()
    {
        var migration = new FakeMigration(version: 1);
        var sut = BuildRunner(migration);

        sut.RunPending(_db);

        migration.WasRun.Should().BeTrue();
    }

    [Fact]
    public void RunPending_RecordsMigrationVersion_AfterSuccessfulRun()
    {
        var migration = new FakeMigration(version: 1);
        var sut = BuildRunner(migration);
        sut.RunPending(_db);

        // Run again — migration must be skipped because version was recorded
        sut.RunPending(_db);

        migration.RunCount.Should().Be(1);
    }

    [Fact]
    public void RunPending_DoesNotRecordVersion_WhenMigrationThrows()
    {
        var failing = new FakeMigration(version: 1, throws: true);
        BuildRunner(failing).RunPending(_db);

        // Run again with a non-throwing migration of the same version —
        // it must run again because the failed run should not have recorded the version
        var retry = new FakeMigration(version: 1);
        BuildRunner(retry).RunPending(_db);

        retry.WasRun.Should().BeTrue();
    }

    [Fact]
    public void RunPending_SkipsMigration_WhenAlreadyRecorded()
    {
        // Pre-seed the migration record directly into the shared DB
        _db.GetCollection<MigrationRecord>("_migrations")
           .Insert(new MigrationRecord { Version = 1, ExecutedAt = DateTime.UtcNow });

        var migration = new FakeMigration(version: 1);
        BuildRunner(migration).RunPending(_db);

        migration.WasRun.Should().BeFalse();
    }

    [Fact]
    public void RunPending_RunsMigrationsInVersionOrder()
    {
        var order = new List<int>();
        var m3 = new FakeMigration(version: 3, onRun: () => order.Add(3));
        var m1 = new FakeMigration(version: 1, onRun: () => order.Add(1));
        var m2 = new FakeMigration(version: 2, onRun: () => order.Add(2));
        var sut = BuildRunner(m3, m1, m2);

        sut.RunPending(_db);

        order.Should().Equal(1, 2, 3);
    }

    [Fact]
    public void RunPending_ReturnsTrue_WhenAllMigrationsSucceed()
    {
        var sut = BuildRunner(new FakeMigration(1), new FakeMigration(2));

        var result = sut.RunPending(_db);

        result.Should().BeTrue();
    }

    [Fact]
    public void RunPending_ReturnsFalse_WhenMigrationThrows()
    {
        var sut = BuildRunner(new FakeMigration(version: 1, throws: true));

        var result = sut.RunPending(_db);

        result.Should().BeFalse();
    }

    [Fact]
    public void RunPending_StopsAfterFailure_DoesNotRunSubsequentMigrations()
    {
        var m1 = new FakeMigration(version: 1, throws: true);
        var m2 = new FakeMigration(version: 2);
        BuildRunner(m1, m2).RunPending(_db);

        m2.WasRun.Should().BeFalse();
    }

    // ── Fakes ──────────────────────────────────────────────────────────────────

    private sealed class FakeMigration(int version, bool throws = false, Action? onRun = null) : IStatsMigration
    {
        public int Version => version;
        public bool WasRun => RunCount > 0;
        public int RunCount { get; private set; }

        public void Up(ILiteDatabase db)
        {
            RunCount++;
            onRun?.Invoke();
            if (throws)
                throw new InvalidOperationException("Simulated migration failure");
        }
    }
}
