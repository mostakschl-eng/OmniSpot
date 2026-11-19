use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[test]
fn handle_returns_no_args_when_empty() {
    let builder = CliBuilder::new();
    let handler = CliHandler::new(builder);
    let mut options = CliOptions::default();

    let result = handler.handle(&[], &mut options);
    assert_eq!(result, CliResult::NoArgs);
}

#[test]
fn handle_returns_help_requested_for_help_flag() {
    let builder = CliBuilder::new();
    let handler = CliHandler::new(builder);
    let mut options = CliOptions::default();

    assert_eq!(
        handler.handle(&["--help".to_string()], &mut options),
        CliResult::HelpRequested
    );
    assert_eq!(
        handler.handle(&["-h".to_string()], &mut options),
        CliResult::HelpRequested
    );
}

#[test]
fn handle_executes_flag_options() {
    let builder = CliBuilder::new().add_flag_option("--debug", "Enable debug", |opts| {
        opts.enable_debug_logging = true;
    });

    let handler = CliHandler::new(builder);
    let mut options = CliOptions::default();

    let result = handler.handle(&["--debug".to_string()], &mut options);
    assert_eq!(result, CliResult::Handled);
    assert!(options.enable_debug_logging);
}

#[test]
fn handle_executes_valued_options() {
    let captured = Arc::new(std::sync::Mutex::new(String::new()));
    let c = captured.clone();

    let builder = CliBuilder::new().add_valued_option("--config", "Set config path", move |_opts, val| {
        *c.lock().unwrap() = val.to_string();
    });

    let handler = CliHandler::new(builder);
    let mut options = CliOptions::default();

    let result = handler.handle(
        &["--config".to_string(), "custom.json".to_string()],
        &mut options,
    );
    assert_eq!(result, CliResult::Handled);
    assert_eq!(*captured.lock().unwrap(), "custom.json");
}

#[test]
fn handle_errors_when_valued_option_missing_value() {
    let builder = CliBuilder::new().add_valued_option("--config", "Set config path", |_opts, _val| {});

    let handler = CliHandler::new(builder);
    let mut options = CliOptions::default();

    let result = handler.handle(&["--config".to_string()], &mut options);
    match result {
        CliResult::Error(msg) => assert!(msg.contains("requires a value")),
        _ => panic!("Expected error result"),
    }
}

#[test]
fn handle_executes_matched_commands() {
    let executed = Arc::new(AtomicBool::new(false));
    let ex = executed.clone();

    let builder = CliBuilder::new().add_command("run-migration", "Run migrations", move || {
        ex.store(true, Ordering::SeqCst);
    });

    let handler = CliHandler::new(builder);
    let mut options = CliOptions::default();

    let result = handler.handle(&["run-migration".to_string()], &mut options);
    assert_eq!(result, CliResult::Handled);
    assert!(executed.load(Ordering::SeqCst));
}
