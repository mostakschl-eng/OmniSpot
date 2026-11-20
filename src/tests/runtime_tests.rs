use super::*;

#[test]
fn process_command_line_preserves_quoted_paths_and_raw_arguments() {
    assert_eq!(
        command_line(r" C:\Program Files\App.exe ", None),
        r#""C:\Program Files\App.exe""#
    );
    assert_eq!(
        command_line(r#""C:\Program Files\App.exe""#, Some("--test value")),
        r#""C:\Program Files\App.exe" --test value"#
    );
    assert_eq!(
        command_line("explorer.exe", Some(r"shell:AppsFolder\Family!App")),
        r#""explorer.exe" shell:AppsFolder\Family!App"#
    );
}

#[test]
fn create_process_reports_windows_errors_and_launches_controlled_child() {
    let error = start_process(r"Z:\missing-omnispot-process.exe", None).unwrap_err();
    assert!(error.win32_error.is_some());
    let cmd = std::path::PathBuf::from(std::env::var("WINDIR").unwrap())
        .join("System32")
        .join("cmd.exe");
    start_process(cmd.to_str().unwrap(), Some("/d /c exit 0")).unwrap();
}
