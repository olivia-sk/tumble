//! Helpers for running the built `tumble` in tests.

#![allow(dead_code)] // each test file uses a different subset

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub fn tumble<I, S>(args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new(env!("CARGO_BIN_EXE_tumble")).args(args).output().expect("run tumble")
}

pub fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

pub fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// Writes a small RGBA PNG (or any format the `image` crate infers from the
/// extension) and returns its path.
pub fn image_file(path: &Path) -> PathBuf {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let img = image::RgbaImage::from_fn(20, 10, |x, _| image::Rgba([x as u8 * 12, 80, 160, 255]));
    image::DynamicImage::ImageRgba8(img).to_rgb8().save(path).unwrap();
    path.to_path_buf()
}

/// File names in `dir`, sorted.
pub fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

pub fn is_webp(path: &Path) -> bool {
    let b = std::fs::read(path).unwrap();
    b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP"
}

/// Whether a vendor library sits next to the built tumble (the engines
/// build script copies them there from vendor/).
pub fn vendor_dll(name: &str) -> bool {
    Path::new(env!("CARGO_BIN_EXE_tumble")).with_file_name(name).is_file()
}

/// Vendor library names on this OS.
#[cfg(windows)]
pub const PDFIUM: &str = "pdfium.dll";
#[cfg(windows)]
pub const HEIF: &str = "heif.dll";
#[cfg(windows)]
pub const HEIF_ENCODER: &str = "libx265.dll";
#[cfg(windows)]
pub const HEIF_REQUIRED: &str = "heif.dll, libde265.dll, aom.dll";
#[cfg(target_os = "macos")]
pub const PDFIUM: &str = "libpdfium.dylib";
#[cfg(target_os = "macos")]
pub const HEIF: &str = "libheif.1.dylib";
#[cfg(target_os = "macos")]
pub const HEIF_ENCODER: &str = "libx265.199.dylib";
#[cfg(target_os = "macos")]
pub const HEIF_REQUIRED: &str = "libheif.1.dylib, libde265.0.dylib";
#[cfg(not(any(windows, target_os = "macos")))]
pub const PDFIUM: &str = "libpdfium.so";
#[cfg(not(any(windows, target_os = "macos")))]
pub const HEIF: &str = "libheif.so.1";
#[cfg(not(any(windows, target_os = "macos")))]
pub const HEIF_ENCODER: &str = "libx265.so.199";
#[cfg(not(any(windows, target_os = "macos")))]
pub const HEIF_REQUIRED: &str = "libheif.so.1, libde265.so.0";

/// The file name of a copy of tumble: `tumble.exe` on Windows.
pub fn exe_name(stem: &str) -> String {
    format!("{stem}{}", std::env::consts::EXE_SUFFIX)
}

/// Points `command`'s settings and data folders into `root`, so tests never
/// touch the real ones.
pub fn isolate<'c>(command: &'c mut Command, root: &Path) -> &'c mut Command {
    if cfg!(windows) {
        command.env("APPDATA", root).env("LOCALAPPDATA", root)
    } else {
        command
            .env("HOME", root)
            .env("XDG_CONFIG_HOME", root.join(".config"))
            .env("XDG_DATA_HOME", root.join(".local/share"))
    }
}

/// Where config.toml and presets.toml go under an `isolate`d `root`.
pub fn settings_dir(root: &Path) -> PathBuf {
    if cfg!(windows) {
        root.join("Tumble")
    } else if cfg!(target_os = "macos") {
        root.join("Library/Application Support/Tumble")
    } else {
        root.join(".config/tumble")
    }
}

/// Where menu-mode failures are logged under an `isolate`d `root`.
pub fn log_dir(root: &Path) -> PathBuf {
    if cfg!(windows) {
        root.join(r"Tumble\logs")
    } else if cfg!(target_os = "macos") {
        root.join("Library/Logs/Tumble")
    } else {
        root.join(".local/share/tumble/logs")
    }
}
