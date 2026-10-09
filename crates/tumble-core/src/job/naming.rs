//! Output file names: `photo.webp`, then `photo (1).webp`, `photo (2).webp`.

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Hands out output paths for a whole batch, so parallel jobs never pick the
/// same name and no job ever writes over one of the batch's inputs.
#[derive(Default)]
pub struct OutputNamer {
    /// Paths as `key` makes them.
    taken: Mutex<HashSet<String>>,
    protected: Mutex<HashSet<String>>,
}

/// Lower-cased on Windows and macOS, whose file names ignore case by
/// default; as is on Linux, where `a.png` and `A.png` are different files.
pub fn key(path: &Path) -> String {
    let s = path.to_string_lossy();
    if cfg!(any(windows, target_os = "macos")) { s.to_lowercase() } else { s.into_owned() }
}

impl OutputNamer {
    pub fn new() -> OutputNamer {
        OutputNamer::default()
    }

    /// Never hand out this path, even with `overwrite`. Used for inputs.
    pub fn protect(&self, path: &Path) {
        let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        self.protected.lock().unwrap().insert(key(&path));
    }

    /// Picks the path for `name` in `dir`. With `overwrite` an existing file
    /// is replaced; otherwise the first free numbered name is used.
    pub fn claim(&self, dir: &Path, name: &OsStr, overwrite: bool) -> PathBuf {
        let protected = self.protected.lock().unwrap();
        let mut taken = self.taken.lock().unwrap();
        let (stem, ext) = split_name(name);
        for n in 0u32.. {
            let candidate = if n == 0 {
                dir.join(name)
            } else {
                let mut s = stem.clone();
                s.push(format!(" ({n})"));
                if let Some(ext) = &ext {
                    s.push(".");
                    s.push(ext);
                }
                dir.join(s)
            };
            let abs = std::path::absolute(&candidate).unwrap_or_else(|_| candidate.clone());
            let k = key(&abs);
            let on_disk = candidate.symlink_metadata().is_ok();
            if taken.contains(&k) || protected.contains(&k) || (on_disk && !overwrite) {
                continue;
            }
            taken.insert(k);
            return candidate;
        }
        unreachable!("ran out of numbered names")
    }
}

/// `photo.tar.png` -> (`photo.tar`, `png`); `README` -> (`README`, None).
fn split_name(name: &OsStr) -> (OsString, Option<OsString>) {
    let path = Path::new(name);
    match (path.file_stem(), path.extension()) {
        (Some(stem), Some(ext)) => (stem.to_os_string(), Some(ext.to_os_string())),
        _ => (name.to_os_string(), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::ScratchDir;
    use std::fs;

    #[test]
    fn numbers_existing_and_claimed_names() {
        let dir = ScratchDir::new_in(&std::env::temp_dir(), "tumble-test-").unwrap();
        let d = dir.path();
        fs::write(d.join("photo.webp"), b"x").unwrap();
        let namer = OutputNamer::new();
        let name = OsStr::new("photo.webp");
        assert_eq!(namer.claim(d, name, false), d.join("photo (1).webp"));
        assert_eq!(namer.claim(d, name, false), d.join("photo (2).webp"));
        let upper = namer.claim(d, OsStr::new("PHOTO.webp"), false);
        if cfg!(any(windows, target_os = "macos")) {
            assert_eq!(upper, d.join("PHOTO (3).webp"), "names differing only in case collide");
        } else {
            assert_eq!(upper, d.join("PHOTO.webp"), "case matters on Linux");
        }
    }

    #[test]
    fn overwrite_replaces_files_but_not_inputs_or_claimed_names() {
        let dir = ScratchDir::new_in(&std::env::temp_dir(), "tumble-test-").unwrap();
        let d = dir.path();
        fs::write(d.join("a.webp"), b"x").unwrap();
        fs::write(d.join("b.webp"), b"x").unwrap();
        let namer = OutputNamer::new();
        namer.protect(&d.join("b.webp"));
        assert_eq!(namer.claim(d, OsStr::new("a.webp"), true), d.join("a.webp"));
        assert_eq!(namer.claim(d, OsStr::new("a.webp"), true), d.join("a (1).webp"));
        assert_eq!(namer.claim(d, OsStr::new("b.webp"), true), d.join("b (1).webp"));
    }

    #[test]
    fn names_without_extension() {
        let dir = ScratchDir::new_in(&std::env::temp_dir(), "tumble-test-").unwrap();
        let d = dir.path();
        fs::write(d.join("README"), b"x").unwrap();
        let namer = OutputNamer::new();
        assert_eq!(namer.claim(d, OsStr::new("README"), false), d.join("README (1)"));
    }
}
