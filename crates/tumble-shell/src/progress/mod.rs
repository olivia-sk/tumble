//! The progress dialog for right-click jobs. It appears only if the job runs
//! longer than a delay (about a second), and its Cancel button cancels the
//! job.
//!
//! - Windows: the standard shell progress dialog (`IProgressDialog`)
//! - Linux: `zenity --progress`, or `kdialog --progressbar` on KDE
//! - macOS: none of its own; Finder shows a running Quick Action in the
//!   menu bar, and its stop button ends the job (see `tumble pick`)

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::spawn;
#[cfg(windows)]
pub use windows::spawn;

/// What the job reports and the dialog shows.
#[derive(Default)]
pub struct State {
    pub total: AtomicUsize,
    pub done: AtomicUsize,
    /// Fraction of each running file, by file name.
    pub running: Mutex<Vec<(String, f32)>>,
    pub finished: AtomicBool,
}

impl State {
    /// (done, total) in thousandths of a file.
    #[cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]
    fn overall(&self) -> (u64, u64) {
        let total = self.total.load(Ordering::Relaxed).max(1) as u64 * 1000;
        let partial: f32 = self.running.lock().unwrap().iter().map(|(_, f)| f).sum();
        let done = self.done.load(Ordering::Relaxed) as u64 * 1000 + (partial * 1000.0) as u64;
        (done.min(total), total)
    }

    /// The first file still running.
    #[cfg_attr(not(any(windows, target_os = "linux")), allow(dead_code))]
    fn current(&self) -> String {
        self.running.lock().unwrap().first().map(|(n, _)| n.clone()).unwrap_or_default()
    }
}

/// No dialog of its own on macOS: the thread only waits for the job.
#[cfg(not(any(windows, target_os = "linux")))]
pub fn spawn(
    _title: String,
    _line: String,
    _state: std::sync::Arc<State>,
    _cancel: tumble_core::CancelToken,
    _delay: std::time::Duration,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(|| {})
}

/// Waits `delay`, returning false if the job finished first.
#[cfg(target_os = "linux")]
fn wait_delay(state: &State, delay: std::time::Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < delay {
        if state.finished.load(Ordering::SeqCst) {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    true
}
