//! WebP output through libwebp. Quality 100 means lossless; anything lower is
//! lossy at that quality. Decoding goes through the `image` crate.

use image::DynamicImage;
use std::fs;
use std::path::Path;
use tumble_core::EngineError;

/// libwebp's maximum width and height.
const MAX_SIDE: u32 = 16383;

pub fn encode(img: &DynamicImage, quality: u8, path: &Path) -> Result<(), EngineError> {
    let (w, h) = (img.width(), img.height());
    if w > MAX_SIDE || h > MAX_SIDE {
        return Err(EngineError::failed(format!(
            "WebP images can be at most {MAX_SIDE} px per side; this one is {w}x{h} (try --resize)"
        )));
    }
    let encoder = match img {
        DynamicImage::ImageRgb8(rgb) => webp::Encoder::from_rgb(rgb.as_raw(), w, h),
        DynamicImage::ImageRgba8(rgba) => webp::Encoder::from_rgba(rgba.as_raw(), w, h),
        other => {
            unreachable!("webp input must be prepared as Rgb8 or Rgba8, got {:?}", other.color())
        }
    };
    let lossless = quality >= 100;
    let data = encoder
        .encode_simple(lossless, f32::from(quality))
        .map_err(|e| EngineError::failed(format!("cannot encode WebP: {e:?}")))?;
    fs::write(path, &*data)?;
    Ok(())
}
