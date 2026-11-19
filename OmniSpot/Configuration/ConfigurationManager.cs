using Microsoft.Extensions.Logging;
using OmniSpot.Configuration.Migrations;

namespace OmniSpot.Configuration;

internal class ConfigurationManager(
    ConfigurationService configService,
    ConfigurationValidator configValidator,
    MigrationRunner migrationRunner,
    PackagedAppPathSanitizer pathSanitizer,
    ILogger<ConfigurationManager> logger)
{
    public event Action<Configuration>? ConfigurationChanged;

    public Configuration? GetConfiguration()
    {
        if (!RustConfigurationBackend.MigrationsRun)
        {
            if (!migrationRunner.RunPending())
            {
                return null;
            }
            RustConfigurationBackend.MarkMigrationsRun();
        }

        var current = RustConfigurationBackend.GetCurrent();
        if (current is null)
        {
            LoadConfiguration();
            current = RustConfigurationBackend.GetCurrent();
        }

        return current;
    }

    public bool UpdateConfiguration(Configuration newConfig)
    {
        try
        {
            configService.WriteConfiguration(newConfig);
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
            var config = configService.ReadConfiguration();

            var sanitized = pathSanitizer.Sanitize(config);
            if (sanitized is not null)
            {
                configService.WriteConfiguration(sanitized);
                config = sanitized;
            }

            configValidator.ValidateAndLog(config);

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
