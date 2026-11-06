#[derive(Default)]
pub(crate) struct CliOptions {
    pub enable_debug_logging: bool,
    pub enable_trace_logging: bool,
    pub import_legacy: Option<String>,
}
