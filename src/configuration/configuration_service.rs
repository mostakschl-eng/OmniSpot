use std::sync::{Arc, Mutex};
use crate::configuration::configuration::Configuration;
use crate::configuration::migrations::i_migration::ConfigurationDatabase;
use crate::configuration::storage::SettingsDocument;

pub(crate) const SETTINGS_DOCUMENT_ID: i32 = 1;

pub(crate) struct ConfigurationService {
    db: Arc<Mutex<ConfigurationDatabase>>,
}

impl ConfigurationService {
    pub(crate) fn new(db: Arc<Mutex<ConfigurationDatabase>>) -> Self {
        Self { db }
    }

    pub(crate) fn read_configuration(&self) -> Configuration {
        let mut db = self.db.lock().unwrap();

        let doc = match &db.settings {
            Some(existing) => existing.clone(),
            None => {
                let defaults = Self::seed_defaults(&mut db);
                defaults
            }
        };

        doc.to_configuration()
    }

    pub(crate) fn write_configuration(&self, config: &Configuration) {
        let mut db = self.db.lock().unwrap();
        let document = SettingsDocument::from_configuration(config, SETTINGS_DOCUMENT_ID);
        db.settings = Some(document);
    }

    fn seed_defaults(db: &mut ConfigurationDatabase) -> SettingsDocument {
        let defaults = SettingsDocument::from_configuration(&Configuration::default(), SETTINGS_DOCUMENT_ID);

        db.settings = Some(defaults.clone());
        defaults
    }
}

#[cfg(test)]
#[path = "../tests/configuration/configuration_service_tests.rs"]
mod tests;
