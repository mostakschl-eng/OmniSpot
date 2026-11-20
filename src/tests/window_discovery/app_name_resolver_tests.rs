use super::*;

struct Cache(&'static str);
impl IAppRegistryCache for Cache {
    fn get_display_name(&self, name: &str, path: Option<&str>) -> String {
        assert_eq!(name, "different.exe");
        assert_eq!(path, Some(r"C:\different.exe"));
        self.0.into()
    }
}

#[test]
fn resolved_display_name_maps_to_wpf_letter_keys() {
    for (name, expected) in [
        ("alpha", Some(A)),
        ("Zulu", Some(A + 25)),
        ("1app", None),
        ("", None),
        ("😀app", None),
    ] {
        let resolver = AppNameResolver {
            app_registry_cache: Cache(name),
        };
        assert_eq!(
            resolver.get_dynamic_key("different.exe", r"C:\different.exe"),
            expected
        );
    }
}
