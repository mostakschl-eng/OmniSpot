use super::i_app_name_resolver::IAppNameResolver;
use crate::input::keyboard_input::A;
use crate::stats::i_app_registry_cache::IAppRegistryCache;

pub(crate) struct AppNameResolver<C: IAppRegistryCache> {
    pub app_registry_cache: C,
}

impl<C: IAppRegistryCache> IAppNameResolver for AppNameResolver<C> {
    fn get_dynamic_key(&self, process_name: &str, process_path: &str) -> Option<u32> {
        let display_name = self
            .app_registry_cache
            .get_display_name(process_name, Some(process_path));
        get_dynamic_key(&display_name).map(|offset| A + u32::from(offset))
    }
}

pub(super) fn get_dynamic_key(display_name: &str) -> Option<u16> {
    let first = char::from_u32(display_name.encode_utf16().next()? as u32)?;
    let first = super::upper_invariant(first) as u32;
    match first {
        65..=90 => Some((first - 65) as u16),
        _ => None,
    }
}

#[cfg(test)]
#[path = "../tests/WindowDiscovery/AppNameResolverTests.rs"]
mod tests;
