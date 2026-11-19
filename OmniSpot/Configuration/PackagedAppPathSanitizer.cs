using Microsoft.Extensions.Logging;
using OmniSpot.WindowDiscovery;

namespace OmniSpot.Configuration;

/// <summary>Thin package metadata adapter for the Rust configuration sanitizer.</summary>
internal sealed class PackagedAppPathSanitizer(
    IPackagedAppsService packagedAppsService,
    ILogger<PackagedAppPathSanitizer> logger)
{
    public Configuration? Sanitize(Configuration config)
    {
        var sanitized = RustConfigurationBackend.Sanitize(config, packagedAppsService);
        if (sanitized is not null)
        {
            logger.LogInformation("Rust configuration sanitizer corrected packaged application paths");
        }
        return sanitized;
    }
}
