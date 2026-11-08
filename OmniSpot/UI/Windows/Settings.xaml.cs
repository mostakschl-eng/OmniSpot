using OmniSpot.Extensions;
using OmniSpot.UI.Pages;
using OmniSpot.UI.ViewModels;
using System.ComponentModel;
using Wpf.Ui;
using Wpf.Ui.Abstractions;
using Wpf.Ui.Controls;

namespace OmniSpot.UI.Windows;

internal partial class Settings
{
    private readonly SettingsViewModel _viewModel;

    public Settings(INavigationViewPageProvider pageProvider, SettingsViewModel viewModel, ISnackbarService snackbarService, IContentDialogService contentDialogService)
    {
        InitializeComponent();
        _viewModel = viewModel;
        DataContext = viewModel;

        NavigationView.SetPageProviderService(pageProvider);
        snackbarService.SetSnackbarPresenter(RootSnackbarPresenter);
        contentDialogService.SetDialogHost(RootDialogHost);
    }

    protected override void OnActivated(EventArgs e)
    {
        if (NavigationView.SelectedItem is null)
        {
            NavigationView.Navigate(typeof(Hotkeys));
        }
    }

    protected override void OnClosing(CancelEventArgs e)
    {
        base.OnClosing(e);

        if (_viewModel.State.IsDirty)
        {
            var result = new MessageBox
            {
                Title = "Unsaved Changes",
                Content = "You have unsaved changes. Closing this window will discard them. Are you sure?",
                PrimaryButtonText = "Discard",
                PrimaryButtonAppearance = ControlAppearance.Secondary,
                SecondaryButtonText = "Cancel",
                SecondaryButtonAppearance = ControlAppearance.Primary,
                IsCloseButtonEnabled = false
            }.ShowSync();

            if (result != MessageBoxResult.Primary)
            {
                e.Cancel = true;
            }
            else
            {
                _viewModel.State.LoadConfiguration();
            }
        }
    }
}