using OmniSpot.UI.ViewModels;
using System.Windows.Controls;

namespace OmniSpot.UI.Pages;

internal partial class General : Page
{
    public General(GeneralSettingsViewModel viewModel)
    {
        InitializeComponent();
        DataContext = viewModel;
    }
}