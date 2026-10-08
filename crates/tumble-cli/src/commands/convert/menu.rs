//! Menu mode: what runs when a right-click menu item is chosen
//! (`tumble convert --to <fmt> <files>`; on Windows through
//! `tumblew.exe`, which starts `tumble.exe convert ...` with no window).
//!
//! 1. Every process for the same target joins one batch (`instance`).
//! 2. The leader converts the batch with no console output; the progress
//!    dialog appears if it takes longer than about a second, and its Cancel
//!    button stops the job. SIGTERM (the stop button of a running macOS
//!    Quick Action) cancels it too.
//! 3. One notification sums it up; clicking it opens the output folder
//!    where the system allows that.
//! 4. Failures go to `<date>.log` in `tumble_core::config::log_dir()`
//!    (`%LOCALAPPDATA%\Tumble\logs` on Windows).
//!
//! `TUMBLE_NO_UI=1` skips the dialog and the toast (used by tests).

use super::report::{Mode, Observer, Reporter};
use super::{batch, inputs, setup};
use crate::args::ConvertArgs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tumble_core::{CancelToken, brand};
use tumble_shell::instance::{self, Role};
use tumble_shell::progress::{self, State};

/// Invocations that arrive this close together join one batch.
const BATCH_WINDOW: Duration = Duration::from_millis(500);
/// The progress dialog appears only for jobs longer than this.
const DIALOG_DELAY: Duration = Duration::from_secs(1);

pub(super) fn ui_enabled() -> bool {
    std::env::var_os(brand::env_var("NO_UI")).is_none_or(|v| v.is_empty() || v == "0")
}

/// Feeds the shell progress dialog.
struct DialogObserver(Arc<State>);

fn short_name(p: &Path) -> String {
    p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned())
}

impl Observer for DialogObserver {
    fn started(&self, input: &Path) {
        self.0.running.lock().unwrap().push((short_name(input), 0.0));
    }
    fn progress(&self, input: &Path, fraction: f32) {
        let name = short_name(input);
        if let Some(e) = self.0.running.lock().unwrap().iter_mut().find(|(n, _)| *n == name) {
            e.1 = fraction;
        }
    }
    fn finished(&self, input: &Path) {
        let name = short_name(input);
        let mut running = self.0.running.lock().unwrap();
        if let Some(i) = running.iter().position(|(n, _)| *n == name) {
            running.remove(i);
        }
        self.0.done.fetch_add(1, Ordering::Relaxed);
    }
}

pub fn run_menu(args: ConvertArgs) -> u8 {
    let setup = match setup::from_args(&args) {
        Ok(s) => s,
        Err((msg, code)) => {
            log(&[format!("bad menu arguments: {msg}")]);
            return code;
        }
    };
    let paths: Vec<PathBuf> =
        args.inputs.iter().map(|p| std::path::absolute(p).unwrap_or_else(|_| p.clone())).collect();
    let key = format!("{}-{}", setup.target.id, args.preset.as_deref().unwrap_or(""));
    let paths = match instance::gather(&key, paths, BATCH_WINDOW) {
        Ok(Role::Joined) => return crate::exit::OK,
        Ok(Role::Leader(all)) => all,
        Err(e) => {
            log(&[format!("could not batch menu invocations: {e}")]);
            args.inputs.clone()
        }
    };

    let registry = tumble_engines::default_registry();
    let plan =
        inputs::collect(&paths, args.recursive, setup.out.as_deref(), setup.target.id, &registry);
    let state = Arc::new(State::default());
    state.total.store(plan.jobs.len() + plan.missing.len(), Ordering::Relaxed);
    let observer = DialogObserver(state.clone());
    let reporter = Reporter::new(Mode::Silent, Some(&observer));
    let cancel = CancelToken::new();
    #[cfg(unix)]
    {
        let cancel = cancel.clone();
        let _ = ctrlc::set_handler(move || cancel.cancel());
    }

    let ui = ui_enabled();
    let dialog = ui.then(|| {
        let n = plan.jobs.len();
        let line = format!(
            "Converting {n} {} to {}",
            if n == 1 { "file" } else { "files" },
            setup.target.name
        );
        progress::spawn(
            brand::APP_NAME.to_string(),
            line,
            state.clone(),
            cancel.clone(),
            DIALOG_DELAY,
        )
    });
    batch::execute(&plan, &setup, args.overwrite, &registry, &reporter, &cancel);
    state.finished.store(true, Ordering::SeqCst);
    if let Some(d) = dialog {
        let _ = d.join();
    }

    let failures = reporter.failures.lock().unwrap().clone();
    let log_path = if failures.is_empty() {
        None
    } else {
        log(&failures
            .iter()
            .map(|(input, err)| format!("{} -> {}: {err}", input.display(), setup.target.id))
            .collect::<Vec<_>>())
    };
    if ui {
        let converted = reporter.tally.converted.load(Ordering::Relaxed);
        let outputs = reporter.outputs.lock().unwrap().clone();
        toast(
            converted,
            failures.len(),
            cancel.is_cancelled(),
            setup.target.name,
            &outputs,
            log_path.as_deref(),
        );
    }
    batch::exit_code(&reporter)
}

fn toast(
    converted: usize,
    failed: usize,
    cancelled: bool,
    target: &str,
    outputs: &[PathBuf],
    log_file: Option<&Path>,
) {
    let files = |n: usize| if n == 1 { "file" } else { "files" };
    let title = match (cancelled, failed) {
        (true, _) => format!("Cancelled after {converted} {}", files(converted)),
        (false, 0) => format!("{converted} {} converted", files(converted)),
        (false, f) => format!("{converted} converted, {f} failed"),
    };
    let folder = outputs.first().and_then(|p| p.parent()).map(Path::to_path_buf);
    let detail = match (&folder, log_file) {
        (Some(dir), None) => format!("to {target} in {}", dir.display()),
        (Some(dir), Some(log)) => {
            format!("to {target} in {}. Details: {}", dir.display(), log.display())
        }
        (None, Some(log)) => format!("Details: {}", log.display()),
        (None, None) => String::new(),
    };
    // With nothing converted, the click opens the log folder instead.
    let open = folder.or_else(|| log_file.and_then(Path::parent).map(Path::to_path_buf));
    if let Err(e) = tumble_shell::toast::show(&title, &detail, open.as_deref()) {
        log(&[format!("could not show the notification: {e}")]);
    }
}

/// Appends timestamped lines to today's log; returns its path.
pub(super) fn log(lines: &[String]) -> Option<PathBuf> {
    let dir = tumble_core::config::log_dir()?;
    std::fs::create_dir_all(&dir).ok()?;
    let (y, mo, d, h, mi, s) = tumble_shell::local_now();
    let path = dir.join(format!("{y:04}-{mo:02}-{d:02}.log"));
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&path).ok()?;
    for line in lines {
        let _ = writeln!(file, "{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}  {line}");
    }
    Some(path)
}
