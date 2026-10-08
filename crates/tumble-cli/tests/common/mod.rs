//! Helpers for running the built `tumble.exe` in tests.

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

/// Whether a vendor DLL sits next to the built tumble.exe (the engines
/// build script copies them there from vendor/).
pub fn vendor_dll(name: &str) -> bool {
    Path::new(env!("CARGO_BIN_EXE_tumble")).with_file_name(name).is_file()
}
