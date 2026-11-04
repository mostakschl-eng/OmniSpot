using AwesomeAssertions;
using Microsoft.Extensions.Logging.Abstractions;
using OmniSpot.Spotlight;
using OmniSpot.WindowDiscovery;
using Xunit;

namespace OmniSpot.Tests.Spotlight;

public class WindowsSearchFileProviderTests
{
    private readonly WindowsSearchFileProvider _sut;

    public WindowsSearchFileProviderTests()
    {
        var iconExtractor = new IconExtractor();
        _sut = new WindowsSearchFileProvider(iconExtractor, NullLogger<WindowsSearchFileProvider>.Instance);
    }

    [Fact]
    public async Task SearchFilesAsync_ReturnsEmpty_WhenQueryIsWhitespace()
    {
        // Act
        var results = await _sut.SearchFilesAsync("   ", 10, TestContext.Current.CancellationToken);

        // Assert
        results.Should().BeEmpty();
    }

    [Fact]
    public async Task SearchFilesAsync_ThrowsOrCancels_WhenTokenIsCancelled()
    {
        // Arrange
        using var cts = new CancellationTokenSource();
        cts.Cancel();

        // Act & Assert
        await Assert.ThrowsAnyAsync<OperationCanceledException>(() =>
            _sut.SearchFilesAsync("test", 10, cts.Token));
    }
}

