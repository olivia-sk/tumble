//! Running FFmpeg: progress from `-progress pipe:1`, cancellation by
//! killing the process, and the tail of stderr for error messages.

use crate::tools;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Child, Stdio};
use std::sync::mpsc;
use std::time::Duration;
use tumble_core::{CancelToken, EngineError, Progress};

/// Kills the child if we leave early (error, cancel, panic).
struct Guard(Child);

impl Drop for Guard {
    fn drop(&mut self) {
        if let Ok(None) = self.0.try_wait() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

/// Runs `exe args...`. `duration` (seconds) turns FFmpeg's `out_time_us`
/// into a fraction.
pub fn run(
    exe: &Path,
    args: &[OsString],
    duration: Option<f64>,
    progress: &dyn Progress,
    cancel: &CancelToken,
) -> Result<(), EngineError> {
    let child = tools::command(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| EngineError::failed(format!("cannot start {}: {e}", exe.display())))?;
    let mut guard = Guard(child);

    let stdout = guard.0.stdout.take().expect("piped stdout");
    let (tx, rx) = mpsc::channel::<f64>();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(us) =
                line.strip_prefix("out_time_us=").and_then(|v| v.trim().parse::<f64>().ok())
                && tx.send(us / 1_000_000.0).is_err()
            {
                break;
            }
        }
    });
    let mut stderr = guard.0.stderr.take().expect("piped stderr");
    let errors = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });

    let status = loop {
        if cancel.is_cancelled() {
            return Err(EngineError::Cancelled); // the guard kills FFmpeg
        }
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(seconds) => {
                if let Some(total) = duration.filter(|d| *d > 0.0) {
                    progress.update((seconds / total).clamp(0.0, 1.0) as f32);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            // stdout closed: FFmpeg is finishing; don't spin.
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                std::thread::sleep(Duration::from_millis(20))
            }
        }
        if let Some(status) = guard.0.try_wait()? {
            break status;
        }
    };
    if status.success() {
        return Ok(());
    }
    let text = errors.join().unwrap_or_default();
    let tail: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let tail = tail[tail.len().saturating_sub(3)..].join(" / ");
    Err(EngineError::failed(if tail.is_empty() {
        format!("FFmpeg failed ({status})")
    } else {
        format!("FFmpeg failed: {tail}")
    }))
}
