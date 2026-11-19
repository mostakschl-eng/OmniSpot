using OmniSpot.Configuration;
using OmniSpot.WindowDiscovery;
using System.IO;
using Windows.Win32;
using Windows.Win32.Foundation;
using Windows.Win32.Graphics.Dwm;
using Windows.Win32.UI.WindowsAndMessaging;

namespace OmniSpot;

internal sealed record AppSwitchResult(
    uint? ProcessId,
    string ProcessPath,
    HWND Handle,
    SHOW_WINDOW_CMD State,
    bool NeedsElevation,
    bool WasStarted)
{
    public string ProcessName { get; } = Path.GetFileName(ProcessPath);

    public static AppSwitchResult FromApplicationWindow(ApplicationWindow w) =>
        new(w.ProcessId, w.ProcessImagePath, w.Handle, SHOW_WINDOW_CMD.SW_SHOW, w.NeedsElevation, WasStarted: false);

    public static AppSwitchResult FromStarted(string processPath) =>
        new(null, processPath, HWND.Null, SHOW_WINDOW_CMD.SW_SHOW, NeedsElevation: false, WasStarted: true);
};

internal class Switcher(ConfigurationManager configurationManager)
{
    public AppSwitchResult? Execute(IReadOnlyList<ApplicationConfiguration> appGroup)
    {
        var result = RustSwitcherBackend.Execute(appGroup);
        if (result is { WasStarted: false } && configurationManager.GetConfiguration()?.PulseBorderEnabled == true)
        {
            _ = PulseBorder(new IntPtr(RustSwitcherBackend.ToInt64(result.Handle)), System.Drawing.Color.Gold, 100);
        }
        return result;
    }

    public bool SwitchToWindowByIndex(
        IReadOnlyList<ApplicationConfiguration> applications,
        int index,
        out ApplicationWindow? focusedWindow)
    {
        var result = RustSwitcherBackend.SwitchToWindowByIndex(applications, index);
        focusedWindow = result.Window;
        if (focusedWindow is not null && configurationManager.GetConfiguration()?.PulseBorderEnabled == true)
        {
            _ = PulseBorder(new IntPtr(RustSwitcherBackend.ToInt64(focusedWindow.Handle)), System.Drawing.Color.Gold, 100);
        }
        return result.Switched;
    }

    internal void Open(ApplicationConfiguration appConfig)
    {
        RustSwitcherBackend.Open(appConfig);
    }

    internal void ActivateWindow(ApplicationWindow window, bool pulseBorder = true)
    {
        var hwnd = window.Handle;
        RustSwitcherBackend.ActivateWindow(window);

        if (pulseBorder && configurationManager.GetConfiguration()?.PulseBorderEnabled == true)
        {
            // it needs to go async because we're calling it from a keyboard hook which has narrow time limits
            _ = PulseBorder(hwnd, Color.Gold, 100);
        }
    }

    private async Task PulseBorder(IntPtr hwnd, Color highlightColor, int durationMs = 400)
    {
        var color = highlightColor.R | (highlightColor.G << 8) | (highlightColor.B << 16);
        // ReSharper disable once InconsistentNaming
        var DWMWA_COLOR_DEFAULT = -1;

        await Task.Run(async () =>
        {
            SetDwmColor(color);
            await Task.Delay(durationMs);
            SetDwmColor(DWMWA_COLOR_DEFAULT);
            await Task.Delay(durationMs);
            SetDwmColor(color);
            await Task.Delay(durationMs);
            SetDwmColor(DWMWA_COLOR_DEFAULT);
        });

        unsafe void SetDwmColor(int colorRef)
        {
            // The pointer is only "alive" for the duration of this specific call
            PInvoke.DwmSetWindowAttribute(
                new HWND(hwnd),
                DWMWINDOWATTRIBUTE.DWMWA_BORDER_COLOR,
                &colorRef,
                sizeof(int)
            );
        }
    }

    internal void HideWindow(HWND hwnd)
    {
        RustSwitcherBackend.HideWindow(hwnd);
    }

}
