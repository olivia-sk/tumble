//! `tumble pick <files>`: what the macOS Quick Action runs. Finder cannot
//! show a submenu of formats under a Quick Action, so this asks with a
//! short list instead: only the formats every selected file can become, the
//! same ones the Windows menu offers. Then it converts in menu mode.
//!
//! `TUMBLE_PICK=<format>` answers the question without a window (tests).

use super::menu::{log, run_menu};
use crate::args::ConvertArgs;
use crate::exit;
use std::path::PathBuf;
use tumble_core::menu::common_menu_targets;
use tumble_core::{Format, FormatId, brand};

pub fn run_pick(files: Vec<PathBuf>) -> u8 {
    let registry = tumble_engines::default_registry();
    let mut formats = Vec::new();
    for file in &files {
        match Format::of_path(file).filter(|_| !file.is_dir()) {
            Some(f) => formats.push(f),
            None => {
                let name = file.file_name().unwrap_or(file.as_os_str()).to_string_lossy();
                tell("Can't convert this file", &format!("{name} isn't a format Tumble reads."));
                return exit::NO_ROUTE;
            }
        }
    }
    let targets = common_menu_targets(&registry, &formats);
    if targets.is_empty() {
        tell("Nothing to convert to", "There is no format all of the selected files can become.");
        return exit::NO_ROUTE;
    }

    let answer = match std::env::var(brand::env_var("PICK")) {
        Ok(id) => Format::parse(&id).map(|f| f.id).filter(|id| targets.contains(id)),
        Err(_) => match ask(&targets, files.len()) {
            Ok(answer) => answer,
            Err(e) => {
                log(&[format!("could not ask for a format: {e}")]);
                None
            }
        },
    };
    let Some(to) = answer else {
        return exit::OK; // cancelled
    };
    run_menu(ConvertArgs {
        inputs: files,
        to: Some(to.as_str().to_string()),
        out: None,
        recursive: false,
        jobs: None,
        quality: None,
        resize: None,
        at: None,
        preset: None,
        overwrite: false,
        json: false,
    })
}

/// A notification, unless `TUMBLE_NO_UI` is set.
fn tell(title: &str, detail: &str) {
    log(&[format!("{title}: {detail}")]);
    if super::menu::ui_enabled() {
        let _ = tumble_shell::toast::show(title, detail, None);
    }
}

fn prompt(count: usize) -> String {
    format!("Convert {count} {} to:", if count == 1 { "file" } else { "files" })
}

/// Shows the list and returns the chosen format, or `None` on Cancel.
#[cfg(target_os = "macos")]
fn ask(targets: &[FormatId], count: usize) -> std::io::Result<Option<FormatId>> {
    // The names go in as arguments, so nothing needs AppleScript escaping.
    let script = format!(
        "on run argv\n\
         activate\n\
         set answer to choose from list (items 2 thru -1 of argv) with title \"{}\" \
         with prompt (item 1 of argv) default items {{item 2 of argv}} OK button name \"Convert\"\n\
         if answer is false then return \"\"\n\
         return item 1 of answer\n\
         end run",
        brand::APP_NAME
    );
    let out = std::process::Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(script)
        .arg(prompt(count))
        .args(targets.iter().map(|t| t.format().name))
        .stdin(std::process::Stdio::null())
        .output()?;
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(targets.iter().copied().find(|t| t.format().name == name))
}

/// On Linux the menus list the formats themselves; this is only for running
/// `tumble pick` by hand, with zenity.
#[cfg(not(target_os = "macos"))]
fn ask(targets: &[FormatId], count: usize) -> std::io::Result<Option<FormatId>> {
    let out = std::process::Command::new("zenity")
        .args(["--list", "--hide-header", "--column=Format"])
        .arg(format!("--title={}", brand::APP_NAME))
        .arg(format!("--text={}", prompt(count)))
        .args(targets.iter().map(|t| t.format().name))
        .stdin(std::process::Stdio::null())
        .output()?;
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(targets.iter().copied().find(|t| t.format().name == name))
}
