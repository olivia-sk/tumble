//! Which library reads and writes each format.

mod avif;
mod exr;
mod standard;
mod svg;
mod webp;

use image::DynamicImage;
use std::path::Path;
use tumble_core::{ConvertOptions, EngineError, FormatId};

/// Formats the built-in codecs decode. HEIC goes through libheif (see
/// `crate::heif`), which `decode` and `encode` below dispatch to.
pub const READS: &[&str] = &[
    "jpeg", "png", "webp", "avif", "gif", "tiff", "bmp", "ico", "tga", "ppm", "qoi", "exr", "svg",
];

/// Formats the built-in codecs encode.
pub const WRITES: &[&str] =
    &["jpeg", "png", "webp", "avif", "gif", "tiff", "bmp", "ico", "tga", "ppm", "qoi", "exr"];

/// Quality used for lossy outputs when the user gives none.
pub fn default_quality(format: FormatId) -> u8 {
    match format.as_str() {
        "webp" => 80,
        "avif" | "heic" => 75,
        _ => 85,
    }
}

pub fn decode(
    format: FormatId,
    path: &Path,
    options: &ConvertOptions,
) -> Result<DynamicImage, EngineError> {
    match format.as_str() {
        "avif" => avif::decode(path),
        "heic" => crate::heif::codec::decode(path),
        "svg" => svg::decode(path, options.resize),
        _ => standard::decode(format, path),
    }
}

/// Writes `img`, which `pixels::prepare` has already put in a pixel layout
/// the target supports.
pub fn encode(
    format: FormatId,
    img: &DynamicImage,
    options: &ConvertOptions,
    path: &Path,
) -> Result<(), EngineError> {
    let quality = options.quality.unwrap_or_else(|| default_quality(format));
    match format.as_str() {
        "webp" => webp::encode(img, quality, path),
        "exr" => exr::encode(img, path),
        "heic" => crate::heif::codec::encode(img, quality, path),
        _ => standard::encode(format, img, quality, path),
    }
}

fn image_error(context: &str, e: image::ImageError) -> EngineError {
    match e {
        image::ImageError::IoError(io) => EngineError::Io(io),
        other => EngineError::failed(format!("{context}: {other}")),
    }
}
