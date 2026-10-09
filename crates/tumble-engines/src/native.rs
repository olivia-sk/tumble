//! Finding and loading the vendor libraries (libheif, PDFium): DLLs on
//! Windows, `.dylib` on macOS, `.so` on Linux.
//!
//! They are only ever loaded by full path from the folder holding the
//! running exe, never through the default search order (current folder,
//! PATH, `LD_LIBRARY_PATH`), so a planted library cannot be picked up. Their
//! own dependencies are resolved from that same folder (Windows load flags,
//! or the `$ORIGIN` / `@loader_path` rpath `fetch-vendor.sh` builds them
//! with), then the system.
//!
//! Under `cargo test` the exe lives in `target/<profile>/deps/`; the build
//! script copies `vendor/` into `target/<profile>/`, so the parent of a
//! `deps` folder is searched as well.

use libloading::Library;
use std::path::{Path, PathBuf};

/// Where messages say the libraries belong.
pub const WHERE: &str = if cfg!(windows) { "next to tumble.exe" } else { "next to tumble" };

/// The script that fetches them, for messages.
pub const FETCH: &str =
    if cfg!(windows) { "scripts/fetch-vendor.ps1" } else { "scripts/fetch-vendor.sh" };

/// Folders that may hold vendor libraries, most preferred first.
fn dirs() -> Vec<PathBuf> {
    // Resolve links: ~/.local/bin/tumble points into the install folder.
    let Some(dir) = std::env::current_exe()
        .ok()
        .map(|e| std::fs::canonicalize(&e).unwrap_or(e))
        .and_then(|e| e.parent().map(Path::to_path_buf))
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

/// Loads `path`, resolving its dependencies from its own folder, then the
/// system (System32 on Windows).
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
