using Microsoft.Extensions.Logging;
using OmniSpot.Configuration;
using OmniSpot.Stats;
using OmniSpot.WindowDiscovery;
using System.IO;

namespace OmniSpot.Spotlight;

internal class AppSearchProvider(
    ConfigurationManager configurationManager,
    RunningApplicationsService runningApplicationsService,
    IPackagedAppsService packagedAppsService,
    IAppRegistryCache appRegistryCache,
    IconExtractor iconExtractor,
    SessionStats sessionStats,
    StatsRepository statsRepository,
    ILogger<AppSearchProvider> logger) : IAppSearchProvider
{
    private const int MaxSearchResults = 25;
    private static readonly string[] StartMenuFolders =
    [
        Environment.GetFolderPath(Environment.SpecialFolder.StartMenu),
        Environment.GetFolderPath(Environment.SpecialFolder.CommonStartMenu)
    ];

    private readonly Lazy<IReadOnlyList<(string Name, string Path)>> _startMenuShortcuts =
        new(LoadStartMenuShortcuts, LazyThreadSafetyMode.ExecutionAndPublication);
    private readonly object _catalogLock = new();
    private Task<IReadOnlyList<(SearchResult Result, string AlternateName, string? IconPath)>>? _catalog;
    private Task<IReadOnlyList<(SearchResult Result, string AlternateName, string? IconPath)>>? _refreshCatalog;
    private DateTime _catalogCreatedAt;

    public Task<IReadOnlyList<SearchResult>> GetSuggestedAppsAsync(CancellationToken cancellationToken = default)
    {
        return Task.Run(() => GetSuggestedApps(cancellationToken), cancellationToken);
    }

    private IReadOnlyList<SearchResult> GetSuggestedApps(CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        var suggestions = new List<SearchResult>();
        var seenTargets = new HashSet<string>(StringComparer.OrdinalIgnoreCase);

        var config = configurationManager.GetConfiguration();
        var configuredApps = config?.Applications ?? [];
        var usageCounts = sessionStats.GetUsageCounts();
        var historicalUsageCounts = LoadHistoricalUsageCounts();
        var recentProcessNames = sessionStats.GetRecentProcessNames();
        var recentRanks = recentProcessNames
            .Select((processName, index) => (processName, index))
            .ToDictionary(entry => entry.processName, entry => recentProcessNames.Count - entry.index,
                StringComparer.OrdinalIgnoreCase);

        foreach (var app in configuredApps
            .OrderByDescending(app => recentRanks.GetValueOrDefault(app.ProcessName))
            .ThenByDescending(app => usageCounts.GetValueOrDefault(app.ProcessName) +
                                     historicalUsageCounts.GetValueOrDefault(app.ProcessName)))
        {
            var displayName = appRegistryCache.GetDisplayName(app.ProcessName, app.ProcessPath);
            var icon = iconExtractor.GetByProcessPath(app.ProcessPath);

            if (seenTargets.Add(app.ProcessPath))
            {
                suggestions.Add(new SearchResult(
                    Title: displayName,
                    Subtitle: app.ProcessPath,
                    Target: app.ProcessPath,
                    Type: SearchResultType.App,
                    Icon: icon,
                    Score: 100));
            }
        }

        try
        {
            var running = runningApplicationsService.GetRunningApplications(
                configuredApps.Select(a => a.ProcessName));

            foreach (var app in running)
            {
                if (suggestions.Count >= 10)
                {
                    break;
                }

                if (seenTargets.Add(app.ProcessImagePath))
                {
                    suggestions.Add(new SearchResult(
                        Title: app.DisplayName,
                        Subtitle: app.ProcessImagePath,
                        Target: app.ProcessImagePath,
                        Type: SearchResultType.App,
                        Icon: app.Icon,
                        Score: 80));
                }
            }
        }
        catch (Exception ex)
        {
            logger.LogWarning(ex, "Failed to inspect running applications for suggestions");
        }

        return suggestions.Take(10).ToList();
    }

    public async Task<IReadOnlyList<SearchResult>> SearchAppsAsync(string query, CancellationToken cancellationToken = default)
    {
        if (string.IsNullOrWhiteSpace(query))
        {
            return await GetSuggestedAppsAsync(cancellationToken);
        }

        Task<IReadOnlyList<(SearchResult Result, string AlternateName, string? IconPath)>> catalog;
        lock (_catalogLock)
        {
            if (_catalog is null || _catalog.IsFaulted)
            {
                _catalog = Task.Run(BuildCatalog);
                _catalogCreatedAt = DateTime.UtcNow;
            }
            else if (DateTime.UtcNow - _catalogCreatedAt > TimeSpan.FromSeconds(15) &&
                     (_refreshCatalog is null || _refreshCatalog.IsFaulted))
            {
                _refreshCatalog = Task.Run(BuildCatalog);
                _catalogCreatedAt = DateTime.UtcNow;
            }

            if (_refreshCatalog?.IsCompletedSuccessfully == true)
            {
                _catalog = _refreshCatalog;
                _refreshCatalog = null;
            }

            catalog = _catalog;
        }

        var entries = await catalog.WaitAsync(cancellationToken);
        var q = query.Trim();
        return entries
            .Select(entry => (entry.Result, entry.IconPath, Score: CalculateScore(entry.Result.Title, entry.AlternateName, q)))
            .Where(entry => entry.Score > 0)
            .OrderByDescending(entry => entry.Score + entry.Result.Score)
            .Take(MaxSearchResults)
            .Select(entry => entry.Result with
            {
                Score = entry.Score + entry.Result.Score,
                Icon = string.Equals(entry.Result.Subtitle, entry.IconPath, StringComparison.OrdinalIgnoreCase)
                    ? iconExtractor.GetByProcessPath(entry.IconPath!)
                    : iconExtractor.GetByIconPath(entry.IconPath)
            })
            .ToList();
    }

    private IReadOnlyList<(SearchResult Result, string AlternateName, string? IconPath)> BuildCatalog()
    {
        var results = new List<(SearchResult Result, string AlternateName, string? IconPath)>();
        var seenTargets = new HashSet<string>(StringComparer.OrdinalIgnoreCase);

        // 1. Configured applications
        var config = configurationManager.GetConfiguration();
        var configuredApps = config?.Applications ?? [];

        foreach (var app in configuredApps)
        {
            var displayName = appRegistryCache.GetDisplayName(app.ProcessName, app.ProcessPath);
            if (seenTargets.Add(app.ProcessPath))
            {
                results.Add((new SearchResult(
                    Title: displayName,
                    Subtitle: app.ProcessPath,
                    Target: app.ProcessPath,
                    Type: SearchResultType.App,
                    Score: 10), app.ProcessName, app.ProcessPath));
            }
        }

        // 2. Running applications
        try
        {
            var running = runningApplicationsService.GetRunningApplications([], includeIcons: false);
            foreach (var app in running)
            {
                if (seenTargets.Add(app.ProcessImagePath))
                {
                    results.Add((new SearchResult(
                        Title: app.DisplayName,
                        Subtitle: app.ProcessImagePath,
                        Target: app.ProcessImagePath,
                        Type: SearchResultType.App,
                        Score: 0), app.ProcessName, app.ProcessImagePath));
                }
            }
        }
        catch (Exception ex)
        {
            logger.LogWarning(ex, "Failed to search running applications");
        }

        // 3. Start Menu Shortcuts (.lnk)
        try
        {
            foreach (var shortcut in _startMenuShortcuts.Value)
            {
                if (seenTargets.Add(shortcut.Path))
                {
                    results.Add((new SearchResult(
                        Title: shortcut.Name,
                        Subtitle: shortcut.Path,
                        Target: shortcut.Path,
                        Type: SearchResultType.App,
                        Score: 0), shortcut.Name, shortcut.Path));
                }
            }
        }
        catch (Exception ex)
        {
            logger.LogWarning(ex, "Failed to search Start Menu shortcuts");
        }

        // 4. Packaged Apps
        try
        {
            foreach (var package in packagedAppsService.GetSearchableApps())
            {
                if (seenTargets.Add(package.Path))
                {
                    results.Add((new SearchResult(
                        Title: package.Name,
                        Subtitle: package.Path,
                        Target: package.Aumid ?? package.Path,
                        Type: SearchResultType.App,
                        Score: 0), package.Name, package.IconPath));
                }
            }
        }
        catch (Exception ex)
        {
            logger.LogWarning(ex, "Failed to search Packaged apps");
        }

        return results;
    }

    private static IReadOnlyList<(string Name, string Path)> LoadStartMenuShortcuts()
    {
        var shortcuts = new List<(string Name, string Path)>();

        foreach (var rootFolder in StartMenuFolders)
        {
            if (!Directory.Exists(rootFolder))
            {
                continue;
            }

            try
            {
                shortcuts.AddRange(Directory.EnumerateFiles(rootFolder, "*.lnk", SearchOption.AllDirectories)
                    .Select(path => (Path.GetFileNameWithoutExtension(path), path)));
            }
            catch (UnauthorizedAccessException)
            {
                // Some Start Menu folders can be inaccessible; keep the usable entries.
            }
            catch (IOException)
            {
                // The folder can change while it is being enumerated.
            }
        }

        return shortcuts;
    }

    private IReadOnlyDictionary<string, int> LoadHistoricalUsageCounts()
    {
        var usage = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);
        foreach (var bucket in statsRepository.GetAllHistoricBuckets())
        {
            foreach (var entry in bucket.StaticAppUsage.Concat(bucket.DynamicAppUsage))
            {
                usage[entry.Key] = usage.GetValueOrDefault(entry.Key) + entry.Value.Switches;
            }
        }

        return usage;
    }

    private static int CalculateScore(string displayName, string altName, string query)
    {
        var normalizedQuery = Normalize(query);
        var displayScore = CalculateCandidateScore(displayName, normalizedQuery);
        var alternateScore = CalculateCandidateScore(altName, normalizedQuery);
        return Math.Max(displayScore, alternateScore);
    }

    private static int CalculateCandidateScore(string candidate, string normalizedQuery)
    {
        var normalizedCandidate = Normalize(candidate);
        if (normalizedCandidate.Length == 0 || normalizedQuery.Length == 0)
        {
            return 0;
        }

        if (normalizedCandidate.Equals(normalizedQuery, StringComparison.Ordinal))
        {
            return 100;
        }

        if (normalizedCandidate.StartsWith(normalizedQuery, StringComparison.Ordinal))
        {
            return 90;
        }

        if (normalizedCandidate.Contains(normalizedQuery, StringComparison.Ordinal))
        {
            return 80;
        }

        var queryTokens = normalizedQuery.Split(' ', StringSplitOptions.RemoveEmptyEntries);
        var candidateTokens = normalizedCandidate.Split(' ', StringSplitOptions.RemoveEmptyEntries);
        if (queryTokens.All(queryToken => candidateTokens.Any(candidateToken => candidateToken.StartsWith(queryToken, StringComparison.Ordinal))))
        {
            return 72;
        }

        return IsSubsequence(normalizedQuery, normalizedCandidate) ? 55 : 0;
    }

    private static bool IsSubsequence(string query, string candidate)
    {
        var queryIndex = 0;
        foreach (var character in candidate)
        {
            if (queryIndex < query.Length && query[queryIndex] == character)
            {
                queryIndex++;
            }
        }

        return queryIndex == query.Length;
    }

    private static string Normalize(string value) =>
        string.Join(' ', value.Trim().ToLowerInvariant().Split([' ', '\t', '_', '-', '.'], StringSplitOptions.RemoveEmptyEntries));
}
