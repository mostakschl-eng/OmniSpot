namespace OmniSpot.Spotlight;

internal interface IFileSearchProvider
{
    Task<IReadOnlyList<SearchResult>> SearchFilesAsync(string query, int maxResults = 15, CancellationToken cancellationToken = default);
}
