//! The standard Windows shell progress dialog (`IProgressDialog`) for
//! right-click jobs. It appears only if the job runs longer than a delay
//! (about a second), and its Cancel button cancels the job.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tumble_core::CancelToken;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize,
};
use windows::Win32::UI::Shell::{
    IProgressDialog, PROGDLG_AUTOTIME, PROGDLG_NOMINIMIZE, PROGDLG_NORMAL,
};
use windows::core::{GUID, HSTRING};

/// CLSID_ProgressDialog from shlobj.h, which the bindings do not export.
const PROGRESS_DIALOG: GUID = GUID::from_u128(0xF8383852_FCD3_11D1_A6B9_006097DF5BD4);

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
    fn overall(&self) -> (u64, u64) {
        let total = self.total.load(Ordering::Relaxed).max(1) as u64 * 1000;
        let partial: f32 = self.running.lock().unwrap().iter().map(|(_, f)| f).sum();
        let done = self.done.load(Ordering::Relaxed) as u64 * 1000 + (partial * 1000.0) as u64;
        (done.min(total), total)
    }

    fn current(&self) -> String {
        self.running.lock().unwrap().first().map(|(n, _)| n.clone()).unwrap_or_default()
    }
}

/// Shows the dialog on its own thread once `delay` has passed, until
/// `state.finished` is set. Cancel in the dialog cancels `cancel`.
pub fn spawn(
    title: String,
    line: String,
    state: Arc<State>,
    cancel: CancelToken,
    delay: Duration,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let start = Instant::now();
        while start.elapsed() < delay {
            if state.finished.load(Ordering::SeqCst) {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        // SAFETY: COM is initialised for this thread and torn down at the end;
        // the dialog object lives only inside this block.
        unsafe {
            if CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_err() {
                return;
            }
            if let Ok(dialog) =
                CoCreateInstance::<_, IProgressDialog>(&PROGRESS_DIALOG, None, CLSCTX_INPROC_SERVER)
            {
                let _ = dialog.SetTitle(&HSTRING::from(title));
                let _ = dialog.SetLine(1, &HSTRING::from(line), false, None);
                let _ = dialog.SetCancelMsg(&HSTRING::from("Cancelling..."), None);
                let flags = PROGDLG_NORMAL | PROGDLG_AUTOTIME | PROGDLG_NOMINIMIZE;
                if dialog.StartProgressDialog(None, None, flags, None).is_ok() {
                    while !state.finished.load(Ordering::SeqCst) {
                        if dialog.HasUserCancelled().as_bool() {
                            cancel.cancel();
                        }
                        let (done, total) = state.overall();
                        let _ = dialog.SetProgress64(done, total);
                        let _ = dialog.SetLine(2, &HSTRING::from(state.current()), true, None);
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    let _ = dialog.StopProgressDialog();
                }
            }
            CoUninitialize();
        }
    })
}
