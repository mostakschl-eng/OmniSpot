using KeyboardHookLite;
using Microsoft.Extensions.Logging;
using OmniSpot.Extensions;
using OmniSpot.Overlay;
using OmniSpot.Configuration;
using OmniSpot.Spotlight;
using OmniSpot.Stats;
using OmniSpot.WindowDiscovery;
using System.Text.Json;
using System.Windows;
using System.Windows.Input;
using AppConfig = OmniSpot.Configuration.Configuration;

namespace OmniSpot.Input;

/// <summary>Managed keyboard and WPF host for the Rust-owned hook engine.</summary>
internal sealed class Hook(
    ILogger<Hook> logger,
    RustHookBackend rustHook,
    OverlayShowTimer overlayShowTimer,
    WarningOverlayService warningOverlayService,
    AppOverlayService overlayService,
    IProcessInspector processInspector,
    IAppNameResolver appNameResolver,
    StatsService statsService,
    SpotlightService spotlightService) : IDisposable
{
    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web);
    private readonly KeyboardHook _keyboardHook = new();
    private bool _started;

    public void Start(AppConfig config)
    {
        rustHook.RegisterHost(DispatchNativeCallback);
        rustHook.Start(config);
        _keyboardHook.KeyboardPressed += OnKeyboardPressed;
        _started = true;
        logger.LogInformation("Started keyboard hook with Rust event engine");
    }

    public void UpdateConfiguration(AppConfig config)
    {
        rustHook.UpdateConfiguration(config);
    }

    public void SetOverlayPreviewEnabled(bool enabled) => rustHook.SetOverlayPreviewEnabled(enabled);

    private void OnKeyboardPressed(object? sender, KeyboardHookEventArgs e)
    {
        try
        {
            using var _ = logger.MeasureTime($"Handling key event {e.ToFriendlyString()}");
            e.SuppressKeyPress = rustHook.Process((uint)e.InputEvent.Key, e.IsKeyDown(), (uint)e.InputEvent.Flags,
                System.Globalization.CultureInfo.CurrentCulture.Name);
        }
        catch (Exception ex)
        {
            logger.LogError(ex, "Unexpected error handling key press in Rust hook");
        }
    }

    private uint DispatchNativeCallback(uint eventId, byte[] payload)
    {
        try
        {
            switch (eventId)
            {
                case 1:
                    var timeout = JsonDocument.Parse(payload).RootElement.GetProperty("timeoutMs").GetInt32();
                    overlayShowTimer.Configure(rustHook.ShowOverlayIfEnabled, timeout);
                    return 1;
                case 2:
                    overlayShowTimer.Start();
                    return 1;
                case 3:
                    overlayShowTimer.Cancel();
                    return 1;
                case 4:
                    return overlayService.IsVisible ? 1u : 0u;
                case 5:
                    var overlay = JsonSerializer.Deserialize<OverlayRequest>(payload, JsonOptions)
                        ?? throw new InvalidOperationException("Rust sent an invalid overlay request.");
                    overlayService.Show(overlay.Applications.Select(ToApplication).ToArray(), overlay.Dynamic);
                    return 1;
                case 6:
                    overlayService.Hide();
                    return 1;
                case 7:
                    var warning = JsonSerializer.Deserialize<string>(payload, JsonOptions);
                    warningOverlayService.Show(warning switch
                    {
                        "elevated" => WarningContent.Elevated,
                        "keyEventsStealing" => WarningContent.KeyEventsStealing,
                        _ => throw new InvalidOperationException($"Unknown Rust hook warning '{warning}'.")
                    });
                    return 1;
                case 8:
                    spotlightService.Toggle();
                    return 1;
                case 9:
                    EnqueueStats(payload);
                    return 1;
                case 10:
                    MonitorPotentialElevation(JsonSerializer.Deserialize<string>(payload, JsonOptions)
                        ?? throw new InvalidOperationException("Rust sent an empty process path."));
                    return 1;
                case 11:
                    logger.LogError("Rust hook event failed: {Error}", JsonSerializer.Deserialize<string>(payload, JsonOptions));
                    return 1;
                case 12:
                    var request = JsonSerializer.Deserialize<ResolveKeyRequest>(payload, JsonOptions)
                        ?? throw new InvalidOperationException("Rust sent an invalid dynamic-key request.");
                    return appNameResolver.GetDynamicKey(request.ProcessName, request.ProcessPath) is { } key
                        ? (uint)key : 0;
                default:
                    logger.LogWarning("Rust hook sent unknown host callback {EventId}", eventId);
                    return 0;
            }
        }
        catch (Exception ex)
        {
            logger.LogError(ex, "Failed to dispatch Rust hook callback {EventId}", eventId);
            return 0;
        }
    }

    private void EnqueueStats(byte[] payload)
    {
        using var document = JsonDocument.Parse(payload);
        var root = document.RootElement;
        switch (root.GetProperty("kind").GetString())
        {
            case "switch":
                statsService.Enqueue(new SwitchEvent(
                    root.GetProperty("processName").GetString()!,
                    root.GetProperty("processId").ValueKind == JsonValueKind.Null ? null : root.GetProperty("processId").GetUInt32(),
                    root.GetProperty("processPath").GetString()!,
                    root.GetProperty("totalChoices").GetInt32(),
                    root.GetProperty("modifierDownTick").GetInt64(),
                    root.GetProperty("letterDownTick").GetInt64(),
                    root.GetProperty("previousLetterUpTick").ValueKind == JsonValueKind.Null ? null : root.GetProperty("previousLetterUpTick").GetInt64(),
                    root.GetProperty("isDynamic").GetBoolean(),
                    (Key)root.GetProperty("triggerKey").GetUInt32()));
                break;
            case "peek":
                statsService.Enqueue(new PeekEvent(
                    root.GetProperty("targetProcessName").GetString()!,
                    root.GetProperty("targetProcessPath").GetString()!,
                    root.GetProperty("armTick").GetInt64(),
                    root.GetProperty("finishTick").GetInt64(),
                    root.GetProperty("isDynamic").GetBoolean()));
                break;
            case "altTab":
                statsService.Enqueue(new AltTabEvent(root.GetProperty("navCount").GetInt32()));
                break;
            default:
                throw new InvalidOperationException("Rust sent an unknown stats event.");
        }
    }

    private void MonitorPotentialElevation(string path)
    {
        _ = Task.Run(async () =>
        {
            if (await processInspector.WaitForPotentialElevation(path))
            {
                logger.LogDebug("Newly started process {ProcessPath} is elevated, showing warning", path);
                Application.Current.Dispatcher.Invoke(rustHook.PotentialElevationDetected);
            }
        });
    }

    private static ApplicationConfiguration ToApplication(NativeApplication app) => new(
        (Key)app.Key, app.ProcessPath, Enum.Parse<CycleMode>(app.CycleMode), app.StartIfNotRunning,
        Enum.Parse<ApplicationType>(app.ApplicationType), app.Aumid);

    public void Dispose()
    {
        if (_started)
        {
            _keyboardHook.KeyboardPressed -= OnKeyboardPressed;
            rustHook.Stop();
            _started = false;
        }
        _keyboardHook.Dispose();
    }

    private sealed record ResolveKeyRequest(string ProcessName, string ProcessPath);
    private sealed record OverlayRequest(NativeApplication[] Applications, bool Dynamic);
    private sealed record NativeApplication(uint Key, string ProcessPath, string CycleMode,
        bool StartIfNotRunning, string ApplicationType, string? Aumid);
}
