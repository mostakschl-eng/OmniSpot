pub(crate) trait IAppRegistryCache {
    fn get_display_name(&self, process_name: &str, process_path: Option<&str>) -> String;
}
