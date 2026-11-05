use super::model::*;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationResult {
    pub process: Option<String>,
    pub message: Option<String>,
}
impl ValidationResult {
    pub fn success() -> Self {
        Self {
            process: None,
            message: None,
        }
    }
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            process: None,
            message: Some(message.into()),
        }
    }
    pub fn app_error(app: &ApplicationConfiguration, message: impl Into<String>) -> Self {
        Self {
            process: Some(app.process_name().to_string()),
            message: Some(message.into()),
        }
    }
    pub fn is_success(&self) -> bool {
        self.message.is_none()
    }
}

#[derive(Default)]
pub struct ConfigurationValidator;
impl ConfigurationValidator {
    pub fn validate(&self, config: &Configuration) -> ValidationResult {
        if !config.modifier.is_modifier() {
            return ValidationResult::error(format!(
                "Invalid modifier key ({:?})",
                config.modifier
            ));
        }
        if !config.spotlight_modifier.is_modifier() {
            return ValidationResult::error(format!(
                "Invalid spotlight modifier key ({:?})",
                config.spotlight_modifier
            ));
        }
        for app in &config.applications {
            let result = self.validate_application(app);
            if !result.is_success() {
                return result;
            }
        }
        ValidationResult::success()
    }
    pub fn validate_application(&self, app: &ApplicationConfiguration) -> ValidationResult {
        if app.application_type == ApplicationType::Packaged {
            if app.aumid.as_deref().unwrap_or("").trim().is_empty() {
                return ValidationResult::app_error(
                    app,
                    "Packaged application AUMID must be set. Re-add the application to fix it.",
                );
            }
        } else if !Path::new(&app.process_path).is_file() {
            return ValidationResult::app_error(
                app,
                "Application process path missing or invalid. Re-add the application to fix it.",
            );
        }
        if !app.key.is_letter() {
            return ValidationResult::app_error(
                app,
                "Application key invalid - it should be a single letter",
            );
        }
        ValidationResult::success()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(key: Key, path: &str) -> ApplicationConfiguration {
        ApplicationConfiguration {
            key,
            process_path: path.to_string(),
            cycle_mode: CycleMode::NextApp,
            start_if_not_running: false,
            application_type: ApplicationType::Win32,
            aumid: None,
        }
    }

    #[test]
    fn rejects_non_modifier_primary_key() {
        let config = Configuration {
            modifier: Key::A,
            ..Configuration::default()
        };

        assert!(
            !ConfigurationValidator::default()
                .validate(&config)
                .is_success()
        );
    }

    #[test]
    fn accepts_packaged_application_without_a_real_executable_path() {
        let config = Configuration {
            applications: vec![ApplicationConfiguration {
                application_type: ApplicationType::Packaged,
                aumid: Some("Contoso.App!App".to_string()),
                ..app(Key::A, r"C:\missing\app.exe")
            }],
            ..Configuration::default()
        };

        assert!(
            ConfigurationValidator::default()
                .validate(&config)
                .is_success()
        );
    }

    #[test]
    fn rejects_missing_win32_executable() {
        let config = Configuration {
            applications: vec![app(Key::A, r"C:\missing\app.exe")],
            ..Configuration::default()
        };

        let result = ConfigurationValidator::default().validate(&config);

        assert_eq!(result.process.as_deref(), Some("app.exe"));
        assert!(result.message.unwrap().contains("missing or invalid"));
    }
}
