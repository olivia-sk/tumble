//! Copies the vendor DLLs (fetched by scripts/fetch-vendor.ps1) next to the
//! binaries in target/<profile>/, where the engines look for them during
//! development and tests. Does nothing if vendor/ is missing.

use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let vendor = manifest.join("../../vendor");
    println!("cargo:rerun-if-changed={}", vendor.display());
    let Ok(entries) = fs::read_dir(&vendor) else { return };

    // OUT_DIR is target/<profile>/build/<crate>-<hash>/out.
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let Some(profile_dir) = out.ancestors().nth(3) else { return };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("dll")) {
            println!("cargo:rerun-if-changed={}", path.display());
            let dest = profile_dir.join(entry.file_name());
            let stale = match (fs::metadata(&path), fs::metadata(&dest)) {
                (Ok(src), Ok(dst)) => {
                    src.len() != dst.len() || src.modified().ok() > dst.modified().ok()
                }
                _ => true,
            };
            if stale {
                // A DLL in use by a running test cannot be replaced; the next
                // build retries.
                let _ = fs::copy(&path, &dest);
            }
        }
    }
}
