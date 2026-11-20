pub(crate) mod migrations;
pub(crate) mod storage;
pub(crate) mod app_theme_setting;
pub(crate) mod application_type;
pub(crate) mod configuration;
pub(crate) mod configuration_service;
pub(crate) mod configuration_manager;
pub(crate) mod configuration_validator;
pub(crate) mod packaged_app_path_sanitizer;

#[cfg(test)]
#[path = "../tests/configuration/storage_tests.rs"]
mod storage_tests;

#[allow(unused_imports)]
pub(crate) use configuration_validator::{
    ConfigurationValidator, ValidationResult, ValidationResultStatus, VALID_MODIFIERS,
};
#[allow(unused_imports)]
pub(crate) use packaged_app_path_sanitizer::{
    IPackagedAppsService, PackagedAppInfo, PackagedAppPathSanitizer,
};
