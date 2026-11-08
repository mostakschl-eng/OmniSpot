namespace OmniSpot.Spotlight;

internal sealed class RustSpotlightSearchService(
    RustSearchBackend backend,
    SpotlightIconService iconService) : ISpotlightSearchService
{
    public async Task<IReadOnlyList<SearchResult>> SearchAsync(
        string query,
        CancellationToken cancellationToken = default)
    {
        var results = await backend.SearchIndexedAsync(query.Trim(), 8, null, cancellationToken);
        return await Task.Run(() => EnrichWithIcons(results), cancellationToken);
    }

    public async Task<IReadOnlyList<SearchResult>> GetSuggestionsAsync(
        CancellationToken cancellationToken = default)
    {
        var results = await backend.SearchIndexedAsync(string.Empty, 8, SearchResultType.App, cancellationToken);
        return await Task.Run(() => EnrichWithIcons(results), cancellationToken);
    }

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

    private IReadOnlyList<SearchResult> EnrichWithIcons(IReadOnlyList<SearchResult> results)
    {
        if (results.Count == 0)
        {
            return results;
        }

        var list = new List<SearchResult>(results.Count);
        foreach (var result in results)
        {
            var icon = iconService.GetIcon(result.Target, result.Type);
            list.Add(result with { Icon = icon });
        }
        return list;
    }
}