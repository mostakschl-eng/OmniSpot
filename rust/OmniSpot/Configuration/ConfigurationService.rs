use super::{model::Configuration, storage::SettingsDocument};
use serde_json;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub enum ConfigurationError {
    Io(io::Error),
    Json(serde_json::Error),
    InvalidPath,
    InvalidConfiguration(String),
}
impl From<io::Error> for ConfigurationError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
impl From<serde_json::Error> for ConfigurationError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}
pub struct SettingsPath;
impl SettingsPath {
    pub fn resolve(executable_dir: &Path, portable: bool) -> Result<PathBuf, ConfigurationError> {
        if portable {
            return Ok(executable_dir.join("settings.json"));
        }
        let app_data = std::env::var_os("APPDATA").ok_or(ConfigurationError::InvalidPath)?;
        Ok(PathBuf::from(app_data)
            .join("OmniSpot")
            .join("settings.json"))
    }
}
pub struct ConfigurationService {
    path: PathBuf,
}
impl ConfigurationService {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn read(&self) -> Result<Configuration, ConfigurationError> {
        Ok(self.read_document()?.to_configuration())
    }
    pub fn read_document(&self) -> Result<SettingsDocument, ConfigurationError> {
        if !self.path.exists() {
            return Ok(SettingsDocument::from_configuration(
                &Configuration::default(),
            ));
        }
        let text = fs::read_to_string(&self.path)?;
        Ok(serde_json::from_str::<SettingsDocument>(&text)?)
    }
    pub fn write(&self, config: &Configuration) -> Result<(), ConfigurationError> {
        self.write_document(&SettingsDocument::from_configuration(config))
    }
    pub fn write_document(&self, document: &SettingsDocument) -> Result<(), ConfigurationError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(document)?;
        let temp = self.path.with_extension("json.tmp");
        fs::write(&temp, text)?;
        if self.path.exists() {
            fs::remove_file(&self.path)?;
        }
        fs::rename(temp, &self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_returns_defaults_when_file_is_missing() {
        let path =
            std::env::temp_dir().join(format!("omnispot-config-{}.json", std::process::id()));
        let _ = fs::remove_file(&path);

        let config = ConfigurationService::new(&path).read().unwrap();

        assert_eq!(config, Configuration::default());
    }

    #[test]
    fn write_then_read_preserves_configuration() {
        let path =
            std::env::temp_dir().join(format!("omnispot-config-{}.json", std::process::id()));
        let service = ConfigurationService::new(&path);
        let config = Configuration {
            theme: super::super::model::AppThemeSetting::Dark,
            ..Configuration::default()
        };

        service.write(&config).unwrap();

        assert_eq!(service.read().unwrap(), config);
        let _ = fs::remove_file(path);
    }
}
