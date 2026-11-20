using Microsoft.Extensions.Logging;

namespace OmniSpot.WindowDiscovery;

/// <summary>Managed logging and DTO adapter for the native Rust window enumerator.</summary>
internal sealed class WindowEnumerator(ILogger<WindowEnumerator> logger) : IWindowEnumerator
{
    public List<ApplicationWindow> GetWindows() => RustWindowDiscoveryBackend.GetWindows();
    public List<ApplicationWindow> GetCachedWindows() => RustWindowDiscoveryBackend.GetCachedWindows();
    public ApplicationWindow? GetCurrentWindow() => RustWindowDiscoveryBackend.GetCurrentWindow();
    public List<ApplicationWindow> GetAllWindows() => RustWindowDiscoveryBackend.GetAllWindows();

    private void LogWindows(LogLevel level, IEnumerable<ApplicationWindow> result)
    {
        if (!logger.IsEnabled(level)) return;
        foreach (var item in result)
        {
            logger.Log(level,
                "PID/Handle: {ProcessId}/{Handle}, {ProcessName} ({ProductName}), {Title}, {State}, {WindowStyle}, {WindowStyleEx}, {IsCloaked}",
                item.ProcessId, item.Handle, item.ProcessImagePath, item.GetProductName(), item.Title, item.State,
                WindowStyleHelpers.GetString(item.Style), WindowStyleHelpers.GetString(item.StyleEx),
                item.IsCloaked ? "Cloaked" : "Visible");
        }
    }

    public void LogAllWindows()
    {
        var windows = GetAllWindows();
        logger.LogInformation("Found {WindowCount} windows", windows.Count);
        LogWindows(LogLevel.Information, windows);
    }
}
