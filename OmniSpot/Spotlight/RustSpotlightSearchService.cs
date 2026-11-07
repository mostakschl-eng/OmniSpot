namespace OmniSpot.Spotlight;

internal sealed class RustSpotlightSearchService(
    RustSearchBackend backend) : ISpotlightSearchService
{
    public Task<IReadOnlyList<SearchResult>> SearchAsync(
        string query,
        CancellationToken cancellationToken = default) =>
        backend.SearchIndexedAsync(query.Trim(), 25, null, cancellationToken);

    public Task<IReadOnlyList<SearchResult>> GetSuggestionsAsync(
        CancellationToken cancellationToken = default) =>
        backend.SearchIndexedAsync(string.Empty, 10, SearchResultType.App, cancellationToken);

    public async Task SearchProgressivelyAsync(
        string query,
        Action<IReadOnlyList<SearchResult>> onResults,
        CancellationToken cancellationToken = default)
    {
        var results = await SearchAsync(query, cancellationToken);
        cancellationToken.ThrowIfCancellationRequested();
        onResults(results);
    }

    public void RecordLaunch(SearchResult result)
    {
        if (result.Type == SearchResultType.App)
        {
            _ = backend.RecordLaunchAsync(result.Target);
        }
    }
}
