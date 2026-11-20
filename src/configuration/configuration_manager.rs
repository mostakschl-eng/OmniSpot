use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use super::configuration::Configuration;
use super::configuration_service::ConfigurationService;
use super::configuration_validator::ConfigurationValidator;
use super::migrations::fix_packaged_apps_migration::FixPackagedAppsMigration;
use super::migrations::i_migration::ConfigurationDatabase;
use super::migrations::migration_runner::MigrationRunner;
use super::packaged_app_path_sanitizer::{IPackagedAppsService, PackagedAppPathSanitizer};

pub(crate) type ConfigurationChangeListener = Box<dyn Fn(&Configuration) + Send + Sync>;

pub(crate) struct ConfigurationManager {
    config_service: Arc<ConfigurationService>,
    packaged_apps: Arc<dyn IPackagedAppsService + Send + Sync>,
    db: Arc<Mutex<ConfigurationDatabase>>,
    current_configuration: RwLock<Option<Configuration>>,
    migrations_run: AtomicBool,
    listeners: Mutex<Vec<ConfigurationChangeListener>>,
}

impl ConfigurationManager {
    pub(crate) fn new(
        config_service: Arc<ConfigurationService>,
        packaged_apps: Arc<dyn IPackagedAppsService + Send + Sync>,
        db: Arc<Mutex<ConfigurationDatabase>>,
    ) -> Self {
        Self {
            config_service,
            packaged_apps,
            db,
            current_configuration: RwLock::new(None),
            migrations_run: AtomicBool::new(false),
            listeners: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn on_configuration_changed(&self, listener: impl Fn(&Configuration) + Send + Sync + 'static) {
        let mut list = self.listeners.lock().unwrap();
        list.push(Box::new(listener));
    }

    pub(crate) fn get_configuration(&self) -> Option<Configuration> {
        if !self.migrations_run.load(Ordering::SeqCst) {
            let migration = FixPackagedAppsMigration::new(self.packaged_apps.clone());
            let runner = MigrationRunner::new(vec![&migration]);
            let mut db_guard = self.db.lock().unwrap();
            if !runner.run_pending(&mut db_guard) {
                return None;
            }
            self.migrations_run.store(true, Ordering::SeqCst);
        }

        {
            let current = self.current_configuration.read().unwrap();
            if let Some(config) = current.as_ref() {
                return Some(config.clone());
            }
        }

        self.load_configuration()
    }

    pub(crate) fn update_configuration(&self, new_config: Configuration) -> bool {
        self.config_service.write_configuration(&new_config);
        {
            let mut current = self.current_configuration.write().unwrap();
            *current = Some(new_config.clone());
        }

        self.notify_listeners(&new_config);
        true
    }

    fn load_configuration(&self) -> Option<Configuration> {
        let mut config = self.config_service.read_configuration();

        let sanitizer = PackagedAppPathSanitizer::new(self.packaged_apps.clone());
        if let Some(sanitized) = sanitizer.sanitize(&config) {
            self.config_service.write_configuration(&sanitized);
            config = sanitized;
        }

        let _ = ConfigurationValidator::validate(&config);

        {
            let mut current = self.current_configuration.write().unwrap();
            *current = Some(config.clone());
        }

        self.notify_listeners(&config);
        Some(config)
    }

    fn notify_listeners(&self, config: &Configuration) {
        let listeners = self.listeners.lock().unwrap();
        for listener in listeners.iter() {
            listener(config);
        }
    }
}

#[cfg(test)]
#[path = "../tests/configuration/configuration_manager_tests.rs"]
mod tests;
