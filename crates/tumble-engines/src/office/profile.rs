//! LibreOffice user profiles. Every conversion runs with its own throwaway
//! profile, so it works while the user has LibreOffice open and never
//! touches their settings.
//!
//! A fresh profile costs LibreOffice about 4 s to set up; a copied one
//! about 1 s. So one pristine template is built per LibreOffice version in
//! `%LOCALAPPDATA%\Tumble\office-profile\<version key>` and copied into each
//! job's scratch folder.

use crate::process_tree::{Tree, Waited};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use tumble_core::brand;

/// `file:///C:/Users/Jo%20Doe/...`, as `-env:UserInstallation` wants.
pub fn file_url(path: &Path) -> String {
    let mut url = String::from("file:///");
    for b in path.to_string_lossy().replace('\\', "/").bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b':' | b'-' | b'_' | b'.' | b'~' => {
                url.push(b as char)
            }
            _ => url.push_str(&format!("%{b:02X}")),
        }
    }
    url
}

/// A key that changes when LibreOffice is updated (size and modified time
/// of soffice.bin), plus a template layout version.
fn version_key(soffice: &Path) -> String {
    let bin = soffice.with_file_name("soffice.bin");
    let meta = fs::metadata(&bin).or_else(|_| fs::metadata(soffice));
    match meta {
        Ok(m) => {
            let mtime = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());
            format!("v2-{:x}-{mtime:x}", m.len())
        }
        Err(_) => "unknown".into(),
    }
}

fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &dest)?;
        } else {
            fs::copy(entry.path(), dest)?;
        }
    }
    Ok(())
}

/// The template for this LibreOffice, built on first use. `None` when it
/// cannot be made; callers then let LibreOffice create a fresh profile.
fn template(soffice: &Path) -> Option<PathBuf> {
    static BUILDING: Mutex<()> = Mutex::new(());
    let root = PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
        .join(brand::DATA_DIR)
        .join("office-profile");
    let path = root.join(version_key(soffice));
    if path.join("user").is_dir() {
        return Some(path);
    }
    let _guard = BUILDING.lock().unwrap_or_else(|p| p.into_inner());
    if path.join("user").is_dir() {
        return Some(path);
    }
    fs::create_dir_all(&root).ok()?;
    // Build next to the final place, then rename, so another process never
    // sees a half-made template.
    let building = root.join(format!("building-{}", std::process::id()));
    let _ = fs::remove_dir_all(&building);
    // A real (tiny) conversion makes LibreOffice finish its first-run
    // setup; --terminate_after_init stops before most of it.
    let sample = root.join(format!("sample-{}.txt", std::process::id()));
    if fs::write(&sample, "Tumble").is_err() {
        return None;
    }
    let mut command = std::process::Command::new(soffice);
    command
        .arg(format!("-env:UserInstallation={}", file_url(&building)))
        .args([
            "--headless",
            "--invisible",
            "--norestore",
            "--nologo",
            "--convert-to",
            "pdf",
            "--outdir",
        ])
        .arg(building.join("sample-out"))
        .arg(&sample)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let finished = Tree::spawn(command)
        .and_then(|tree| tree.wait(Duration::from_secs(120), || false))
        .is_ok_and(|w| matches!(w, Waited::Exited));
    let _ = fs::remove_file(&sample);
    let _ = fs::remove_dir_all(building.join("sample-out"));
    if !finished || !building.join("user").is_dir() {
        let _ = fs::remove_dir_all(&building);
        return None;
    }
    if fs::rename(&building, &path).is_err() {
        // Another process won the race; use theirs.
        let _ = fs::remove_dir_all(&building);
    }
    // Templates for older LibreOffice versions are no longer used.
    let current = path.file_name().map(|n| n.to_os_string());
    for entry in fs::read_dir(&root).into_iter().flatten().flatten() {
        let name = entry.file_name();
        let stale =
            Some(&name) != current.as_ref() && !name.to_string_lossy().starts_with("building-");
        if stale && entry.file_type().is_ok_and(|t| t.is_dir()) {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
    path.join("user").is_dir().then_some(path)
}

/// Prepares a profile folder for one job at `dest` and returns its URL.
pub fn prepare(soffice: &Path, dest: &Path) -> String {
    if let Some(t) = template(soffice)
        && copy_dir(&t, dest).is_err()
    {
        let _ = fs::remove_dir_all(dest);
    }
    file_url(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_escape_spaces_and_unicode() {
        assert_eq!(
            file_url(Path::new(r"C:\Users\Jo Doe\Temp\p")),
            "file:///C:/Users/Jo%20Doe/Temp/p"
        );
        assert_eq!(file_url(Path::new(r"C:\ä")), "file:///C:/%C3%A4");
    }
}
