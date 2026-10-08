//! Pixel plumbing shared by every engine that reads or writes images:
//! codecs per format (`codecs/`) and getting pixels into a shape each
//! encoder accepts (`pixels.rs`).

pub mod codecs;
pub mod pixels;

pub use codecs::{READS, WRITES, decode, default_quality};

use image::DynamicImage;
use std::path::Path;
use tumble_core::{ConvertOptions, EngineError, FormatId};

/// Applies `--resize`, adapts the pixels to `to` and writes the file.
/// `from` tells vector sources (already rendered at size) from raster ones.
pub fn save(
    img: DynamicImage,
    from: FormatId,
    to: FormatId,
    options: &ConvertOptions,
    path: &Path,
) -> Result<(), EngineError> {
    let img = pixels::resize(img, from, options);
    let img = pixels::prepare(img, to);
    codecs::encode(to, &img, options, path)
}
