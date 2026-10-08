//! Small helpers shared by the macOS and Linux integrations.

use std::path::{Path, PathBuf};

#[cfg_attr(target_os = "macos", allow(dead_code))]
/// A program on PATH (absolute entries only, never the current folder) or
/// in the usual system folders. File managers and Finder start Tumble with
/// a short PATH, so the system folders are always searched too.
pub fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .filter(|d| d.is_absolute())
        .chain(["/usr/bin", "/bin", "/usr/local/bin", "/opt/homebrew/bin"].map(PathBuf::from))
        .map(|d| d.join(name))
        .find(|p| is_executable(p))
}

#[cfg_attr(target_os = "macos", allow(dead_code))]
pub fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// Quotes `s` for a POSIX shell: `'it'\''s'`.
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Local date and time as (year, month, day, hour, minute, second).
pub fn local_now() -> (u16, u16, u16, u16, u16, u16) {
    // SAFETY: `time` with a null pointer only returns the time, and
    // `localtime_r` writes into the zeroed struct we own.
    unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&now, &mut tm).is_null() {
            return (1970, 1, 1, 0, 0, 0);
        }
        let n = |v: libc::c_int| u16::try_from(v).unwrap_or(0);
        (
            n(tm.tm_year + 1900),
            n(tm.tm_mon + 1),
            n(tm.tm_mday),
            n(tm.tm_hour),
            n(tm.tm_min),
            n(tm.tm_sec),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_for_the_shell() {
        assert_eq!(shell_quote("/Users/jo/Apps/tumble"), "'/Users/jo/Apps/tumble'");
        assert_eq!(shell_quote("/a/Jo's apps/t"), r"'/a/Jo'\''s apps/t'");
    }

    #[test]
    fn finds_sh() {
        assert!(which("sh").is_some());
        assert!(which("no-such-program-tumble").is_none());
    }

    #[test]
    fn local_time_is_plausible() {
        let (y, mo, d, h, mi, s) = local_now();
        assert!(y >= 2024 && (1..=12).contains(&mo) && (1..=31).contains(&d));
        assert!(h < 24 && mi < 60 && s < 61);
    }
}
