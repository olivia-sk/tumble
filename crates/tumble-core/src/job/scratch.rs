//! Folders that delete themselves.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// A uniquely named folder, removed with its contents on drop.
#[derive(Debug)]
pub struct ScratchDir {
    path: PathBuf,
}

impl ScratchDir {
    /// Creates `<parent>/<prefix><unique>`.
    pub fn new_in(parent: &Path, prefix: &str) -> io::Result<ScratchDir> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let pid = std::process::id();
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
        loop {
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!("{prefix}{pid}-{n}-{nanos:08x}"));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(ScratchDir { path }),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removed_on_drop_with_contents() {
        let dir = ScratchDir::new_in(&std::env::temp_dir(), "tumble-test-").unwrap();
        let path = dir.path().to_path_buf();
        fs::create_dir(path.join("sub")).unwrap();
        fs::write(path.join("sub").join("f.txt"), b"x").unwrap();
        let other = ScratchDir::new_in(&std::env::temp_dir(), "tumble-test-").unwrap();
        assert_ne!(other.path(), dir.path());
        drop(dir);
        assert!(!path.exists());
    }
}
