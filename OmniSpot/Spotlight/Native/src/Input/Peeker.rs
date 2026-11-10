use crate::switcher::AppSwitchResult;
use crate::window_discovery::application_window::ApplicationWindow;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::Performance::QueryPerformanceCounter;
use windows::Win32::System::Threading::*;

pub(crate) const PEEK_THRESHOLD_MS: u32 = 400;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PeekResult {
    pub previous_window: ApplicationWindow,
    pub target_handle: i64,
    pub target_was_minimized: bool,
    pub target_process_name: String,
    pub target_process_path: String,
    pub armed_at_tick: i64,
    pub is_dynamic: bool,
}

#[derive(Default)]
pub(crate) struct Peeker {
    timer: Option<PTP_TIMER>,
    active: Box<AtomicBool>,
    snapshot: Option<PeekResult>,
}

impl Peeker {
    pub fn arm(
        &mut self,
        previous_window: ApplicationWindow,
        result: AppSwitchResult,
        is_dynamic: bool,
    ) -> windows::core::Result<()> {
        self.cancel();
        let mut tick = 0;
        unsafe {
            QueryPerformanceCounter(&mut tick)?;
        }
        self.snapshot = Some(PeekResult {
            previous_window,
            target_handle: result.handle,
            target_was_minimized: result.state == 2,
            target_process_name: result.process_name,
            target_process_path: result.process_path,
            armed_at_tick: tick,
            is_dynamic,
        });
        let timer = unsafe {
            CreateThreadpoolTimer(
                Some(activate),
                Some((&*self.active as *const AtomicBool).cast_mut().cast()),
                None,
            )?
        };
        self.timer = Some(timer);
        let due = (-(PEEK_THRESHOLD_MS as i64 * 10_000)) as u64;
        let due = FILETIME {
            dwLowDateTime: due as u32,
            dwHighDateTime: (due >> 32) as u32,
        };
        unsafe {
            SetThreadpoolTimer(timer, Some(&due), 0, 0);
        }
        Ok(())
    }

    pub fn try_finish(&mut self) -> Option<PeekResult> {
        let result = if self.active.load(Ordering::Acquire) {
            self.snapshot.clone()
        } else {
            None
        };
        self.cancel();
        result
    }

    pub fn cancel(&mut self) {
        if let Some(timer) = self.timer.take() {
            // Drain callbacks before resetting or dropping their state.
            unsafe {
                SetThreadpoolTimer(timer, None, 0, 0);
                WaitForThreadpoolTimerCallbacks(timer, true);
                CloseThreadpoolTimer(timer);
            }
        }
        self.active.store(false, Ordering::Release);
        self.snapshot = None;
    }
}

impl Drop for Peeker {
    fn drop(&mut self) {
        self.cancel();
    }
}

unsafe extern "system" fn activate(
    _: PTP_CALLBACK_INSTANCE,
    context: *mut core::ffi::c_void,
    _: PTP_TIMER,
) {
    (*(context as *const AtomicBool)).store(true, Ordering::Release);
}

#[cfg(test)]
#[path = "../tests/Input/PeekerTests.rs"]
mod tests;
