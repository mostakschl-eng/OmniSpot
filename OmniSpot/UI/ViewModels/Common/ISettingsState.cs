using OmniSpot.Configuration;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Windows.Input;

namespace OmniSpot.UI.ViewModels.Common;

internal interface ISettingsState : INotifyPropertyChanged
{
    Key ModifierKey { get; set; }
    bool PulseBorderEnabled { get; set; }
    AppThemeSetting Theme { get; set; }
    bool OverlayEnabled { get; set; }
    int OverlayShowDelayMs { get; set; }
    bool OverlayKeepOpenWhileModifierHeld { get; set; }
    bool PeekEnabled { get; set; }
    bool DynamicModeEnabled { get; set; }
    bool StatsEnabled { get; set; }
    bool SpotlightEnabled { get; set; }
    Key SpotlightModifierKey { get; set; }
    ObservableCollection<string> SpotlightCustomDirectories { get; set; }
    ObservableCollection<ApplicationShortcutViewModel> Applications { get; set; }
    bool HasNoApplications { get; }
    ObservableCollection<DynamicApplicationViewModel> DynamicApplications { get; set; }
    bool HasDynamicApplications { get; }
    bool LaunchAtStartup { get; set; }

    bool IsDirty { get; }
    bool CanSave { get; }
    bool HasValidationErrors { get; }
    string? ValidationSummary { get; }

    void LoadConfiguration();
    bool Save();
    ApplicationShortcutViewModel? AddApplication(ApplicationSelectionArgs args);
    void RemoveApplication(ApplicationShortcutViewModel application);
    void MoveApplication(int oldIndex, int newIndex);
    ApplicationShortcutViewModel? PinApplication(DynamicApplicationViewModel dynamic);
    void AddSpotlightCustomDirectory(string path);
    void RemoveSpotlightCustomDirectory(string path);
}