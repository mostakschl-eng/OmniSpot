pub(crate) mod migrations;
pub(crate) mod storage;
#[path = "AppThemeSetting.rs"]
pub(crate) mod app_theme_setting;
#[path = "ApplicationType.rs"]
pub(crate) mod application_type;
#[path = "Configuration.rs"]
pub(crate) mod configuration;
#[path = "ConfigurationService.rs"]
pub(crate) mod configuration_service;
#[path = "ConfigurationManager.rs"]
pub(crate) mod configuration_manager;
#[path = "ConfigurationValidator.rs"]
pub(crate) mod configuration_validator;
#[path = "PackagedAppPathSanitizer.rs"]
pub(crate) mod packaged_app_path_sanitizer;

#[cfg(test)]
#[path = "../tests/Configuration/StorageTests.rs"]
mod storage_tests;

#[allow(unused_imports)]
pub(crate) use configuration_validator::{
    ConfigurationValidator, ValidationResult, ValidationResultStatus, VALID_MODIFIERS,
};
#[allow(unused_imports)]
pub(crate) use packaged_app_path_sanitizer::{
    IPackagedAppsService, PackagedAppInfo, PackagedAppPathSanitizer,
};
