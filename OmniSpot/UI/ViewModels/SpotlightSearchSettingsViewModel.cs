using OmniSpot.UI.ViewModels.Common;

namespace OmniSpot.UI.ViewModels;

internal class SpotlightSearchSettingsViewModel(ISettingsState state)
{
    public ISettingsState State { get; } = state;
}