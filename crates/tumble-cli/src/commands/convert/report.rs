//! Progress and results: human text, one JSON event per line, or silent
//! (menu mode, where a dialog and a toast take over).
//!
//! JSON events (all on stdout):
//!   {"event":"start","input":"..."}
//!   {"event":"progress","input":"...","fraction":0.5}
//!   {"event":"done","input":"...","outputs":["..."],"route":"png -> webp (image)"}
//!   {"event":"error","input":"...","error":"...","unsupported":false}
//!   {"event":"summary","converted":3,"failed":1,"unsupported":0,"skipped":2}

use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use tumble_core::Progress;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Human,
    Json,
    Silent,
}

/// Something watching the batch, such as the shell progress dialog.
pub trait Observer: Sync {
    fn started(&self, input: &Path);
    fn progress(&self, input: &Path, fraction: f32);
    fn finished(&self, input: &Path);
}

#[derive(Default)]
pub struct Tally {
    pub converted: AtomicUsize,
    pub failed: AtomicUsize,
    pub unsupported: AtomicUsize,
}

pub struct Reporter<'a> {
    mode: Mode,
    observer: Option<&'a dyn Observer>,
    /// Serialises output so lines from parallel jobs never interleave.
    lock: Mutex<()>,
    pub tally: Tally,
    /// (input, error) for every file that did not convert.
    pub failures: Mutex<Vec<(PathBuf, String)>>,
    /// Every file written.
    pub outputs: Mutex<Vec<PathBuf>>,
}

fn path_str(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

impl<'a> Reporter<'a> {
    pub fn new(mode: Mode, observer: Option<&'a dyn Observer>) -> Reporter<'a> {
        Reporter {
            mode,
            observer,
            lock: Mutex::new(()),
            tally: Tally::default(),
            failures: Mutex::new(Vec::new()),
            outputs: Mutex::new(Vec::new()),
        }
    }

    fn event(&self, value: Value) {
        let _guard = self.lock.lock().unwrap();
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{value}");
        let _ = out.flush();
    }

    pub fn start(&self, input: &Path) {
        if let Some(o) = self.observer {
            o.started(input);
        }
        if self.mode == Mode::Json {
            self.event(json!({ "event": "start", "input": path_str(input) }));
        }
    }

    pub fn progress<'r>(&'r self, input: &'r Path) -> JobProgress<'r, 'a> {
        JobProgress { reporter: self, input }
    }

    pub fn done(&self, input: &Path, outputs: &[PathBuf], route: &str) {
        self.tally.converted.fetch_add(1, Ordering::Relaxed);
        self.outputs.lock().unwrap().extend_from_slice(outputs);
        if let Some(o) = self.observer {
            o.finished(input);
        }
        match self.mode {
            Mode::Json => {
                let outputs: Vec<String> = outputs.iter().map(|p| path_str(p)).collect();
                self.event(json!({
                    "event": "done", "input": path_str(input), "outputs": outputs, "route": route
                }));
            }
            Mode::Human => {
                let _guard = self.lock.lock().unwrap();
                for output in outputs {
                    println!("{} -> {}", input.display(), output.display());
                }
            }
            Mode::Silent => {}
        }
    }

    pub fn error(&self, input: &Path, error: &str, unsupported: bool) {
        let counter = if unsupported { &self.tally.unsupported } else { &self.tally.failed };
        counter.fetch_add(1, Ordering::Relaxed);
        self.failures.lock().unwrap().push((input.to_path_buf(), error.to_string()));
        if let Some(o) = self.observer {
            o.finished(input);
        }
        match self.mode {
            Mode::Json => self.event(json!({
                "event": "error", "input": path_str(input), "error": error, "unsupported": unsupported
            })),
            Mode::Human => {
                let _guard = self.lock.lock().unwrap();
                eprintln!("error: {}: {error}", input.display());
            }
            Mode::Silent => {}
        }
    }

    pub fn summary(&self, skipped: usize) {
        let converted = self.tally.converted.load(Ordering::Relaxed);
        let failed = self.tally.failed.load(Ordering::Relaxed);
        let unsupported = self.tally.unsupported.load(Ordering::Relaxed);
        match self.mode {
            Mode::Json => self.event(json!({
                "event": "summary", "converted": converted, "failed": failed,
                "unsupported": unsupported, "skipped": skipped
            })),
            Mode::Human => {
                let mut parts = vec![format!(
                    "{converted} {} converted",
                    if converted == 1 { "file" } else { "files" }
                )];
                if failed > 0 {
                    parts.push(format!("{failed} failed"));
                }
                if unsupported > 0 {
                    parts.push(format!("{unsupported} unsupported"));
                }
                if skipped > 0 {
                    parts.push(format!("{skipped} skipped"));
                }
                eprintln!("{}", parts.join(", "));
            }
            Mode::Silent => {}
        }
    }
}

/// Forwards a job's progress to the observer and, in JSON mode, as events
/// rounded to whole percent.
pub struct JobProgress<'r, 'a> {
    reporter: &'r Reporter<'a>,
    input: &'r Path,
}

impl Progress for JobProgress<'_, '_> {
    fn update(&self, fraction: f32) {
        if let Some(o) = self.reporter.observer {
            o.progress(self.input, fraction);
        }
        if self.reporter.mode == Mode::Json {
            let fraction = (fraction * 100.0).round() / 100.0;
            self.reporter.event(json!({
                "event": "progress", "input": path_str(self.input), "fraction": fraction
            }));
        }
    }
}
