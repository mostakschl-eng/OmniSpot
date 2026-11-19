using LiteDB;
using OmniSpot;
using OmniSpot.WindowDiscovery;
using OmniSpot.Configuration.Storage;

namespace OmniSpot.Configuration.Migrations;

internal class FixPackagedAppsMigration(IPackagedAppsService packagedAppsService) : IMigration
{
    public int Version => 1;

    public void Up(LiteDatabase db)
    {
        var collection = db.GetCollection<SettingsDocument>("settings");
        var document = collection.FindById(1);

        if (document is null)
        {
            // first run of the application won't have any settings yet
            return;
        }

        var apps = document.Applications.Select(app => app.ToApplicationConfiguration()).ToArray();
        var matches = apps.Where(app => app.Type == ApplicationType.Win32)
            .Select(app => (App: app, Info: packagedAppsService.GetByInstalledPath(app.ProcessPath, null)))
            .Where(item => item.Info is not null)
            .Select(item => new RustConfigurationBackend.PackagedPath(item.App.ProcessPath, item.Info!.Aumid))
            .ToArray();
        var migrated = RustConfigurationBackend.MigratePackagedApps(apps, matches);
        if (!apps.SequenceEqual(migrated))
        {
            document.Applications = migrated.Select(ApplicationConfigurationDocument.FromApplicationConfiguration).ToList();
            collection.Update(document);
        }
    }
}
