using Microsoft.Extensions.Logging;
using OmniSpot.WindowDiscovery;
using System.Data.OleDb;

namespace OmniSpot.Spotlight;

internal class WindowsSearchFileProvider(
    IconExtractor iconExtractor,
    ILogger<WindowsSearchFileProvider> logger) : IFileSearchProvider
{
    private const string ConnectionString = "Provider=Search.CollatorDSO;Extended Properties='Application=Windows';";

    public async Task<IReadOnlyList<SearchResult>> SearchFilesAsync(
        string query,
        int maxResults = 15,
        CancellationToken cancellationToken = default)
    {
        if (string.IsNullOrWhiteSpace(query))
        {
            return [];
        }

        cancellationToken.ThrowIfCancellationRequested();

        return await Task.Run(() =>
        {
            var results = new List<SearchResult>();
            var terms = query.Trim()
                .Split([' ', '\t'], StringSplitOptions.RemoveEmptyEntries)
                .Select(term => term.Replace("'", "''"))
                .Where(term => term.Length > 0)
                .ToArray();

            if (terms.Length == 0)
            {
                return (IReadOnlyList<SearchResult>)results;
            }

            var nameConditions = terms.Select(term =>
                $"(System.ItemName LIKE '%{term}%' OR CONTAINS(System.ItemName, '\"{term}*\"'))");

            var sql = $"SELECT TOP {maxResults} System.ItemName, System.ItemUrl, System.ItemPathDisplay " +
                      $"FROM SystemIndex " +
                      $"WHERE SCOPE='file:' AND {string.Join(" AND ", nameConditions)} " +
                      "AND System.ItemName NOT LIKE '%.dll' AND System.ItemName NOT LIKE '%.exe' " +
                      "AND System.ItemName NOT LIKE '%.sys' AND System.ItemName NOT LIKE '%.pdb' " +
                      "ORDER BY System.Search.Rank DESC";

            try
            {
                using var connection = new OleDbConnection(ConnectionString);
                connection.Open();

                using var command = new OleDbCommand(sql, connection);
                using var reader = command.ExecuteReader();

                while (reader.Read())
                {
                    cancellationToken.ThrowIfCancellationRequested();

                    var name = reader["System.ItemName"]?.ToString() ?? string.Empty;
                    var path = reader["System.ItemPathDisplay"]?.ToString();

                    if (string.IsNullOrEmpty(path))
                    {
                        var url = reader["System.ItemUrl"]?.ToString() ?? string.Empty;
                        path = url.StartsWith("file:", StringComparison.OrdinalIgnoreCase)
                            ? Uri.UnescapeDataString(url[5..]).Replace('/', '\\')
                            : url;
                    }

                    if (!string.IsNullOrWhiteSpace(name) && !string.IsNullOrWhiteSpace(path))
                    {
                        var icon = iconExtractor.GetByProcessPath(path);
                        results.Add(new SearchResult(
                            Title: name,
                            Subtitle: path,
                            Target: path,
                            Type: SearchResultType.File,
                            Icon: icon,
                            Score: 50));
                    }
                }
            }
            catch (OperationCanceledException)
            {
                throw;
            }
            catch (Exception ex)
            {
                logger.LogWarning(ex, "Failed to query Windows Search for query '{Query}'", query);
            }

            return (IReadOnlyList<SearchResult>)results;
        }, cancellationToken);
    }
}
