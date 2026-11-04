using OmniSpot.UI.ViewModels;
using System.Windows.Controls;

namespace OmniSpot.UI.Pages;

internal partial class About : Page
{
    public About(AboutViewModel viewModel)
    {
        InitializeComponent();
        DataContext = viewModel;
    }
}