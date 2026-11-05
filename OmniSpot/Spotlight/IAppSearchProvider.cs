namespace OmniSpot.Spotlight;

internal interface IAppSearchProvider
{
    void WarmUp();
    Task<IReadOnlyList<SearchResult>> SearchAppsAsync(string query, CancellationToken cancellationToken = default);
    Task<IReadOnlyList<SearchResult>> GetSuggestedAppsAsync(CancellationToken cancellationToken = default);
}