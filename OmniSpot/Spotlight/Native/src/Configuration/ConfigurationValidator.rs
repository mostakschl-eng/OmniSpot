use super::application_type::ApplicationType;
use super::configuration::{ApplicationConfiguration, Configuration};
use crate::input::keyboard_input::{
    A, APPS, CAPITAL, LEFT_ALT, LEFT_CTRL, LEFT_SHIFT, LWIN, RIGHT_ALT, RIGHT_CTRL, RIGHT_SHIFT, Z,
};

pub(crate) const VALID_MODIFIERS: [u32; 9] = [
    LEFT_CTRL,
    LEFT_ALT,
    LEFT_SHIFT,
    LWIN,
    RIGHT_CTRL,
    RIGHT_ALT,
    APPS,
    RIGHT_SHIFT,
    CAPITAL,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ValidationResultStatus {
    Success,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ValidationResult {
    pub status: ValidationResultStatus,
    pub process: Option<String>,
    pub message: Option<String>,
}

impl ValidationResult {
    pub fn success() -> Self {
        Self {
            status: ValidationResultStatus::Success,
            process: None,
            message: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            status: ValidationResultStatus::Error,
            process: None,
            message: Some(message.into()),
        }
    }

    pub fn error_with_process(process: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status: ValidationResultStatus::Error,
            process: Some(process.into()),
            message: Some(message.into()),
        }
    }

    pub fn is_error(&self) -> bool {
        self.status == ValidationResultStatus::Error
    }
}

pub(crate) struct ConfigurationValidator;

impl ConfigurationValidator {
    pub fn validate(configuration: &Configuration) -> ValidationResult {
        if !VALID_MODIFIERS.contains(&configuration.modifier) {
            return ValidationResult::error(format!(
                "Invalid modifier key ({}) - must be one of: {:?}",
                configuration.modifier, VALID_MODIFIERS
            ));
        }

        if !VALID_MODIFIERS.contains(&configuration.spotlight_modifier) {
            return ValidationResult::error(format!(
                "Invalid spotlight modifier key ({}) - must be one of: {:?}",
                configuration.spotlight_modifier, VALID_MODIFIERS
            ));
        }

        for app in &configuration.applications {
            let result = Self::validate_application(app);
            if result.is_error() {
                return result;
            }
        }

        ValidationResult::success()
    }

    pub fn validate_application(app: &ApplicationConfiguration) -> ValidationResult {
        if app.application_type == ApplicationType::Packaged {
            if app
                .aumid
                .as_deref()
                .map(|s| s.trim().is_empty())
                .unwrap_or(true)
            {
                return ValidationResult::error_with_process(
                    app.process_name(),
                    "Packaged application AUMID must be set. Re-add the application to fix it.",
                );
            }
        } else {
            let path = app.process_path.trim();
            if path.is_empty() || !std::path::Path::new(path).is_file() {
                return ValidationResult::error_with_process(
                    app.process_name(),
                    "Application process path missing or invalid. Re-add the application to fix it.",
                );
            }
        }

        if app.key < A || app.key > Z {
            return ValidationResult::error_with_process(
                app.process_name(),
                "Application key invalid - it should be a single letter",
            );
        }

        ValidationResult::success()
    }
}

#[cfg(test)]
#[path = "../tests/Configuration/ConfigurationValidatorTests.rs"]
mod tests;
