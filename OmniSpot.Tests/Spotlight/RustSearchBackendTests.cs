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
        var results = await _sut.SearchIndexedAsync("notepad", 5, cancellationToken: TestContext.Current.CancellationToken);

        // Assert
        results.Should().NotBeEmpty();
        results[0].Type.Should().Be(SearchResultType.App);
        results[0].Title.Should().Be("Notepad");
    }

    [Theory]
    [InlineData(0)]
    [InlineData(-1)]
    public async Task SearchIndexedAsync_ReturnsEmpty_WhenLimitIsNotPositive(int maxResults)
    {
        var results = await _sut.SearchIndexedAsync("notepad", maxResults, cancellationToken: TestContext.Current.CancellationToken);

        results.Should().BeEmpty();
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

    [Fact]
    public async Task SearchIndexedAsync_BrowsesDirectory_WhenQueryIsPath()
    {
        // Arrange
        _sut.WarmUp();
        var windowsPath = Environment.GetFolderPath(Environment.SpecialFolder.Windows) + "\\";

        // Act
        var results = await _sut.SearchIndexedAsync(windowsPath, 10, cancellationToken: TestContext.Current.CancellationToken);

        // Assert
        results.Should().NotBeEmpty();
        results.Should().Contain(r => r.Type == SearchResultType.Folder);
    }

    [Fact]
    public void SpotlightIconService_ResolvesIcon_ForFolder()
    {
        // Arrange
        System.Windows.Media.ImageSource? resolvedIcon = null;
        var thread = new System.Threading.Thread(() =>
        {
            var iconExtractor = new OmniSpot.WindowDiscovery.IconExtractor();
            var sut = new SpotlightIconService(iconExtractor);

            // Act
            resolvedIcon = sut.GetIcon(Environment.GetFolderPath(Environment.SpecialFolder.Windows), SearchResultType.Folder);
        });
        thread.SetApartmentState(System.Threading.ApartmentState.STA);
        thread.Start();
        thread.Join();

        // Assert
        resolvedIcon.Should().NotBeNull();
    }

    [Fact]
    public void AppOverlayWindow_CanInitialize_WithoutXamlParseException()
    {
        // Arrange
        Exception? exception = null;
        var thread = new System.Threading.Thread(() =>
        {
            try
            {
                if (System.Windows.Application.Current == null)
                {
                    _ = new System.Windows.Application();
                }
                var vm = new OmniSpot.UI.ViewModels.AppOverlayViewModel();
                _ = new OmniSpot.UI.Windows.AppOverlayWindow(vm);
            }
            catch (Exception ex)
            {
                exception = ex;
            }
        });
        thread.SetApartmentState(System.Threading.ApartmentState.STA);
        thread.Start();
        thread.Join();

        // Assert
        exception.Should().BeNull();
    }
}