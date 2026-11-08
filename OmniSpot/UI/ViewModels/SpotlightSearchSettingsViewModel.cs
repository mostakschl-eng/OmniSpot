using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using OmniSpot.UI.ViewModels.Common;
using System.Collections.ObjectModel;
using System.IO;
using System.Windows.Input;

namespace OmniSpot.UI.ViewModels;

internal partial class SpotlightSearchSettingsViewModel(ISettingsState state) : ObservableObject
{
    public ISettingsState State { get; } = state;

    public bool SpotlightEnabled
    {
        get => State.SpotlightEnabled;
        set => State.SpotlightEnabled = value;
    }

    public Key SpotlightModifierKey
    {
        get => State.SpotlightModifierKey;
        set => State.SpotlightModifierKey = value;
    }

    public ObservableCollection<string> SpotlightCustomDirectories => State.SpotlightCustomDirectories;

    [RelayCommand]
    private void AddCustomFolder()
    {
        var dialog = new Microsoft.Win32.OpenFolderDialog
        {
            Title = "Select Folder to Index",
            Multiselect = false
        };

        if (dialog.ShowDialog() == true && !string.IsNullOrWhiteSpace(dialog.FolderName) && Directory.Exists(dialog.FolderName))
        {
            State.AddSpotlightCustomDirectory(dialog.FolderName);
        }
    }

    [RelayCommand]
    private void RemoveCustomFolder(string? path)
    {
        if (!string.IsNullOrWhiteSpace(path))
        {
            State.RemoveSpotlightCustomDirectory(path);
        }
    }
}