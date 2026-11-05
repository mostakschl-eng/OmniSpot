#[path = "Controls/mod.rs"]
pub(crate) mod controls;
#[path = "Converters/mod.rs"]
pub(crate) mod converters;
#[path = "Pages/mod.rs"]
pub(crate) mod pages;
#[path = "ViewModels/mod.rs"]
pub(crate) mod view_models;
#[path = "Windows/mod.rs"]
pub(crate) mod windows;

pub use windows::settings::SettingsWindow;
