using OmniSpot.UI.ViewModels.Common;

namespace OmniSpot.UI.ViewModels;

internal class OverlaySettingsViewModel(ISettingsState state)
{
    public ISettingsState State { get; } = state;
}