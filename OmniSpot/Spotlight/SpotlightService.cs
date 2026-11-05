using Microsoft.Extensions.Logging;
using OmniSpot.Configuration;
using OmniSpot.UI.ViewModels;
using OmniSpot.UI.Windows;
using System.Windows;

namespace OmniSpot.Spotlight;

internal class SpotlightService(
    SpotlightWindow window,
    SpotlightViewModel viewModel,
    ConfigurationManager configurationManager,
    ILogger<SpotlightService> logger)
{
    public bool IsVisible => window.IsVisible;

    public void Toggle()
    {
        var config = configurationManager.GetConfiguration();
        if (config?.SpotlightEnabled == false)
        {
            return;
        }

        Application.Current.Dispatcher.Invoke(() =>
        {
            if (window.IsVisible)
            {
                Hide();
            }
            else
            {
                Show();
            }
        });
    }

    public void Show()
    {
        Application.Current.Dispatcher.Invoke(() =>
        {
            logger.LogDebug("Opening Spotlight search window");
            viewModel.ResetAndLoad();
            window.Show();
            window.Activate();
            window.FocusSearchBox();
        });
    }

    public void Hide()
    {
        Application.Current.Dispatcher.Invoke(() =>
        {
            logger.LogDebug("Closing Spotlight search window");
            window.Hide();
        });
    }
}