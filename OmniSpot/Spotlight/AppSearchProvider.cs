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

    public Task<IReadOnlyList<SearchResult>> SearchAppsAsync(string query, CancellationToken cancellationToken = default)
    {
        return Task.Run(() => SearchApps(query, cancellationToken), cancellationToken);
    }

    private IReadOnlyList<SearchResult> SearchApps(string query, CancellationToken cancellationToken)
    {
        if (string.IsNullOrWhiteSpace(query))
        {
            return GetSuggestedApps(cancellationToken);
        }

        cancellationToken.ThrowIfCancellationRequested();
        var results = new List<SearchResult>();
        var seenTargets = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        var q = query.Trim();

        // 1. Configured applications
        var config = configurationManager.GetConfiguration();
        var configuredApps = config?.Applications ?? [];

        foreach (var app in configuredApps)
        {
            cancellationToken.ThrowIfCancellationRequested();
            var displayName = appRegistryCache.GetDisplayName(app.ProcessName, app.ProcessPath);
            var score = CalculateScore(displayName, app.ProcessName, q);
            if (score > 0 && seenTargets.Add(app.ProcessPath))
            {
                var icon = iconExtractor.GetByProcessPath(app.ProcessPath);
                results.Add(new SearchResult(
                    Title: displayName,
                    Subtitle: app.ProcessPath,
                    Target: app.ProcessPath,
                    Type: SearchResultType.App,
                    Icon: icon,
                    Score: score + 10)); // Boost configured apps
            }
        }

        // 2. Running applications
        try
        {
            var running = runningApplicationsService.GetRunningApplications([]);
            foreach (var app in running)
            {
                cancellationToken.ThrowIfCancellationRequested();
                var score = CalculateScore(app.DisplayName, app.ProcessName, q);
                if (score > 0 && seenTargets.Add(app.ProcessImagePath))
                {
                    results.Add(new SearchResult(
                        Title: app.DisplayName,
                        Subtitle: app.ProcessImagePath,
                        Target: app.ProcessImagePath,
                        Type: SearchResultType.App,
                        Icon: app.Icon,
                        Score: score));
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
                cancellationToken.ThrowIfCancellationRequested();
                var score = CalculateScore(shortcut.Name, shortcut.Name, q);
                if (score > 0 && seenTargets.Add(shortcut.Path))
                {
                    var icon = iconExtractor.GetByProcessPath(shortcut.Path);
                    results.Add(new SearchResult(
                        Title: shortcut.Name,
                        Subtitle: shortcut.Path,
                        Target: shortcut.Path,
                        Type: SearchResultType.App,
                        Icon: icon,
                        Score: score));
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
            var packages = packagedAppsService.GetInstalledPackages();
            foreach (var path in packages.Paths)
            {
                cancellationToken.ThrowIfCancellationRequested();
                var displayName = packagedAppsService.GetDisplayName(path);
                if (!string.IsNullOrWhiteSpace(displayName))
                {
                    var score = CalculateScore(displayName, displayName, q);
                    if (score > 0 && seenTargets.Add(path))
                    {
                        var info = packagedAppsService.GetByInstalledPath(path, null);
                        var icon = iconExtractor.GetByIconPath(info?.IconPath);
                        results.Add(new SearchResult(
                            Title: displayName,
                            Subtitle: path,
                            Target: info?.Aumid ?? path,
                            Type: SearchResultType.App,
                            Icon: icon,
                            Score: score));
                    }
                }
            }
        }
        catch (Exception ex)
        {
            logger.LogWarning(ex, "Failed to search Packaged apps");
        }

        var ordered = results
            .OrderByDescending(r => r.Score)
            .Take(MaxSearchResults)
            .ToList();
        return ordered;
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
