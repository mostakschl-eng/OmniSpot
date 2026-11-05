pub(crate) struct AboutViewModel;

impl AboutViewModel {
    pub(crate) const APP_NAME: &'static str = "OmniSpot";
    pub(crate) const APP_WEBSITE: &'static str = "https://app-switcher.com";
    pub(crate) const RUNTIME: &'static str = "Rust";

    pub(crate) fn app_version() -> String {
        format!("Version {}", env!("CARGO_PKG_VERSION"))
    }
}
