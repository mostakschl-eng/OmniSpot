using AwesomeAssertions;
using NSubstitute;
using OmniSpot.UI.ViewModels;
using OmniSpot.UI.ViewModels.Common;
using System.Windows.Input;
using Xunit;

namespace OmniSpot.Tests.UI.ViewModels;

public class SpotlightSearchSettingsViewModelTests
{
    private readonly ISettingsState _state = Substitute.For<ISettingsState>();
    private readonly SpotlightSearchSettingsViewModel _sut;

    public SpotlightSearchSettingsViewModelTests()
    {
        _sut = new SpotlightSearchSettingsViewModel(_state);
    }

    [Fact]
    public void State_ReturnsInjectedSettingsState()
    {
        _sut.State.Should().BeSameAs(_state);
    }

    [Fact]
    public void SpotlightEnabled_CanBeModifiedViaState()
    {
        _state.SpotlightEnabled.Returns(true);
        _sut.State.SpotlightEnabled.Should().BeTrue();

        _sut.State.SpotlightEnabled = false;
        _state.Received(1).SpotlightEnabled = false;
    }

    [Fact]
    public void SpotlightModifierKey_CanBeModifiedViaState()
    {
        _state.SpotlightModifierKey.Returns(Key.LeftAlt);
        _sut.State.SpotlightModifierKey.Should().Be(Key.LeftAlt);

        _sut.State.SpotlightModifierKey = Key.LeftCtrl;
        _state.Received(1).SpotlightModifierKey = Key.LeftCtrl;
    }
}

