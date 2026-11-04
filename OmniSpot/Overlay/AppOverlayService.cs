using OmniSpot.Configuration;
using OmniSpot.Extensions;
using OmniSpot.Input;
using OmniSpot.Stats;
using OmniSpot.UI.ViewModels;
using OmniSpot.UI.Windows;
using OmniSpot.WindowDiscovery;
using Microsoft.Extensions.Logging;
using System.IO;
using System.Windows;
using System.Windows.Input;
using System.Windows.Threading;
using Windows.Win32.Foundation;

namespace OmniSpot.Overlay;

internal class AppOverlayService(
    AppOverlayWindow window,
    AppOverlayViewModel viewModel,
    AppRegistryCache appRegistryCache,
    IWindowEnumerator windowEnumerator,
    IconExtractor iconExtractor,
    WindowTitleParser windowTitleParser,
    DynamicModeService dynamicModeService,
    IPackagedAppsService packagedAppsService,
    Switcher switcher,
    ILogger<AppOverlayService> logger)
{
    private record WindowSnapshot(ApplicationWindow Window, string Title, bool IsActive = false);
    private record WindowGroupSnapshot(string AppName, IReadOnlyList<WindowSnapshot> Windows);
    private record AppSnapshot(ApplicationConfiguration Application, string DisplayName, string? PackagedAppIconPath, bool IsRunning, bool NeedsElevation);

    public bool IsVisible { get; private set; }

    private volatile int _showToken;

    public void Show(IReadOnlyList<ApplicationConfiguration> applications, bool dynamicModeEnabled)
    {
        var token = Interlocked.Increment(ref _showToken);

        var measureTime = logger.MeasureTime("Show overlay prep");
        var allWindows = windowEnumerator.GetWindows();
        var runningProcesses = allWindows
            .DistinctBy(w => w.ProcessName, StringComparer.OrdinalIgnoreCase)
            .ToDictionary(n => n.ProcessName, StringComparer.OrdinalIgnoreCase);

        var needsElevationProcessNames = allWindows
            .Where(w => w.NeedsElevation)
            .Select(w => w.ProcessName)
            .ToHashSet(StringComparer.OrdinalIgnoreCase);

        var windowGroups = GetAppWindowData(allWindows, applications);
        var appSnapshots = applications
            .Select(app => new AppSnapshot(
                app,
                appRegistryCache.GetDisplayName(app.ProcessName),
                GetIconPath(app),
                runningProcesses.ContainsKey(app.ProcessName),
                needsElevationProcessNames.Contains(app.ProcessName)))
            .ToList();

        var dynamicSnapshots = dynamicModeEnabled
            ? dynamicModeService.GetAllDynamicApps(applications, allWindows)
                .Select(app => new AppSnapshot(
                    app,
                    appRegistryCache.GetDisplayName(app.ProcessName, app.ProcessPath),
                    GetIconPath(app),
                    IsRunning: true,
                    needsElevationProcessNames.Contains(app.ProcessName)))
                .ToList()
            : [];

        measureTime.Dispose();

        Application.Current.Dispatcher.BeginInvoke(() =>
            ApplyToViewModel(windowGroups, appSnapshots, dynamicSnapshots, token));
        return;

        string? GetIconPath(ApplicationConfiguration app)
        {
            if (app.Type == ApplicationType.Win32)
            {
                return null;
            }

            try
            {
                if (app.Aumid != null)
                {
                    return packagedAppsService.GetByAumid(app.Aumid)?.IconPath;
                }

                var runningProcess = runningProcesses.GetValueOrDefault(app.ProcessName);
                var processId = runningProcess?.ProcessId;
                return packagedAppsService
                    .GetByInstalledPath(runningProcess?.ProcessImagePath ?? app.ProcessPath, processId)?.IconPath;
            }
            catch (Exception e)
            {
                logger.LogWarning(e, "Failed to get icon path for {ProcessPath}", app.ProcessPath);
                return null;
            }
        }
    }

    public void Hide()
    {
        Interlocked.Increment(ref _showToken);

        Application.Current.Dispatcher.BeginInvoke(() =>
        {
            window.Hide();
            IsVisible = false;
            logger.LogDebug("Overlay hidden");
        });
    }

    private List<WindowGroupSnapshot> GetAppWindowData(
        IReadOnlyList<ApplicationWindow> allWindows,
        IReadOnlyList<ApplicationConfiguration> applications)
    {
        var currentWindow = windowEnumerator.GetCurrentWindow();
        var groups = new List<WindowGroupSnapshot>();

        foreach (var app in applications
                     .Where(a => a.CycleMode == CycleMode.NextWindow)
                     .DistinctBy(a => a.ProcessName, StringComparer.OrdinalIgnoreCase))
        {
            var windows = allWindows
                .Where(w => w.ProcessImagePath.EndsWith(app.ProcessName, StringComparison.OrdinalIgnoreCase))
                .OrderBy(static w => (IntPtr)w.Handle)
                .Take(10)
                .ToList();
            if (windows.Count < 2)
            {
                continue;
            }

            var commonSuffix = windowTitleParser.FindCommonSuffix(windows.Select(w => w.Title).ToList());
            groups.Add(new WindowGroupSnapshot(
                appRegistryCache.GetDisplayName(app.ProcessName),
                windows.Select(w => new WindowSnapshot(
                    w,
                    windowTitleParser.StripSuffix(w.Title, commonSuffix),
                    w.Handle == currentWindow?.Handle)).ToList()));
        }

        return groups;
    }

    private void ApplyToViewModel(
        List<WindowGroupSnapshot> windowGroups,
        List<AppSnapshot> appSnapshots,
        List<AppSnapshot> dynamicSnapshots,
        int token)
    {
        // Icon extraction and BitmapImage construction must happen on the UI thread
        var windowGroupItems = windowGroups
            .Select(group => new OverlayWindowGroup(group.AppName, group.Windows
                .Select((w, i) => new OverlayAppItem(
                    IndexToKey(i), w.Title, iconExtractor.GetByProcessPath(w.Window.ProcessImagePath), w.IsActive,
                    WindowHandle: (IntPtr)w.Window.Handle))
                .ToList()))
            .ToList();

        var processPath = windowEnumerator.GetCurrentWindow()?.ProcessImagePath;

        var running = new List<OverlayAppItem>();
        var launchable = new List<OverlayAppItem>();
        foreach (var app in appSnapshots)
        {
            var item = CreateOverlayAppItem(app);
            (app.IsRunning ? running : launchable).Add(item);
        }

        var dynamic = dynamicSnapshots
            .Select(CreateOverlayAppItem)
            .ToList();

        viewModel.AppSelected = OpenSelectedItem;
        viewModel.Update(windowGroupItems, running, launchable, dynamic);

        // Show the window after WPF's data-binding (priority 8) and render (priority 7) passes
        // to avoid showing a blank/stale frame before content is ready.
        Application.Current.Dispatcher.BeginInvoke(DispatcherPriority.ContextIdle, () =>
        {
            if (_showToken != token)
            {
                return;
            }

            window.PositionAtCursor();
            window.Show();
            IsVisible = true;
            logger.LogDebug(
                "Overlay shown: {Windows} focused windows, {Running} running, {Launchable} launchable, {Dynamic} dynamic apps",
                windowGroupItems.Sum(group => group.Windows.Count), running.Count, launchable.Count, dynamic.Count);
        });

        OverlayAppItem CreateOverlayAppItem(AppSnapshot app)
        {
            var isActive = app.Application.ProcessPath == processPath;
            var icon = app.PackagedAppIconPath != null ? iconExtractor.GetByIconPath(app.PackagedAppIconPath) : iconExtractor.GetByProcessPath(app.Application.ProcessPath);
            var item = new OverlayAppItem(app.Application.Key, app.DisplayName, icon, isActive, app.NeedsElevation, Application: app.Application);
            return item;
        }
    }

    private void OpenSelectedItem(OverlayAppItem item)
    {
        try
        {
            if (item.WindowHandle is { } selectedHandle)
            {
                var currentWindow = windowEnumerator.GetWindows().FirstOrDefault(w => w.Handle == new HWND(selectedHandle));
                if (currentWindow is not null)
                {
                    switcher.ActivateWindow(currentWindow, pulseBorder: false);
                }
            }
            else if (item.Application is { } app)
            {
                switcher.Open(app);
            }
        }
        catch (Exception e)
        {
            logger.LogError(e, "Failed to open overlay item {Name}", item.Name);
        }
        finally
        {
            Hide();
        }
    }

    // Key.D0 = 34, Key.D1 = 35, …, Key.D9 = 43 — sequential enum values; index 9 wraps to D0
    private static Key IndexToKey(int i) => i == 9 ? Key.D0 : Key.D1 + i;
}
