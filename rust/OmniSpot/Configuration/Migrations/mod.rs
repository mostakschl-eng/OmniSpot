use super::model::ApplicationType;
use super::storage::SettingsDocument;
use crate::window_discovery::PackagedApps;

pub trait Migration {
    fn version(&self) -> u32;
    fn up(&self, document: &mut SettingsDocument) -> Result<(), String>;
}

#[derive(Default)]
pub struct MigrationRunner {
    migrations: Vec<Box<dyn Migration>>,
}

impl MigrationRunner {
    pub fn new() -> Self {
        Self {
            migrations: Vec::new(),
        }
    }
    pub fn with_migration(mut self, migration: Box<dyn Migration>) -> Self {
        self.migrations.push(migration);
        self
    }
    pub fn run_pending(&self, document: &mut SettingsDocument) -> Result<bool, String> {
        let mut changed = false;
        let mut pending = self
            .migrations
            .iter()
            .filter(|m| m.version() > document.schema_version)
            .collect::<Vec<_>>();
        pending.sort_by_key(|migration| migration.version());
        for migration in pending {
            migration.up(document)?;
            document.schema_version = migration.version();
            changed = true;
        }
        Ok(changed)
    }
}

/// Converts legacy Win32 records that are actually installed Windows packages
/// into launchable packaged-app records with their AUMID.
pub struct FixPackagedAppsMigration;

impl Migration for FixPackagedAppsMigration {
    fn version(&self) -> u32 {
        2
    }

    fn up(&self, document: &mut SettingsDocument) -> Result<(), String> {
        let mut packages = PackagedApps::default();
        for app in &mut document.applications {
            if app.application_type != ApplicationType::Win32 {
                continue;
            }
            if let Some(package) = packages.installed().iter().find(|package| {
                std::path::Path::new(&app.process_path)
                    .starts_with(std::path::Path::new(&package.installed_path))
            }) && let Some(aumid) = &package.aumid
            {
                app.application_type = ApplicationType::Packaged;
                app.aumid = Some(aumid.clone());
            }
        }
        Ok(())
    }
}
