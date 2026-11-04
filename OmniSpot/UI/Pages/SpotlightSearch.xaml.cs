using OmniSpot.UI.ViewModels;
using System.Windows.Controls;

namespace OmniSpot.UI.Pages;

internal partial class SpotlightSearch : Page
{
    public SpotlightSearch(SpotlightSearchSettingsViewModel viewModel)
    {
        InitializeComponent();
        DataContext = viewModel.State;
    }
}