//! The optional settings file (PRD section 12), `%APPDATA%\Tumble\config.toml`:
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

/// `%APPDATA%\Tumble`, or `$XDG_CONFIG_HOME/Tumble` / `~/.config/Tumble`
/// elsewhere.
pub fn config_dir() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join(brand::DATA_DIR))
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
