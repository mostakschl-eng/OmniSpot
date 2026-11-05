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
    listeners: Vec<Box<dyn Fn(&Configuration)>>,
}
impl ConfigurationManager {
    pub fn new(service: ConfigurationService) -> Self {
        Self {
            service,
            validator: ConfigurationValidator,
            migrations: MigrationRunner::new()
                .with_migration(Box::new(super::migrations::FixPackagedAppsMigration)),
            current: None,
            listeners: Vec::new(),
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
        let config = document.to_configuration();
        self.current = Some(config.clone());
        for listener in &self.listeners {
            listener(&config);
        }
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
        for listener in &self.listeners {
            listener(&config);
        }
        Ok(())
    }
    pub fn on_changed(&mut self, listener: impl Fn(&Configuration) + 'static) {
        self.listeners.push(Box::new(listener));
    }
}
