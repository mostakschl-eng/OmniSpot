using AwesomeAssertions;
using Microsoft.Extensions.Logging.Abstractions;
using NSubstitute;
using NSubstitute.ExceptionExtensions;
using OmniSpot.Spotlight;
using Xunit;

namespace OmniSpot.Tests.Spotlight;

public class SpotlightSearchServiceTests
{
    private readonly IAppSearchProvider _appProvider = Substitute.For<IAppSearchProvider>();
    private readonly IFileSearchProvider _fileProvider = Substitute.For<IFileSearchProvider>();
    private readonly SpotlightSearchService _sut;

    public SpotlightSearchServiceTests()
    {
        _sut = new SpotlightSearchService(
            _appProvider,
            _fileProvider,
            NullLogger<SpotlightSearchService>.Instance);
    }

    [Fact]
    public async Task SearchAsync_ReturnsSuggestions_WhenQueryIsEmptyOrWhitespace()
    {
        // Arrange
        var ct = TestContext.Current.CancellationToken;
        var suggestions = new List<SearchResult>
        {
            new("Notepad", "C:\\Windows\\notepad.exe", "C:\\Windows\\notepad.exe", SearchResultType.App),
            new("Calculator", "calc.exe", "calc.exe", SearchResultType.App)
        };
        _appProvider.GetSuggestedAppsAsync(Arg.Any<CancellationToken>()).Returns(suggestions);

        // Act
        var resultEmpty = await _sut.SearchAsync(string.Empty, ct);
        var resultWhitespace = await _sut.SearchAsync("   ", ct);

        // Assert
        resultEmpty.Should().BeEquivalentTo(suggestions);
        resultWhitespace.Should().BeEquivalentTo(suggestions);
        _ = _fileProvider.DidNotReceiveWithAnyArgs().SearchFilesAsync(default!, default, Arg.Any<CancellationToken>());
    }

    [Fact]
    public async Task SearchAsync_CombinesAppsAndFiles_WhenQueryProvided()
    {
        // Arrange
        var ct = TestContext.Current.CancellationToken;
        var appResults = new List<SearchResult>
        {
            new("Visual Studio Code", "C:\\Program Files\\VSCode\\Code.exe", "C:\\Program Files\\VSCode\\Code.exe", SearchResultType.App, Score: 90)
        };
        var fileResults = new List<SearchResult>
        {
            new("CodeConventions.md", "D:\\OmniSpot\\.agents", "D:\\OmniSpot\\.agents\\CodeConventions.md", SearchResultType.File, Score: 80)
        };

        _appProvider.SearchAppsAsync("code", Arg.Any<CancellationToken>()).Returns(appResults);
        _fileProvider.SearchFilesAsync("code", Arg.Any<int>(), Arg.Any<CancellationToken>()).Returns(fileResults);

        // Act
        var results = await _sut.SearchAsync("code", ct);

        // Assert
        results.Should().HaveCount(2);
        results[0].Title.Should().Be("Visual Studio Code");
        results[1].Title.Should().Be("CodeConventions.md");
    }

    [Fact]
    public async Task SearchAsync_ReturnsAppResults_WhenFileProviderFails()
    {
        // Arrange
        var ct = TestContext.Current.CancellationToken;
        var appResults = new List<SearchResult>
        {
            new("Spotify", "spotify.exe", "spotify.exe", SearchResultType.App)
        };
        _appProvider.SearchAppsAsync("spot", Arg.Any<CancellationToken>()).Returns(appResults);
        _fileProvider.SearchFilesAsync("spot", Arg.Any<int>(), Arg.Any<CancellationToken>())
            .ThrowsAsync(new InvalidOperationException("Windows Search service is stopped"));

        // Act
        var results = await _sut.SearchAsync("spot", ct);

        // Assert
        results.Should().BeEquivalentTo(appResults);
    }

    [Fact]
    public async Task SearchAsync_DeduplicatesResults_WhenSameTargetReturned()
    {
        // Arrange
        var ct = TestContext.Current.CancellationToken;
        var appResults = new List<SearchResult>
        {
            new("Editor", "C:\\Tools\\Editor.exe", "C:\\Tools\\Editor.exe", SearchResultType.App, Score: 100)
        };
        var fileResults = new List<SearchResult>
        {
            new("Editor", "C:\\Tools\\Editor.exe", "C:\\Tools\\Editor.exe", SearchResultType.File, Score: 50)
        };
        _appProvider.SearchAppsAsync("edit", Arg.Any<CancellationToken>()).Returns(appResults);
        _fileProvider.SearchFilesAsync("edit", Arg.Any<int>(), Arg.Any<CancellationToken>()).Returns(fileResults);

        // Act
        var results = await _sut.SearchAsync("edit", ct);

        // Assert
        results.Should().HaveCount(1);
        results[0].Type.Should().Be(SearchResultType.App);
    }

    [Fact]
    public async Task SearchProgressivelyAsync_ShowsApps_BeforeFilesFinish()
    {
        // Arrange
        var pendingFiles = new TaskCompletionSource<IReadOnlyList<SearchResult>>();
        var app = new SearchResult("Editor", "editor.exe", "editor.exe", SearchResultType.App);
        var file = new SearchResult("Document", "doc.txt", "doc.txt", SearchResultType.File);
        var updates = new List<IReadOnlyList<SearchResult>>();
        _appProvider.SearchAppsAsync("edit", Arg.Any<CancellationToken>()).Returns([app]);
        _fileProvider.SearchFilesAsync("edit", Arg.Any<int>(), Arg.Any<CancellationToken>()).Returns(pendingFiles.Task);

        // Act
        var search = _sut.SearchProgressivelyAsync("edit", updates.Add, TestContext.Current.CancellationToken);
        await Task.Delay(20, TestContext.Current.CancellationToken);

        // Assert
        updates.Should().ContainSingle().Which.Should().ContainSingle().Which.Should().Be(app);

        pendingFiles.SetResult([file]);
        await search;
        updates.Last().Should().HaveCount(2);
    }
}
