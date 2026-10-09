//! Notifications on macOS through `osascript`'s `display notification`.
//! macOS does not let a script's notification do anything when clicked, so
//! unlike Windows and Linux it cannot open the output folder.

use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use tumble_core::brand;

pub fn show(title: &str, detail: &str, _folder: Option<&Path>) -> io::Result<()> {
    // The texts go in as arguments, so nothing needs AppleScript escaping.
    let status = Command::new("/usr/bin/osascript")
        .args(["-e", "on run argv"])
        .arg("-e")
        .arg(format!(
            "display notification (item 2 of argv) with title \"{}\" subtitle (item 1 of argv)",
            brand::APP_NAME
        ))
        .args(["-e", "end run"])
        .args([title, detail])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("osascript failed ({status})")))
    }
}
