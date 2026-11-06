use super::{
    migrations::MigrationRunner,
    model::Configuration,
    service::{ConfigurationError, ConfigurationService},
    validator::{ConfigurationValidator, ValidationResult},
};

pub struct ConfigurationManager {
    service: ConfigurationService,
    validator: ConfigurationValidator,
    migrations: MigrationRunner,
    current: Option<Configuration>,
}
impl ConfigurationManager {
    pub fn new(service: ConfigurationService) -> Self {
        Self {
            service,
            validator: ConfigurationValidator,
            migrations: MigrationRunner::new()
                .with_migration(Box::new(super::migrations::FixPackagedAppsMigration)),
            current: None,
        }
    }
    pub fn load(&mut self) -> Result<Configuration, ConfigurationError> {
        let mut document = self.service.read_document()?;
        if self
            .migrations
            .run_pending(&mut document)
            .map_err(ConfigurationError::InvalidConfiguration)?
        {
            self.service.write_document(&document)?;
        }
        let mut config = document.to_configuration();
        let mut packages = crate::window_discovery::packaged_apps::PackagedApps::default();
        let sanitizer =
            super::sanitizer::PackagedAppPathSanitizer::new(packages.installed().to_vec());
        if let Some(sanitized) = sanitizer.sanitize(&config) {
            self.service.write(&sanitized)?;
            config = sanitized;
        }
        self.current = Some(config.clone());
        Ok(config)
    }
    pub fn get(&mut self) -> Result<Configuration, ConfigurationError> {
        match &self.current {
            Some(config) => Ok(config.clone()),
            None => self.load(),
        }
    }
    pub fn validate(&self, config: &Configuration) -> ValidationResult {
        self.validator.validate(config)
    }
    pub fn update(&mut self, config: Configuration) -> Result<(), ConfigurationError> {
        let result = self.validator.validate(&config);
        if !result.is_success() {
            return Err(ConfigurationError::InvalidConfiguration(
                result.message.unwrap_or_default(),
            ));
        }
        self.service.write(&config)?;
        self.current = Some(config.clone());
        Ok(())
    }
}
