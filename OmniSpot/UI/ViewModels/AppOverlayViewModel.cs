using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using JetBrains.Annotations;
using OmniSpot.Configuration;
using System.Collections.ObjectModel;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;

namespace OmniSpot.UI.ViewModels;

public record OverlayAppItem(
    Key HotkeyKey,
    string Name,
    ImageSource? Icon,
    bool IsActive = false,
    bool IsElevated = false,
    nint? WindowHandle = null,
    ApplicationConfiguration? Application = null);

public record OverlayWindowGroup(string AppName, IReadOnlyList<OverlayAppItem> Windows);

public partial class AppOverlayViewModel : ObservableObject
{
    public Action<OverlayAppItem>? AppSelected { get; set; }

    public ObservableCollection<OverlayWindowGroup> AppWindowGroups { get; } = [];
    public ObservableCollection<OverlayAppItem> RunningApps { get; } = [];
    public ObservableCollection<OverlayAppItem> LaunchableApps { get; } = [];
    public ObservableCollection<OverlayAppItem> DynamicApps { get; } = [];

    public bool ShowAppWindowGroups => AppWindowGroups.Count > 0;
    public bool ShowRunningApps => RunningApps.Count > 0;
    public bool ShowLaunchableApps => LaunchableApps.Count > 0;
    public bool ShowDynamicApps => DynamicApps.Count > 0;

    [RelayCommand]
    private void OpenApp(OverlayAppItem? item)
    {
        if (item is not null)
        {
            AppSelected?.Invoke(item);
        }
    }

    [UsedImplicitly(Reason = "Design-time constructor")]
    public AppOverlayViewModel()
    {
        var chromeIcon = new BitmapImage(new Uri("pack://application:,,,/Resources/DesignTime/chrome.png"));
        var notepadIcon = new BitmapImage(new Uri("pack://application:,,,/Resources/DesignTime/notepad.png"));
        var explorerIcon = new BitmapImage(new Uri("pack://application:,,,/Resources/DesignTime/explorer.png"));
        var codeIcon = new BitmapImage(new Uri("pack://application:,,,/Resources/DesignTime/code.png"));

        AppWindowGroups = new ObservableCollection<OverlayWindowGroup>([
            new("code", [
                new(Key.D1, "Welcome - Visual Studio Code", codeIcon, IsActive: true),
                new(Key.D2, "README.md - Visual Studio Code", codeIcon)
            ])
        ]);

        RunningApps = new ObservableCollection<OverlayAppItem>([
            new(Key.C, "chrome", chromeIcon, IsElevated: true),
            new(Key.N, "notepad", notepadIcon)
        ]);

        LaunchableApps = new ObservableCollection<OverlayAppItem>([
            new(Key.E, "explorer", explorerIcon)
        ]);

        DynamicApps = new ObservableCollection<OverlayAppItem>([
            new(Key.S, "spotify", null),
            new(Key.T, "terminal", null)
        ]);
    }

    public void Update(
        IEnumerable<OverlayWindowGroup> appWindowGroups,
        IEnumerable<OverlayAppItem> running,
        IEnumerable<OverlayAppItem> launchable,
        IEnumerable<OverlayAppItem> dynamic)
    {
        AppWindowGroups.Clear();
        foreach (var group in appWindowGroups)
        {
            AppWindowGroups.Add(group);
        }

        RunningApps.Clear();
        foreach (var item in running)
        {
            RunningApps.Add(item);
        }

        LaunchableApps.Clear();
        foreach (var item in launchable)
        {
            LaunchableApps.Add(item);
        }

        DynamicApps.Clear();
        foreach (var item in dynamic)
        {
            DynamicApps.Add(item);
        }

        OnPropertyChanged(nameof(ShowAppWindowGroups));
        OnPropertyChanged(nameof(ShowRunningApps));
        OnPropertyChanged(nameof(ShowLaunchableApps));
        OnPropertyChanged(nameof(ShowDynamicApps));
    }
}