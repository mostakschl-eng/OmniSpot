namespace OmniSpot.Spotlight;

internal interface ISpotlightSearchService
{
    Task<IReadOnlyList<SearchResult>> SearchAsync(string query, CancellationToken cancellationToken = default);
    Task<IReadOnlyList<SearchResult>> GetSuggestionsAsync(CancellationToken cancellationToken = default);
}
