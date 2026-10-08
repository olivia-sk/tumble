//! Windows shell integration (PRD section 11):
//!
//! - `menu`      registry install/uninstall/status of the right-click menu
//! - `instance`  batching many right-click invocations into one job
//! - `progress`  the shell progress dialog
//! - `toast`     the "12 files converted" notification
//! - `registry`  a small wrapper over the Win32 registry API
#![cfg(windows)]

pub mod instance;
pub mod menu;
pub mod progress;
mod registry;
pub mod toast;

use std::path::PathBuf;
use tumble_core::brand;

/// The app icon, also embedded in the exes; written to disk for toasts.
const ICON: &[u8] = include_bytes!("../../../assets/tumble.ico");

/// `%LOCALAPPDATA%\Tumble`, created if needed.
pub fn local_data_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join(brand::DATA_DIR);
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Writes the icon to `%LOCALAPPDATA%\Tumble\tumble.ico` for toasts.
pub fn write_icon() -> Option<PathBuf> {
    let path = local_data_dir()?.join("tumble.ico");
    std::fs::write(&path, ICON).ok()?;
    Some(path)
}

/// Local date and time as (year, month, day, hour, minute, second).
pub fn local_now() -> (u16, u16, u16, u16, u16, u16) {
    // SAFETY: plain query.
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    (t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond)
}
