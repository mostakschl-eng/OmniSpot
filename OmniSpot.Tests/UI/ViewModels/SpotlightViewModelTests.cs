using AwesomeAssertions;
using NSubstitute;
using OmniSpot.Spotlight;
using OmniSpot.UI.ViewModels;
using Xunit;

namespace OmniSpot.Tests.UI.ViewModels;

public class SpotlightViewModelTests
{
    private readonly ISpotlightSearchService _searchService = Substitute.For<ISpotlightSearchService>();
    private readonly SpotlightViewModel _sut;

    public SpotlightViewModelTests()
    {
        _sut = new SpotlightViewModel(_searchService);
    }

    [Fact]
    public async Task ResetAndLoad_LoadsSuggestions()
    {
        // Arrange
        var suggestions = new List<SearchResult>
        {
            new("Notepad", "C:\\Windows\\notepad.exe", "C:\\Windows\\notepad.exe", SearchResultType.App)
        };
        _searchService.GetSuggestionsAsync(Arg.Any<CancellationToken>()).Returns(suggestions);

        // Act
        _sut.ResetAndLoad();
        await Task.Delay(50, TestContext.Current.CancellationToken); // Allow async call to complete

        // Assert
        _sut.Query.Should().BeEmpty();
        _sut.ShowSuggestions.Should().BeTrue();
        _sut.Suggestions.Should().HaveCount(1);
    }

    [Fact]
    public async Task QueryChanged_PopulatesResults_WhenNonEmpty()
    {
        // Arrange
        var results = new List<SearchResult>
        {
            new("Visual Studio", "C:\\VS.exe", "C:\\VS.exe", SearchResultType.App),
            new("Document", "C:\\Doc.txt", "C:\\Doc.txt", SearchResultType.File)
        };
        _searchService.SearchProgressivelyAsync("test", Arg.Any<Action<IReadOnlyList<SearchResult>>>(), Arg.Any<CancellationToken>())
            .Returns(call =>
            {
                call.ArgAt<Action<IReadOnlyList<SearchResult>>>(1)(results);
                return Task.CompletedTask;
            });

        // Act
        _sut.Query = "test";
        await Task.Delay(120, TestContext.Current.CancellationToken); // Debounce is 60ms

        // Assert
        _sut.ShowSuggestions.Should().BeFalse();
        _sut.Results.Should().HaveCount(2);
        _sut.SelectedIndex.Should().Be(0);
        _sut.HasResults.Should().BeTrue();
    }

    [Fact]
    public void MoveSelection_NavigatesAndWrapsAround()
    {
        // Arrange
        _sut.Results =
        [
            new("Item1", "1", "1", SearchResultType.App),
            new("Item2", "2", "2", SearchResultType.App),
            new("Item3", "3", "3", SearchResultType.App)
        ];
        _sut.ShowSuggestions = false;
        _sut.SelectedIndex = 0;

        // Act & Assert
        _sut.MoveSelection(1);
        _sut.SelectedIndex.Should().Be(1);

        _sut.MoveSelection(1);
        _sut.SelectedIndex.Should().Be(2);

        _sut.MoveSelection(1); // Wrap to 0
        _sut.SelectedIndex.Should().Be(0);

        _sut.MoveSelection(-1); // Wrap to 2
        _sut.SelectedIndex.Should().Be(2);
    }

    [Fact]
    public void OpenItem_FiresRequestClose()
    {
        // Arrange
        var closeCalled = false;
        _sut.RequestClose += () => closeCalled = true;
        var item = new SearchResult("Test", "t", "t", SearchResultType.App);

        // Act
        _sut.OpenItem(item);

        // Assert
        closeCalled.Should().BeTrue();
    }
}
