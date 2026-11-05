use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::Instant,
};

struct LoggerState {
    file: Option<File>,
    debug: bool,
    trace: bool,
}

static LOGGER: OnceLock<Mutex<LoggerState>> = OnceLock::new();

pub(crate) fn initialize(debug: bool, trace: bool) {
    let path = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("OmniSpot")
        .join("logs");
    let _ = fs::create_dir_all(&path);
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path.join("omnispot.log"))
        .ok();
    let state = LOGGER.get_or_init(|| {
        Mutex::new(LoggerState {
            file: None,
            debug: false,
            trace: false,
        })
    });
    if let Ok(mut state) = state.lock() {
        state.file = file;
        state.debug = debug;
        state.trace = trace;
    }
    info("logging initialized");
}

pub(crate) fn info(message: impl AsRef<str>) {
    write("INFO", message.as_ref(), true);
}

pub(crate) fn debug(message: impl AsRef<str>) {
    write("DEBUG", message.as_ref(), false);
}

pub(crate) fn trace(message: impl AsRef<str>) {
    let Some(logger) = LOGGER.get() else { return };
    let enabled = logger.lock().map(|state| state.trace).unwrap_or(false);
    if enabled {
        write("TRACE", message.as_ref(), true);
    }
}

pub(crate) struct TimingScope {
    operation: String,
    started: Instant,
}

pub(crate) fn measure_time(operation: impl Into<String>) -> TimingScope {
    TimingScope {
        operation: operation.into(),
        started: Instant::now(),
    }
}

impl Drop for TimingScope {
    fn drop(&mut self) {
        debug(format!(
            "{} completed in {} ms",
            self.operation,
            self.started.elapsed().as_millis()
        ));
    }
}

fn write(level: &str, message: &str, force: bool) {
    let Some(logger) = LOGGER.get() else { return };
    let Ok(mut logger) = logger.lock() else {
        return;
    };
    if !force && !logger.debug {
        return;
    }
    if let Some(file) = &mut logger.file {
        let _ = writeln!(file, "[{level}] {message}");
        let _ = file.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::measure_time;

    #[test]
    fn timing_scope_can_be_created_and_dropped() {
        let _scope = measure_time("test operation");
    }
}
