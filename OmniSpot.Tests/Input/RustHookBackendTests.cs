using OmniSpot;
using OmniSpot.Configuration;
using OmniSpot.Input;
using System.Text.Json;
using System.Windows.Input;
using Xunit;
using AwesomeAssertions;
using AppConfig = OmniSpot.Configuration.Configuration;

namespace OmniSpot.Tests.Input;

public class RustHookBackendTests
{
    [Fact]
    public void StartAndHostCallbacks_UseRustHookRuntimeThroughFfi()
    {
        var backend = new RustHookBackend();
        var callbacks = new List<(uint Id, string Payload)>();
        backend.RegisterHost((id, payload) =>
        {
            callbacks.Add((id, payload.Length == 0 ? string.Empty : JsonDocument.Parse(payload).RootElement.ToString()));
            return 1;
        });

        backend.Start(new AppConfig(Key.RightCtrl, [], true, AppThemeSetting.System,
            true, 500, true, false, true, false));
        backend.SetOverlayPreviewEnabled(false);

        callbacks.Select(callback => callback.Id).Should().ContainInOrder(1u, 3u, 6u);
        callbacks[0].Payload.Should().Contain("timeoutMs");
        backend.Process((uint)Key.A, isDown: true, flags: 0x10, culture: "en-US").Should().BeFalse();
        backend.Stop();
    }
}
