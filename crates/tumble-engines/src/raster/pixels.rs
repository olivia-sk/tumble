//! Getting decoded pixels into a shape each encoder accepts.
//!
//! - JPEG and PPM have no alpha: transparent areas are flattened onto white.
//! - ICO holds at most 256x256: larger images are shrunk to fit.
//! - PNG and TIFF keep 16-bit depth; other outputs are 8-bit.
//! - OpenEXR is linear light: 8/16-bit sRGB input is linearised on the way
//!   in, and EXR input is sRGB-encoded on the way out to anything else.
//! - Opaque images drop their alpha channel, which keeps WebP, AVIF and
//!   friends smaller.

use image::imageops::FilterType;
use image::{DynamicImage, ImageBuffer, Rgb, Rgba, Rgba32FImage};
use tumble_core::{ConvertOptions, FormatId};

const ICO_MAX: u32 = 256;

/// Applies `--resize` to raster input. Vector input (SVG, PDF pages) is
/// already rendered at the requested size.
pub fn resize(img: DynamicImage, from: FormatId, options: &ConvertOptions) -> DynamicImage {
    match options.resize {
        Some(r) if !matches!(from.as_str(), "svg" | "pdf") => {
            let (w, h) = r.fit(img.width(), img.height(), false);
            if (w, h) == (img.width(), img.height()) {
                img
            } else {
                img.resize_exact(w, h, FilterType::Lanczos3)
            }
        }
        _ => img,
    }
}

pub fn prepare(img: DynamicImage, to: FormatId) -> DynamicImage {
    let to = to.as_str();
    if to == "exr" {
        return to_linear(img);
    }
    let mut img = if is_float(&img) { to_display(img) } else { img };
    if to == "ico" && (img.width() > ICO_MAX || img.height() > ICO_MAX) {
        img = img.resize(ICO_MAX, ICO_MAX, FilterType::Lanczos3);
    }
    let alpha = has_transparency(&img);
    let deep = matches!(
        img,
        DynamicImage::ImageLuma16(_)
            | DynamicImage::ImageLumaA16(_)
            | DynamicImage::ImageRgb16(_)
            | DynamicImage::ImageRgba16(_)
    );
    match to {
        "jpeg" | "ppm" => DynamicImage::ImageRgb8(flatten_on_white(&img)),
        "png" | "tiff" if deep => {
            if alpha {
                DynamicImage::ImageRgba16(img.to_rgba16())
            } else {
                DynamicImage::ImageRgb16(img.to_rgb16())
            }
        }
        "gif" | "ico" => DynamicImage::ImageRgba8(img.to_rgba8()),
        _ => {
            if alpha {
                DynamicImage::ImageRgba8(img.to_rgba8())
            } else {
                DynamicImage::ImageRgb8(img.to_rgb8())
            }
        }
    }
}

fn is_float(img: &DynamicImage) -> bool {
    matches!(img, DynamicImage::ImageRgb32F(_) | DynamicImage::ImageRgba32F(_))
}

fn has_transparency(img: &DynamicImage) -> bool {
    match img {
        DynamicImage::ImageLumaA8(i) => i.pixels().any(|p| p[1] != u8::MAX),
        DynamicImage::ImageRgba8(i) => i.pixels().any(|p| p[3] != u8::MAX),
        DynamicImage::ImageLumaA16(i) => i.pixels().any(|p| p[1] != u16::MAX),
        DynamicImage::ImageRgba16(i) => i.pixels().any(|p| p[3] != u16::MAX),
        DynamicImage::ImageRgba32F(i) => i.pixels().any(|p| p[3] < 1.0),
        _ => false,
    }
}

fn flatten_on_white(img: &DynamicImage) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    let rgba = img.to_rgba8();
    ImageBuffer::from_fn(rgba.width(), rgba.height(), |x, y| {
        let Rgba([r, g, b, a]) = *rgba.get_pixel(x, y);
        let a = u16::from(a);
        let blend = |c: u8| ((u16::from(c) * a + 255 * (255 - a) + 127) / 255) as u8;
        Rgb([blend(r), blend(g), blend(b)])
    })
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn linear_to_srgb(c: f32) -> f32 {
    let c = c.max(0.0);
    if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

/// For EXR output: float pixels in linear light.
fn to_linear(img: DynamicImage) -> DynamicImage {
    let alpha = has_transparency(&img);
    let mut px: Rgba32FImage = if is_float(&img) {
        img.into_rgba32f()
    } else {
        let mut f = img.into_rgba32f();
        for p in f.pixels_mut() {
            for c in &mut p.0[..3] {
                *c = srgb_to_linear(*c);
            }
        }
        f
    };
    if alpha {
        DynamicImage::ImageRgba32F(std::mem::take(&mut px))
    } else {
        DynamicImage::ImageRgb32F(DynamicImage::ImageRgba32F(px).into_rgb32f())
    }
}

/// From EXR input: clip and sRGB-encode to 16-bit for display formats.
fn to_display(img: DynamicImage) -> DynamicImage {
    let f = img.into_rgba32f();
    let out = ImageBuffer::<Rgba<u16>, Vec<u16>>::from_fn(f.width(), f.height(), |x, y| {
        let p = f.get_pixel(x, y).0;
        let q = |c: f32| (c.clamp(0.0, 1.0) * 65535.0).round() as u16;
        Rgba([q(linear_to_srgb(p[0])), q(linear_to_srgb(p[1])), q(linear_to_srgb(p[2])), q(p[3])])
    });
    DynamicImage::ImageRgba16(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbaImage;

    fn id(s: &'static str) -> FormatId {
        FormatId(s)
    }

    #[test]
    fn jpeg_flattens_transparency_onto_white() {
        let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([0, 0, 0, 0])));
        let out = prepare(img, id("jpeg"));
        assert_eq!(out.as_rgb8().unwrap().get_pixel(0, 0), &Rgb([255, 255, 255]));
    }

    #[test]
    fn opaque_rgba_drops_alpha_and_transparent_keeps_it() {
        let opaque = DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([1, 2, 3, 255])));
        assert!(matches!(prepare(opaque, id("webp")), DynamicImage::ImageRgb8(_)));
        let clear = DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([1, 2, 3, 9])));
        assert!(matches!(prepare(clear, id("webp")), DynamicImage::ImageRgba8(_)));
    }

    #[test]
    fn ico_is_shrunk_to_256() {
        let img = DynamicImage::ImageRgb8(ImageBuffer::new(1024, 512));
        let out = prepare(img, id("ico"));
        assert_eq!((out.width(), out.height()), (256, 128));
    }

    #[test]
    fn exr_round_trip_preserves_srgb_values() {
        let img = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(1, 1, Rgb([200, 100, 30])));
        let linear = prepare(img, id("exr"));
        let back = prepare(linear, id("png"));
        assert_eq!(back.to_rgb8().get_pixel(0, 0), &Rgb([200, 100, 30]));
    }

    #[test]
    fn deep_png_stays_16_bit() {
        let img = DynamicImage::ImageRgb16(ImageBuffer::from_pixel(1, 1, Rgb([1000, 2000, 3000])));
        assert!(matches!(prepare(img.clone(), id("png")), DynamicImage::ImageRgb16(_)));
        assert!(matches!(prepare(img, id("bmp")), DynamicImage::ImageRgb8(_)));
    }
}
