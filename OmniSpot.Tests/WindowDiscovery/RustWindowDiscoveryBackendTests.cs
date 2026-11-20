using AwesomeAssertions;
using OmniSpot;
using OmniSpot.WindowDiscovery;
using Xunit;

namespace OmniSpot.Tests.WindowDiscovery;

public class RustWindowDiscoveryBackendTests
{
    [Fact]
    public void MissingForegroundWindow_IsAValidNullResponse()
    {
        RustWindowDiscoveryBackend.DecodeJson<RustWindowDiscoveryBackend.RustWindow>(
            "null"u8, allowNull: true).Should().BeNull();

        Action readWindowList = () => RustWindowDiscoveryBackend.DecodeJson<RustWindowDiscoveryBackend.RustWindow[]>("null"u8);
        readWindowList.Should().Throw<InvalidOperationException>();
    }

    [Fact]
    public void GetRunningApplications_ReturnsEmpty_WhenNoWindowsWereDiscovered()
    {
        var results = RustWindowDiscoveryBackend.GetRunningApplications(
            Array.Empty<ApplicationWindow>(), Array.Empty<string>(), Array.Empty<string>());

        results.Should().BeEmpty();
    }
}
