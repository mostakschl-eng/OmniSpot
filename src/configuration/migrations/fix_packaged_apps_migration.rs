use super::super::application_type::ApplicationType;
use super::super::packaged_app_path_sanitizer::IPackagedAppsService;
use super::super::storage::ApplicationConfigurationDocument;
use super::i_migration::{ConfigurationDatabase, IMigration};

pub(crate) struct FixPackagedAppsMigration<P: IPackagedAppsService> {
    packaged_apps_service: P,
}

impl<P: IPackagedAppsService> FixPackagedAppsMigration<P> {
    pub(crate) fn new(packaged_apps_service: P) -> Self {
        Self {
            packaged_apps_service,
        }
    }
}

impl<P: IPackagedAppsService> IMigration for FixPackagedAppsMigration<P> {
    fn version(&self) -> i32 {
        1
    }

    fn up(&self, db: &mut ConfigurationDatabase) -> Result<(), String> {
        let Some(document) = &mut db.settings else {
            return Ok(());
        };

        let mut updated_apps = Vec::new();
        let mut changed = false;

        for app in &document.applications {
            if app.r#type == ApplicationType::Win32 {
                if let Some(info) = self
                    .packaged_apps_service
                    .get_by_installed_path(&app.process_path)
                {
                    updated_apps.push(ApplicationConfigurationDocument {
                        key: app.key,
                        process_path: app.process_path.clone(),
                        cycle_mode: app.cycle_mode,
                        start_if_not_running: app.start_if_not_running,
                        r#type: ApplicationType::Packaged,
                        aumid: Some(info.aumid),
                    });
                    changed = true;
                    continue;
                }
            }
            updated_apps.push(app.clone());
        }

        if changed {
            document.applications = updated_apps;
        }

        Ok(())
    }
}

#[cfg(test)]
#[path = "../../tests/configuration/migrations/fix_packaged_apps_migration_tests.rs"]
mod tests;
