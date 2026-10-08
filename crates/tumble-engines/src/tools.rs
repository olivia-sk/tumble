//! Finding external programs (FFmpeg, LibreOffice), PRD section 9:
//!
//! 1. environment variable, e.g. `TUMBLE_FFMPEG`
//! 2. `[tools]` in config.toml
//! 3. next to tumble.exe
//! 4. PATH (absolute entries only; never the current folder)
//! 5. known install folders
//!
//! Tools are always run by the full path found here.

use std::path::{Path, PathBuf};
use tumble_core::brand;

pub struct Tool {
    /// File name, e.g. `ffmpeg.exe`.
    pub exe: &'static str,
    /// Environment variable suffix, e.g. `FFMPEG` for `TUMBLE_FFMPEG`.
    pub env: &'static str,
}

impl Tool {
    pub fn find(&self, configured: Option<&Path>, known: &[PathBuf]) -> Option<PathBuf> {
        let file = |p: PathBuf| p.is_file().then_some(p);
        if let Some(p) =
            std::env::var_os(brand::env_var(self.env)).map(PathBuf::from).and_then(file)
        {
            return Some(p);
        }
        if let Some(p) = configured.map(Path::to_path_buf).and_then(file) {
            return Some(p);
        }
        let exe_dir = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf));
        let path_dirs = std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).filter(|d| d.is_absolute()).collect::<Vec<_>>())
            .unwrap_or_default();
        exe_dir
            .into_iter()
            .chain(path_dirs)
            .chain(known.iter().cloned())
            .map(|d| d.join(self.exe))
            .find(|p| p.is_file())
    }
}

/// Subfolders of `parent` whose names start with `prefix`, then `rest`
/// appended to each: a tiny glob for versioned install folders.
pub fn glob_dirs(parent: &Path, prefix: &str, rest: &[&str]) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(parent) else { return Vec::new() };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with(prefix))
        .map(|e| rest.iter().fold(e.path(), |p, r| p.join(r)))
        .collect();
    found.sort();
    found.reverse(); // newest version first, for date- or version-named folders
    found
}

/// Runs a tool with no console window of its own.
pub fn command(exe: &Path) -> std::process::Command {
    let mut command = std::process::Command::new(exe);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}
