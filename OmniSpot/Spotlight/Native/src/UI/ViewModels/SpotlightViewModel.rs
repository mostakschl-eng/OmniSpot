use crate::runtime::{shell_open, start_process, RuntimeError};
use crate::window_discovery;

// Backend portion of SpotlightViewModel.cs; UI bindings remain in C#.
pub(crate) fn launch_target(target: &str, result_type: &str) -> Result<(), RuntimeError> {
    if is_packaged_search_target(target, result_type) {
        start_process("explorer.exe", Some(&format!("shell:AppsFolder\\{target}")))?;
    } else {
        shell_open(target)?;
    }
    Ok(())
}

fn is_packaged_search_target(target: &str, result_type: &str) -> bool {
    result_type == "App"
        && !window_discovery::starts_with_ignore_case(target, "ms-settings:")
        && !window_discovery::ends_with_ignore_case(target, ".exe")
        && !std::path::Path::new(target).is_file()
}

#[cfg(test)]
#[path = "../../tests/UI/ViewModels/SpotlightViewModelTests.rs"]
mod tests;
