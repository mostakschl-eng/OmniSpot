using AwesomeAssertions;
using OmniSpot;
using OmniSpot.Configuration;
using System.Windows.Input;
using Xunit;
using AppConfig = OmniSpot.Configuration.Configuration;

namespace OmniSpot.Tests.Configuration;

public class RustConfigurationRuntimeTests
{
    [Fact]
    public void CurrentConfigurationAndMigrationState_RoundTripThroughRustRuntime()
    {
        var config = new AppConfig(Key.RightCtrl,
            [new ApplicationConfiguration(Key.N, @"C:\Apps\notepad.exe", CycleMode.NextApp, false)],
            true, AppThemeSetting.Dark, false, 750, true, false, true, false,
            true, Key.LeftAlt, [@"C:\Apps"]);

        RustConfigurationBackend.MigrationsRun.Should().BeFalse();
        RustConfigurationBackend.SetCurrent(config);
        RustConfigurationBackend.GetCurrent().Should().BeEquivalentTo(config);
        RustConfigurationBackend.MarkMigrationsRun();
        RustConfigurationBackend.MigrationsRun.Should().BeTrue();
    }
}
