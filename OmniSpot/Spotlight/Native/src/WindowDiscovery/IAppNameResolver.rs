pub(crate) trait IAppNameResolver {
    fn get_dynamic_key(&self, process_name: &str, process_path: &str) -> Option<u32>;
}
