use super::cli_builder::CliBuilder;
use crate::startup::auto_start::AutoStart;

pub(crate) fn cli_builder() -> CliBuilder {
    let mut builder = CliBuilder::default();
    builder
        .add_command("--log-all-windows", "Log all windows to log file", || {
            Err("WindowDiscovery is not ported yet; no windows were logged.".into())
        })
        .add_command(
            "--enable-auto-start",
            "Add application to system Startup",
            || AutoStart::create_shortcut().map_err(|error| error.to_string()),
        )
        .add_flag_option("--debug", "Enable debug logging", |options| {
            options.enable_debug_logging = true
        })
        .add_flag_option("--trace", "Enable trace logging", |options| {
            options.enable_trace_logging = true
        });
    builder
}
