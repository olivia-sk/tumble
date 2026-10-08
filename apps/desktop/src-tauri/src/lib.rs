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

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Session::new())
        .invoke_handler(tauri::generate_handler![inspect, presets, engines, convert, cancel])
        .run(tauri::generate_context!())
        .expect("error while running Tumble");
}
