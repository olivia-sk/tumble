//! Every user-visible name in one place, so a rename touches as few files as
//! possible. Crate names in the `Cargo.toml` files are the only other spot.
//!
//! Run `tumble menu uninstall` before changing anything here and
//! `tumble menu install` afterwards, or stale registry keys are left behind.

/// Display name of the app.
pub const APP_NAME: &str = "Tumble";

/// Console binary (`tumble.exe` on Windows, `tumble` elsewhere).
pub const CLI_BIN: &str = "tumble";

/// GUI-subsystem binary used by the right-click menu (`tumblew.exe`).
pub const GUI_BIN: &str = "tumblew";

/// Verb key name under `SystemFileAssociations\.<ext>\shell\`.
pub const MENU_VERB: &str = "Tumble";

/// Key under `HKCU\Software\Classes` that holds the shared submenus and the
/// install record.
pub const REGISTRY_KEY: &str = "Tumble";

/// AppUserModelID that toasts are shown under; registered by `menu install`.
pub const AUMID: &str = "Tumble.Converter";

/// Prefix for environment variables, e.g. `TUMBLE_FFMPEG`.
pub const ENV_PREFIX: &str = "TUMBLE_";

/// Folder name under `%APPDATA%` (config, presets) and `%LOCALAPPDATA%`
/// (logs) on Windows, and under `~/Library/...` on macOS.
pub const DATA_DIR: &str = "Tumble";

/// Folder name under `~/.config` and `~/.local/share` on Linux, where names
/// are lower case by convention.
pub const UNIX_DIR: &str = "tumble";

/// Builds an environment variable name such as `TUMBLE_FFMPEG`.
pub fn env_var(suffix: &str) -> String {
    format!("{ENV_PREFIX}{suffix}")
}
