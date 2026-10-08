//! Notifications on Linux through `notify-send` (libnotify), which every
//! desktop's notification server understands. When `notify-send` supports
//! actions (libnotify 0.7.10 and later), clicking the notification opens
//! the output folder with `xdg-open`.

use crate::unix::which;
use std::io;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use tumble_core::brand;

/// Whether this `notify-send` has `--action` and `--wait`.
fn has_actions(notify: &Path) -> bool {
    Command::new(notify)
        .arg("--help")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains("--action"))
}

pub fn show(title: &str, detail: &str, folder: Option<&Path>) -> io::Result<()> {
    let notify = which("notify-send").ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "notify-send not found (install libnotify)")
    })?;
    let mut args = vec![format!("--app-name={}", brand::APP_NAME)];
    if let Some(icon) = crate::write_icon() {
        args.push(format!("--icon={}", icon.display()));
    }
    let open = folder.zip(which("xdg-open")).filter(|_| has_actions(&notify));
    let waits = open.is_some();
    let mut command = match open {
        // Waiting for the click happens in a process of its own, so Tumble
        // can exit straight away.
        Some((dir, xdg_open)) => {
            let mut c = Command::new("/bin/sh");
            c.arg("-c")
                .arg(r#"[ "$("$@")" = default ] && exec "$TUMBLE_XDG_OPEN" "$TUMBLE_FOLDER""#)
                .arg("sh")
                .arg(&notify)
                .args(&args)
                .args(["--action=default=Open folder", "--wait"])
                .env("TUMBLE_XDG_OPEN", xdg_open)
                .env("TUMBLE_FOLDER", dir)
                .process_group(0);
            c
        }
        None => {
            let mut c = Command::new(&notify);
            c.args(&args);
            c
        }
    };
    command.arg("--").arg(title).arg(detail);
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    let child = command.spawn()?;
    if !waits {
        child.wait_with_output()?;
    }
    Ok(())
}
