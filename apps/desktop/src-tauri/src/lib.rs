//! Tumble's desktop window (PRD phase 6): drag and drop, a format picker
//! and a queue. The window calls the engines directly through these
//! commands; the logic lives in `jobs.rs`.

mod jobs;

use jobs::{FileInfo, PresetInfo, Session, Target};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use tauri::{AppHandle, Emitter, Manager, State};
use tumble_core::{CancelToken, Progress};

#[derive(Serialize)]
struct Inspected {
    files: Vec<FileInfo>,
    /// Targets every listed file can reach.
    common: Vec<Target>,
}

#[derive(Serialize)]
struct EngineInfo {
    name: &'static str,
    available: bool,
    detail: String,
}

#[derive(Serialize, Clone)]
struct ProgressEvent {
    job: u32,
    fraction: f32,
}

/// Sends a job's progress to the window, at most once per percent.
struct EmitProgress {
    app: AppHandle,
    job: u32,
    last: AtomicU32,
}

impl Progress for EmitProgress {
    fn update(&self, fraction: f32) {
        let pct = (fraction.clamp(0.0, 1.0) * 100.0) as u32;
        if self.last.swap(pct, Ordering::Relaxed) != pct {
            let _ = self.app.emit("job-progress", ProgressEvent { job: self.job, fraction });
        }
    }
}

#[tauri::command]
fn inspect(paths: Vec<PathBuf>, session: State<'_, Session>) -> Inspected {
    let files = jobs::inspect(&session, &paths);
    let common = jobs::common_targets(&files);
    Inspected { files, common }
}

#[tauri::command]
fn presets() -> Vec<PresetInfo> {
    jobs::preset_list()
}

#[tauri::command]
fn engines() -> Vec<EngineInfo> {
    tumble_engines::engine_report()
        .into_iter()
        .map(|e| EngineInfo { name: e.name, available: e.available, detail: e.detail })
        .collect()
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn convert(
    app: AppHandle,
    job: u32,
    path: PathBuf,
    to: String,
    quality: Option<u8>,
    resize: Option<String>,
    preset: Option<String>,
    out_dir: Option<PathBuf>,
) -> Result<Vec<String>, String> {
    let options = jobs::options(quality, resize.as_deref(), preset.as_deref())?;
    let cancel = CancelToken::new();
    app.state::<Session>().running.lock().unwrap().insert(job, cancel.clone());
    // Conversions block (FFmpeg, LibreOffice): run them on a blocking
    // thread so the window stays responsive.
    let worker = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let session = worker.state::<Session>();
        let progress = EmitProgress { app: worker.clone(), job, last: AtomicU32::new(u32::MAX) };
        jobs::convert(&session, &path, &to, &options, out_dir.as_deref(), &progress, &cancel)
    })
    .await
    .unwrap_or_else(|_| Err("the conversion crashed".to_string()));
    app.state::<Session>().running.lock().unwrap().remove(&job);
    result.map(|outputs| outputs.into_iter().map(|p| p.display().to_string()).collect())
}
#[tauri::command]
fn cancel(job: u32, session: State<'_, Session>) {
    if let Some(token) = session.running.lock().unwrap().get(&job) {
        token.cancel();
    }
}

/// On macOS and Linux, Tumble can be installed by dragging the app out of
/// the .dmg or with the .deb, and neither can run a setup step. So the
/// window adds the right-click menu itself (`tumble menu install`, with the
/// `tumble` next to it) when the menu isn't set up for this copy yet. A menu
/// removed on purpose with `tumble menu uninstall` is left removed.
#[cfg(unix)]
fn ensure_menu() {
    use std::process::{Command, Stdio};
    std::thread::spawn(|| {
        let Some(tumble) = std::env::current_exe()
            .ok()
            .and_then(|e| std::fs::canonicalize(e).ok())
            .map(|e| e.with_file_name("tumble"))
            .filter(|t| t.is_file())
        else {
            return;
        };
        let removed =
            tumble_core::config::data_dir().is_some_and(|d| d.join("menu-removed").exists());
        if removed {
            return;
        }
        let Ok(status) =
            Command::new(&tumble).args(["menu", "status"]).stdin(Stdio::null()).output()
        else {
            return;
        };
        let runs = format!("Runs: {}", tumble.display());
        if String::from_utf8_lossy(&status.stdout).lines().any(|l| l == runs) {
            return;
        }
        let _ = Command::new(&tumble)
            .args(["menu", "install"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    });
}

pub fn run() {
    #[cfg(unix)]
    ensure_menu();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Session::new())
        .invoke_handler(tauri::generate_handler![inspect, presets, engines, convert, cancel])
        .run(tauri::generate_context!())
        .expect("error while running Tumble");
}
