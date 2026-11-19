using Microsoft.Extensions.Logging;
using OmniSpot.Configuration;
using Windows.Win32.Foundation;

namespace OmniSpot.WindowDiscovery;

/// <summary>Managed logging and DTO adapter for the native Rust window enumerator.</summary>
internal sealed class WindowEnumerator(ILogger<WindowEnumerator> logger) : IWindowEnumerator
{
    public List<ApplicationWindow> GetWindows() => RustWindowDiscoveryBackend.GetWindows();
    public List<ApplicationWindow> GetCachedWindows() => RustWindowDiscoveryBackend.GetCachedWindows();
    public ApplicationWindow? GetCurrentWindow() => RustWindowDiscoveryBackend.GetCurrentWindow();
    public FocusedAppWindows GetFocusedAppWindows(
        IReadOnlyList<ApplicationWindow> allWindows,
        IReadOnlyList<ApplicationConfiguration> applications) =>
        RustWindowDiscoveryBackend.GetFocusedWindows(allWindows, applications);
    public List<ApplicationWindow> GetAllWindows() => RustWindowDiscoveryBackend.GetAllWindows();
    public int GetTotalChoicesCount() => GetCachedWindows().Count;

    public void LogWindows(LogLevel level, IEnumerable<ApplicationWindow> result)
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

    public sealed record FocusedAppWindows(ApplicationWindow? FocusedWindow, List<ApplicationWindow> AllWindows)
    {
        public static FocusedAppWindows Empty => new(null, []);
        public int Count => AllWindows.Count;
        public int FocusedWindowIndex => FocusedWindow is null ? -1 : AllWindows.FindIndex(w => w.Handle == FocusedWindow.Handle);
        public ApplicationWindow this[int index] => AllWindows[index];
    }
}
