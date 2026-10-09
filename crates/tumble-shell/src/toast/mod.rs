//! The notification shown when a right-click job ends, e.g. "12 files
//! converted". Clicking it opens the output folder where the system allows
//! that (Windows, and Linux notification servers with actions; not macOS).

use std::io;
use std::path::Path;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::xml;

/// Shows the notification. Errors (notifications disabled, no notifier
/// installed) are returned for logging; they never fail the job.
pub fn show(title: &str, detail: &str, folder: Option<&Path>) -> io::Result<()> {
    #[cfg(windows)]
    return windows::show(title, detail, folder).map_err(io::Error::other);
    #[cfg(target_os = "linux")]
    return linux::show(title, detail, folder);
    #[cfg(target_os = "macos")]
    return macos::show(title, detail, folder);
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        let _ = (title, detail, folder);
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}
