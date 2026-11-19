using LiteDB;
using Microsoft.Extensions.Logging;
using OmniSpot.Extensions;
using OmniSpot.Stats.Storage;
using OmniSpot.WindowDiscovery;

namespace OmniSpot.Stats;

/// <summary>LiteDB persistence adapter for Rust-owned app display-name caching and resolution.</summary>
internal sealed class AppRegistryCache(
    LiteDatabase database,
    IWindowEnumerator windowEnumerator,
    ILogger<AppRegistryCache> logger,
    RustAppRegistryBackend rustAppRegistryBackend) : IAppRegistryCache
{
    public void Prepopulate(Configuration.Configuration config)
    {
        using var _ = logger.MeasureTime("Prepopulating app registry entries");
        var windows = windowEnumerator.GetWindows();
        var collection = database.GetCollection<AppRegistryDocument>(AppRegistryDocument.CollectionName);
        rustAppRegistryBackend.Initialize(collection.FindAll().ToList());

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
        foreach (var entry in rustAppRegistryBackend.GetEntries())
        {
            PersistToRegistry(entry.ProcessName, entry.DisplayName);
        }
    }

    private void PersistToRegistry(string processName, string displayName)
    {
        try
        {
            var collection = database.GetCollection<AppRegistryDocument>(AppRegistryDocument.CollectionName);
            if (collection.FindById(processName) is null)
            {
                collection.Insert(new AppRegistryDocument
                {
                    ProcessName = processName,
                    DisplayName = displayName
                });
            }
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
