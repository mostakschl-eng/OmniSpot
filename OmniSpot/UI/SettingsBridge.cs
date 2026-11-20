using Microsoft.Extensions.Logging;
using AppSettings = OmniSpot.Configuration.Configuration;

namespace OmniSpot.UI;

internal class SettingsBridge(
    RustDatabase database,
    ILogger<SettingsBridge> logger)
{
    public event Action<AppSettings>? ConfigurationChanged;

    public AppSettings? GetConfiguration()
    {
        var current = RustConfigurationBackend.GetCurrent();
        if (current is null)
        {
            LoadConfiguration();
            current = RustConfigurationBackend.GetCurrent();
        }

        return current;
    }

    public bool UpdateConfiguration(AppSettings newConfig)
    {
        try
        {
            database.SaveSettings(newConfig);
            RustConfigurationBackend.SetCurrent(newConfig);
            logger.LogInformation("Configuration updated successfully");
            ConfigurationChanged?.Invoke(newConfig);
            return true;
        }
        catch (Exception ex)
        {
            logger.LogError(ex, "Error updating configuration");
            return false;
        }
    }

    private void LoadConfiguration()
    {
        try
        {
            var (config, validationMessage) = database.LoadSettings();
            if (validationMessage is not null)
                logger.LogError("Config validation error: {Message}", validationMessage);

            RustConfigurationBackend.SetCurrent(config);
            logger.LogInformation("Configuration loaded successfully");
            ConfigurationChanged?.Invoke(config);
        }
        catch (Exception ex)
        {
            logger.LogError(ex, "Error loading configuration");
        }
    }
}
