using System.Diagnostics;
using System.IO;
using Windows.Win32.Foundation;
using Windows.Win32.UI.WindowsAndMessaging;

namespace OmniSpot.WindowDiscovery;

[DebuggerDisplay("{Title} ({ProcessId})")]
internal record ApplicationWindow(
    HWND Handle,
    string Title,
    uint ProcessId,
    string ProcessImagePath,
    SHOW_WINDOW_CMD State,
    Point Position,
    Size Size,
    WINDOW_STYLE Style,
    WindowStyleEx StyleEx,
    bool IsCloaked,
    bool NeedsElevation)
{
    public string ProcessName => Path.GetFileName(ProcessImagePath);

    public string? GetProductName() => FileVersionInfo.GetVersionInfo(ProcessImagePath).ProductName;
}
