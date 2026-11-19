using AwesomeAssertions;
using OmniSpot;
using OmniSpot.WindowDiscovery;
using Xunit;

namespace OmniSpot.Tests.WindowDiscovery;

public class RustWindowDiscoveryBackendTests
{
    [Fact]
    public void GetRunningApplications_ReturnsEmpty_WhenNoWindowsWereDiscovered()
    {
        var results = RustWindowDiscoveryBackend.GetRunningApplications(
            Array.Empty<ApplicationWindow>(), Array.Empty<string>(), Array.Empty<string>());

        results.Should().BeEmpty();
    }
}
