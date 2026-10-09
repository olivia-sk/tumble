//! Shell integration (PRD section 11): the right-click menu and what a
//! right-click job shows.
//!
//! - `menu`      install/uninstall/status of the right-click menu: Explorer
//!   (registry) on Windows, a Finder Quick Action on macOS, and Dolphin,
//!   Nemo, Thunar and GNOME Files on Linux
//! - `instance`  batching many right-click invocations into one job
//! - `progress`  the progress dialog
//! - `toast`     the "12 files converted" notification
//! - `registry`  a small wrapper over the Win32 registry API (Windows)

#[cfg(windows)]
#[path = "instance.rs"]
pub mod instance;
#[cfg(unix)]
#[path = "unix/instance.rs"]
pub mod instance;

#[cfg(windows)]
pub mod menu;
#[cfg(target_os = "linux")]
#[path = "linux/menu.rs"]
pub mod menu;
#[cfg(target_os = "macos")]
#[path = "macos/menu.rs"]
pub mod menu;

pub mod progress;
#[cfg(windows)]
mod registry;
pub mod toast;
#[cfg(unix)]
mod unix;

use std::path::PathBuf;
use tumble_core::{Format, FormatId};

/// One input format and the targets its submenu offers, in order.
#[derive(Clone)]
pub struct FormatMenu {
    pub format: &'static Format,
    pub targets: Vec<FormatId>,
}

/// The app icon, also embedded in the Windows exes; written to disk for
/// notifications and menus.
#[cfg(windows)]
const ICON: (&[u8], &str) = (include_bytes!("../../../assets/tumble.ico"), "tumble.ico");
#[cfg(not(windows))]
const ICON: (&[u8], &str) = (include_bytes!("../../../assets/tumble.png"), "tumble.png");

/// Tumble's data folder (`%LOCALAPPDATA%\Tumble` on Windows; see
/// `tumble_core::config::data_dir`), created if needed.
pub fn local_data_dir() -> Option<PathBuf> {
    let dir = tumble_core::config::data_dir()?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Writes the icon into the data folder for notifications and menus.
pub fn write_icon() -> Option<PathBuf> {
    let path = local_data_dir()?.join(ICON.1);
    if std::fs::read(&path).ok().as_deref() != Some(ICON.0) {
        std::fs::write(&path, ICON.0).ok()?;
    }
    Some(path)
}

/// Local date and time as (year, month, day, hour, minute, second).
#[cfg(windows)]
pub fn local_now() -> (u16, u16, u16, u16, u16, u16) {
    // SAFETY: plain query.
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    (t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond)
}

#[cfg(unix)]
pub use unix::local_now;
