using Microsoft.Extensions.Logging;

namespace OmniSpot.Spotlight;

internal class SpotlightSearchService(
    IAppSearchProvider appSearchProvider,
    IFileSearchProvider fileSearchProvider,
    ISpotlightUsageStore usageStore,
    ILogger<SpotlightSearchService> logger) : ISpotlightSearchService
{
    public void RecordLaunch(SearchResult result)
    {
        if (result.Type == SearchResultType.App)
        {
            usageStore.RecordLaunch(result.Target);
        }
    }
    public async Task<IReadOnlyList<SearchResult>> GetSuggestionsAsync(CancellationToken cancellationToken = default)
    {
        return await appSearchProvider.GetSuggestedAppsAsync(cancellationToken);
    }

    public async Task SearchProgressivelyAsync(string query, Action<IReadOnlyList<SearchResult>> onResults,
        CancellationToken cancellationToken = default)
    {
        var appTask = SearchAppsSafelyAsync(query.Trim(), cancellationToken);
        var fileTask = SearchFilesSafelyAsync(query.Trim(), cancellationToken);
        var apps = await appTask;
        cancellationToken.ThrowIfCancellationRequested();
        onResults(apps);

        var files = await fileTask;
        cancellationToken.ThrowIfCancellationRequested();
        if (files.Count == 0)
        {
            return;
        }

        var seen = apps.Select(app => app.Target).ToHashSet(StringComparer.OrdinalIgnoreCase);
        onResults([.. apps, .. files.Where(file => seen.Add(file.Target))]);
    }

    public async Task<IReadOnlyList<SearchResult>> SearchAsync(string query, CancellationToken cancellationToken = default)
    {
        if (string.IsNullOrWhiteSpace(query))
        {
            return await GetSuggestionsAsync(cancellationToken);
        }

        var trimmedQuery = query.Trim();
        var appTask = SearchAppsSafelyAsync(trimmedQuery, cancellationToken);
        var fileTask = SearchFilesSafelyAsync(trimmedQuery, cancellationToken);

        await Task.WhenAll(appTask, fileTask);

        var apps = await appTask;
        var files = await fileTask;

        var seenTargets = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        var combined = new List<SearchResult>(apps.Count + files.Count);

        foreach (var app in apps)
        {
            if (seenTargets.Add(app.Target))
            {
                combined.Add(app);
            }
        }

        foreach (var file in files)
        {
            if (seenTargets.Add(file.Target))
            {
                combined.Add(file);
            }
        }

        return combined;
    }

    private async Task<IReadOnlyList<SearchResult>> SearchAppsSafelyAsync(string query, CancellationToken cancellationToken)
    {
        try
        {
            return await appSearchProvider.SearchAppsAsync(query, cancellationToken);
        }
        catch (OperationCanceledException)
        {
            throw;
        }
        catch (Exception ex)
        {
            logger.LogWarning(ex, "Application search failed for query '{Query}'", query);
            return [];
        }
    }

    private async Task<IReadOnlyList<SearchResult>> SearchFilesSafelyAsync(string query, CancellationToken cancellationToken)
    {
        if (query.Length < 2)
        {
            return [];
        }

        try
        {
            return await fileSearchProvider.SearchFilesAsync(query, 15, cancellationToken);
        }
        catch (OperationCanceledException)
        {
            throw;
        }
        catch (Exception ex)
        {
            logger.LogWarning(ex, "Windows Search query failed for query '{Query}'", query);
            return [];
        }
    }
}