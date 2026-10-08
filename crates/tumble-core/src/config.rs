//! The optional settings file (PRD section 12), `config.toml` in
//! `config_dir()` (`%APPDATA%\Tumble\config.toml` on Windows):
//!
//! ```toml
//! output = "same-folder"   # or a fixed folder path
//! quality = 85
//! jobs = 4
//! [tools]
//! ffmpeg = 'C:\tools\ffmpeg\bin\ffmpeg.exe'
//! soffice = ''
//! ```
//!
//! Every key is optional; empty strings count as unset. A broken file is
//! reported once and otherwise ignored, so a typo never stops conversions.

use crate::brand;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// `"same-folder"` (the default) or a folder to put every output in.
    pub output: Option<String>,
    pub quality: Option<u8>,
    pub jobs: Option<u16>,
    #[serde(default)]
    pub tools: Tools,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tools {
    pub ffmpeg: Option<PathBuf>,
    pub ffprobe: Option<PathBuf>,
    pub soffice: Option<PathBuf>,
}

impl Config {
    pub fn parse(text: &str) -> Result<Config, String> {
        let mut c: Config = toml::from_str(text).map_err(|e| e.to_string())?;
        if let Some(q) = c.quality
            && q > 100
        {
            return Err(format!("quality must be 0-100, not {q}"));
        }
        let blank = |p: &mut Option<PathBuf>| {
            if p.as_ref().is_some_and(|p| p.as_os_str().is_empty()) {
                *p = None;
            }
        };
        blank(&mut c.tools.ffmpeg);
        blank(&mut c.tools.ffprobe);
        blank(&mut c.tools.soffice);
        if c.output.as_deref().is_some_and(|o| o.is_empty() || o == "same-folder") {
            c.output = None;
        }
        Ok(c)
    }

    /// The fixed output folder, if one is configured.
    pub fn output_dir(&self) -> Option<&Path> {
        self.output.as_deref().map(Path::new)
    }
}

/// An environment variable holding an absolute path, if set.
fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from).filter(|p| p.is_absolute())
}

#[cfg(not(windows))]
fn home() -> Option<PathBuf> {
    env_dir("HOME")
}

/// Settings and presets: `%APPDATA%\Tumble` on Windows,
/// `~/Library/Application Support/Tumble` on macOS, and
/// `$XDG_CONFIG_HOME/tumble` (`~/.config/tumble`) on Linux.
pub fn config_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    return Some(env_dir("APPDATA")?.join(brand::DATA_DIR));
    #[cfg(target_os = "macos")]
    return Some(home()?.join("Library/Application Support").join(brand::DATA_DIR));
    #[cfg(not(any(windows, target_os = "macos")))]
    return Some(
        env_dir("XDG_CONFIG_HOME").or_else(|| Some(home()?.join(".config")))?.join(brand::UNIX_DIR),
    );
}

/// Machine-local data (the LibreOffice profile template, the menu's icon and
/// install record): `%LOCALAPPDATA%\Tumble` on Windows, the same folder as
/// `config_dir()` on macOS, and `$XDG_DATA_HOME/tumble`
/// (`~/.local/share/tumble`) on Linux. Not created here.
pub fn data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    return Some(env_dir("LOCALAPPDATA")?.join(brand::DATA_DIR));
    #[cfg(target_os = "macos")]
    return config_dir();
    #[cfg(not(any(windows, target_os = "macos")))]
    return Some(
        env_dir("XDG_DATA_HOME")
            .or_else(|| Some(home()?.join(".local/share")))?
            .join(brand::UNIX_DIR),
    );
}

/// Where right-click failures are logged: `logs` under `data_dir()`, or
/// `~/Library/Logs/Tumble` on macOS. Not created here.
pub fn log_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    return Some(home()?.join("Library/Logs").join(brand::DATA_DIR));
    #[cfg(not(target_os = "macos"))]
    return Some(data_dir()?.join("logs"));
}

/// The settings for this process, read once. Problems with the file are
/// returned once by `load_warning`.
pub fn current() -> &'static Config {
    &loaded().0
}

/// Why config.toml was ignored, if it was.
pub fn load_warning() -> Option<&'static str> {
    loaded().1.as_deref()
}

fn loaded() -> &'static (Config, Option<String>) {
    static CONFIG: OnceLock<(Config, Option<String>)> = OnceLock::new();
    CONFIG.get_or_init(|| {
        let Some(path) = config_dir().map(|d| d.join("config.toml")) else {
            return (Config::default(), None);
        };
        match std::fs::read_to_string(&path) {
            Err(_) => (Config::default(), None),
            Ok(text) => match Config::parse(&text) {
                Ok(c) => (c, None),
                Err(e) => (Config::default(), Some(format!("ignoring {}: {e}", path.display()))),
            },
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_prd_example() {
        let c = Config::parse(
            r#"
output = "same-folder"
quality = 85
jobs = 4
[tools]
ffmpeg = 'C:\tools\ffmpeg\bin\ffmpeg.exe'
soffice = ''
"#,
        )
        .unwrap();
        assert_eq!(c.output_dir(), None);
        assert_eq!(c.quality, Some(85));
        assert_eq!(c.jobs, Some(4));
        assert_eq!(c.tools.ffmpeg.as_deref(), Some(Path::new(r"C:\tools\ffmpeg\bin\ffmpeg.exe")));
        assert_eq!(c.tools.soffice, None);
    }

    #[test]
    fn rejects_mistakes() {
        assert!(Config::parse("quality = 101").is_err());
        assert!(Config::parse("qualty = 80").is_err(), "unknown keys are typos");
        assert!(Config::parse("jobs = 'four'").is_err());
        assert!(Config::parse("").is_ok());
    }

    #[test]
    fn fixed_output_folder() {
        let c = Config::parse(r#"output = 'D:\Converted'"#).unwrap();
        assert_eq!(c.output_dir(), Some(Path::new(r"D:\Converted")));
    }
}
