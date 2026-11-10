use super::*;
use std::time::{Duration, Instant};

fn previous() -> ApplicationWindow {
    ApplicationWindow {
        handle: 10,
        process_id: 1,
        process_image_path: "previous.exe".into(),
        ..Default::default()
    }
}

fn target(path: &str) -> AppSwitchResult {
    AppSwitchResult {
        process_id: Some(2),
        process_path: path.into(),
        process_name: crate::runtime::file_name(path).to_string(),
        handle: 20,
        state: 2,
        needs_elevation: false,
        was_started: false,
    }
}

fn wait_until_active(peeker: &Peeker) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !peeker.active.load(Ordering::Acquire) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(peeker.active.load(Ordering::Acquire));
}

#[test]
fn unarmed_and_early_release_return_no_peek() {
    let mut peeker = Peeker::default();
    assert!(peeker.try_finish().is_none());
    peeker.arm(previous(), target("target.exe"), false).unwrap();
    assert!(peeker.try_finish().is_none());
    assert!(peeker.timer.is_none());
}

#[test]
fn timer_uses_stopwatch_clock_and_returns_complete_snapshot_once() {
    let mut peeker = Peeker::default();
    let mut before = 0;
    let mut after = 0;
    unsafe {
        QueryPerformanceCounter(&mut before).unwrap();
    }
    peeker
        .arm(previous(), target(r"C:\Apps\Target.exe"), true)
        .unwrap();
    unsafe {
        QueryPerformanceCounter(&mut after).unwrap();
    }
    wait_until_active(&peeker);
    assert!(Peeker::default().try_finish().is_none());
    let result = peeker.try_finish().unwrap();
    assert_eq!(result.previous_window.handle, 10);
    assert_eq!(result.target_handle, 20);
    assert!(result.target_was_minimized);
    assert_eq!(result.target_process_name, "Target.exe");
    assert_eq!(result.target_process_path, r"C:\Apps\Target.exe");
    assert!(result.is_dynamic);
    assert!((before..=after).contains(&result.armed_at_tick));
    assert!(peeker.try_finish().is_none());
}

#[test]
fn cancellation_drains_pending_callbacks_and_is_idempotent() {
    let mut peeker = Peeker::default();
    peeker.arm(previous(), target("target.exe"), false).unwrap();
    peeker.cancel();
    peeker.cancel();
    std::thread::sleep(Duration::from_millis(PEEK_THRESHOLD_MS as u64 + 50));
    assert!(peeker.try_finish().is_none());
}

#[test]
fn rearming_does_not_reuse_the_previous_timer_deadline() {
    let mut peeker = Peeker::default();
    peeker.arm(previous(), target("old.exe"), false).unwrap();
    std::thread::sleep(Duration::from_millis(250));
    peeker.arm(previous(), target("new.exe"), true).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    assert!(peeker.try_finish().is_none());
    peeker.arm(previous(), target("latest.exe"), false).unwrap();
    wait_until_active(&peeker);
    assert_eq!(
        peeker.try_finish().unwrap().target_process_name,
        "latest.exe"
    );
}
