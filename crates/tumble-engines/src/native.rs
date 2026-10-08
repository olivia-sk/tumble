//! Finding and loading the vendor DLLs (libheif, PDFium).
//!
//! DLLs are only ever loaded by full path from the folder holding the
//! running exe, never through the default search order (current folder,
//! PATH), so a planted DLL cannot be picked up. Their own dependencies are
//! resolved from that same folder, then System32.
//!
//! Under `cargo test` the exe lives in `target/<profile>/deps/`; the build
//! script copies `vendor/` into `target/<profile>/`, so the parent of a
//! `deps` folder is searched as well.

use libloading::Library;
use std::path::{Path, PathBuf};

/// Folders that may hold vendor DLLs, most preferred first.
fn dirs() -> Vec<PathBuf> {
    let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf))
    else {
        return Vec::new();
    };
    let mut v = vec![dir.clone()];
    if dir.file_name().is_some_and(|n| n == "deps")
        && let Some(parent) = dir.parent()
    {
        v.push(parent.to_path_buf());
    }
    v
}

/// The first folder that holds every file in `names`, so a DLL and its
/// dependencies always come from the same place.
pub fn find_dir(names: &[&str]) -> Option<PathBuf> {
    dirs().into_iter().find(|d| names.iter().all(|n| d.join(n).is_file()))
}

/// Loads `path`, resolving its dependencies from its own folder and System32.
pub fn load(path: &Path) -> Result<Library, String> {
    #[cfg(windows)]
    let result = {
        use libloading::os::windows::{
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32, Library as WinLibrary,
        };
        // SAFETY: loading runs the DLL's initialisers; these are the pinned,
        // hash-checked vendor builds.
        unsafe {
            WinLibrary::load_with_flags(
                path,
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
            )
        }
        .map(Library::from)
    };
    #[cfg(not(windows))]
    // SAFETY: as above.
    let result = unsafe { Library::new(path) };
    result.map_err(|e| format!("cannot load {}: {e}", path.display()))
}

/// Copies a function pointer out of `lib`. The caller keeps `lib` alive for
/// as long as the pointer is used (the engines keep theirs in a static).
///
/// # Safety
/// `T` must be the exact function pointer type of the exported symbol.
pub unsafe fn symbol<T: Copy>(lib: &Library, name: &str) -> Result<T, String> {
    // SAFETY: forwarded to the caller.
    unsafe { lib.get::<T>(name.as_bytes()) }
        .map(|s| *s)
        .map_err(|e| format!("missing function {name}: {e}"))
}
