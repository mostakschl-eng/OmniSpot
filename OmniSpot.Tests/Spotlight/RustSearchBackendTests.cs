using AwesomeAssertions;
using OmniSpot.Spotlight;
using Xunit;

namespace OmniSpot.Tests.Spotlight;

public class RustSearchBackendTests
{
    private readonly RustSearchBackend _sut = new();

    [Fact]
    public async Task SearchIndexedAsync_ReturnsApps_WhenSearchingForKnownApp()
    {
        // Arrange
        _sut.WarmUp();

        // Act
        var allResults = await _sut.SearchIndexedAsync("notepad", 10, cancellationToken: TestContext.Current.CancellationToken);
        var results = await _sut.SearchIndexedAsync("notepad", 10, SearchResultType.App, cancellationToken: TestContext.Current.CancellationToken);

        // Assert
        results.Should().NotBeEmpty($"because searching for 'notepad' should match Notepad app. All results found: {string.Join(", ", allResults.Select(r => $"{r.Title} ({r.Type}:{r.Score})"))}");
        results[0].Type.Should().Be(SearchResultType.App);
        results.Should().Contain(r => r.Title.Contains("Notepad", StringComparison.OrdinalIgnoreCase));
    }

    [Fact]
    public async Task SearchIndexedAsync_RanksAppAboveGenericFiles_WhenAppExists()
    {
        // Arrange
        _sut.WarmUp();

        // Act
        var results = await _sut.SearchIndexedAsync("chatgpt", 5, cancellationToken: TestContext.Current.CancellationToken);

        // Assert
        results.Should().NotBeEmpty();
        results[0].Type.Should().Be(SearchResultType.App);
        results[0].Title.Should().Be("ChatGPT");
    }

    [Fact]
    public void SpotlightIconService_ResolvesIcon_ForChatGPT()
    {
        // Arrange
        System.Windows.Media.ImageSource? resolvedIcon = null;
        var thread = new System.Threading.Thread(() =>
        {
            var iconExtractor = new OmniSpot.WindowDiscovery.IconExtractor();
            var sut = new SpotlightIconService(iconExtractor);

            // Act
            resolvedIcon = sut.GetIcon("OpenAI.Codex_2p2nqsd0c76g0!App", SearchResultType.App);
        });
        thread.SetApartmentState(System.Threading.ApartmentState.STA);
        thread.Start();
        thread.Join();

        // Assert
        resolvedIcon.Should().NotBeNull();
    }
}

