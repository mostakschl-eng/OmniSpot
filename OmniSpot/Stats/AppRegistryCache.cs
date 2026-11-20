using Microsoft.Extensions.Logging;
using OmniSpot.Extensions;
using OmniSpot.Stats.Storage;
using OmniSpot.WindowDiscovery;

namespace OmniSpot.Stats;

/// <summary>Persistence adapter for Rust-owned app display-name caching and resolution.</summary>
internal sealed class AppRegistryCache(
    RustDatabase database,
    IWindowEnumerator windowEnumerator,
    ILogger<AppRegistryCache> logger,
    RustAppRegistryBackend rustAppRegistryBackend) : IAppRegistryCache
{
    public void Prepopulate(Configuration.Configuration config)
    {
        using var _ = logger.MeasureTime("Prepopulating app registry entries");
        var windows = windowEnumerator.GetWindows();
        rustAppRegistryBackend.Initialize(database.Read<AppRegistryDocument>(AppRegistryDocument.CollectionName));

        foreach (var window in windows)
        {
            ResolveAndPersist(window.ProcessName, window.ProcessImagePath);
        }
        foreach (var app in config.Applications)
        {
            ResolveAndPersist(app.ProcessName, app.ProcessPath);
        }
    }

    public bool TryAdd(string processName, string processPath)
    {
        var result = rustAppRegistryBackend.Resolve(processName, processPath);
        if (result.ShouldPersist) PersistToRegistry(processName, result.DisplayName);
        return !result.WasCached;
    }

    public string GetDisplayName(string processName, string? processPath = null)
    {
        var result = rustAppRegistryBackend.Resolve(processName, processPath);
        if (result.ShouldPersist) PersistToRegistry(processName, result.DisplayName);
        return result.DisplayName;
    }

    internal void PersistNativeEntries()
    {
        database.Write(rustAppRegistryBackend.GetEntries().Select(entry =>
            new RustDatabase.Row(AppRegistryDocument.CollectionName, entry.ProcessName, entry)), overwrite: false);
    }

    private void PersistToRegistry(string processName, string displayName)
    {
        try
        {
            var entry = new AppRegistryDocument { ProcessName = processName, DisplayName = displayName };
            database.Write([new(AppRegistryDocument.CollectionName, processName, entry)], overwrite: false);
        }
        catch (Exception ex)
        {
            logger.LogError(ex, "Failed to persist app registry entry for {ProcessName}", processName);
        }
    }

    private void ResolveAndPersist(string processName, string processPath)
    {
        var result = rustAppRegistryBackend.Resolve(processName, processPath);
        if (result.ShouldPersist) PersistToRegistry(processName, result.DisplayName);
    }
}
