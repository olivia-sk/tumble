//! The progress dialog on Linux: `zenity --progress` (GNOME and most other
//! desktops), or `kdialog --progressbar` driven over D-Bus with `qdbus` on
//! KDE. With neither installed there is no dialog; the notification at the
//! end still appears.

use super::{State, wait_delay};
use crate::unix::which;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread::JoinHandle;
use std::time::Duration;
use tumble_core::CancelToken;

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
        if !wait_delay(&state, delay) {
            return;
        }
        if let Some(zenity) = which("zenity") {
            zenity_dialog(&zenity, &title, &line, &state, &cancel);
        } else if let Some(kdialog) = which("kdialog")
            && let Some(qdbus) =
                ["qdbus6", "qdbus", "qdbus-qt6", "qdbus-qt5"].into_iter().find_map(which)
        {
            kdialog_dialog(&kdialog, &qdbus, &title, &line, &state, &cancel);
        }
    })
}

/// Percent done, kept below 100 until the job ends (zenity closes at 100).
fn percent(state: &State) -> u64 {
    let (done, total) = state.overall();
    (done * 100 / total).min(99)
}

/// zenity reads Pango markup in its text.
fn markup(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn label(line: &str, current: &str) -> String {
    if current.is_empty() { line.to_string() } else { format!("{line}\n{current}") }
}

fn zenity_dialog(exe: &Path, title: &str, line: &str, state: &State, cancel: &CancelToken) {
    let Ok(mut child) = Command::new(exe)
        .args(["--progress", "--auto-close", "--time-remaining", "--percentage=0"])
        .arg(format!("--title={title}"))
        .arg(format!("--text={}", markup(line)))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return;
    };
    let Some(mut stdin) = child.stdin.take() else { return };
    let mut shown = String::new();
    while !state.finished.load(Ordering::SeqCst) {
        // zenity exits early only when Cancel (or the close button) is
        // pressed.
        if !matches!(child.try_wait(), Ok(None)) {
            cancel.cancel();
            return;
        }
        // The label is escaped markup on one line: zenity reads "\n" in it
        // as a line break.
        let update = format!(
            "{}\n# {}\n",
            percent(state),
            markup(&label(line, &state.current())).replace('\n', "\\n")
        );
        if update != shown {
            if stdin.write_all(update.as_bytes()).and_then(|_| stdin.flush()).is_err() {
                cancel.cancel();
                return;
            }
            shown = update;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = stdin.write_all(b"100\n");
    drop(stdin);
    for _ in 0..20 {
        if !matches!(child.try_wait(), Ok(None)) {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn kdialog_dialog(
    kdialog: &Path,
    qdbus: &Path,
    title: &str,
    line: &str,
    state: &State,
    cancel: &CancelToken,
) {
    // Prints "<service> <object path>" and leaves the dialog running.
    let Ok(out) = Command::new(kdialog)
        .args(["--title", title, "--progressbar", line, "100"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
    else {
        return;
    };
    let reply = String::from_utf8_lossy(&out.stdout);
    let mut parts = reply.split_whitespace();
    let (Some(service), Some(object)) = (parts.next(), parts.next()) else { return };
    let call = |args: &[&str]| {
        Command::new(qdbus)
            .args([service, object])
            .args(args)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    call(&["showCancelButton", "true"]);
    let mut shown = (u64::MAX, String::new());
    while !state.finished.load(Ordering::SeqCst) {
        match call(&["wasCancelled"]).as_deref() {
            Some("false") => {}
            // Cancelled, or the dialog was closed.
            _ => {
                cancel.cancel();
                return;
            }
        }
        let now = (percent(state), label(line, &state.current()));
        if now.0 != shown.0 {
            call(&["Set", "", "value", &now.0.to_string()]);
        }
        if now.1 != shown.1 {
            call(&["setLabelText", &now.1]);
        }
        shown = now;
        std::thread::sleep(Duration::from_millis(300));
    }
    call(&["close"]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_escaped_markup() {
        assert_eq!(markup("Tom & <Jo>.png"), "Tom &amp; &lt;Jo&gt;.png");
        assert_eq!(label("Converting 2 files to PNG", ""), "Converting 2 files to PNG");
        assert_eq!(label("Converting", "a.jpg"), "Converting\na.jpg");
    }
}
