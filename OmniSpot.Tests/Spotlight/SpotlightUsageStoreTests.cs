using AwesomeAssertions;
using LiteDB;
using Microsoft.Extensions.Logging.Abstractions;
using OmniSpot.Spotlight;
using Xunit;

namespace OmniSpot.Tests.Spotlight;

public class SpotlightUsageStoreTests
{
    [Fact]
    public void RecordLaunch_PersistsRankingAcrossInstances()
    {
        // Arrange
        using var db = new LiteDatabase(":memory:");
        var first = new SpotlightUsageStore(db, NullLogger<SpotlightUsageStore>.Instance);

        // Act
        first.RecordLaunch("control.exe");
        first.RecordLaunch("CONTROL.EXE");
        var second = new SpotlightUsageStore(db, NullLogger<SpotlightUsageStore>.Instance);

        // Assert
        second.GetBoost("control.exe").Should().Be(4);
        second.GetFrequentTargets(1).Should().ContainSingle().Which.Should().Be("CONTROL.EXE");
    }
}