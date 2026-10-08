//! Formats handled by the `image` crate.

use super::image_error;
use image::codecs::avif::AvifEncoder;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::codecs::pnm::{PnmEncoder, PnmSubtype, SampleEncoding};
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;
use tumble_core::{EngineError, FormatId};

/// rav1e speed, 1 (slowest, smallest) to 10. 6 keeps large photos
/// reasonable without the assembly build.
const AVIF_SPEED: u8 = 6;

fn image_format(format: FormatId) -> ImageFormat {
    match format.as_str() {
        "jpeg" => ImageFormat::Jpeg,
        "png" => ImageFormat::Png,
        "webp" => ImageFormat::WebP,
        "avif" => ImageFormat::Avif,
        "gif" => ImageFormat::Gif,
        "tiff" => ImageFormat::Tiff,
        "bmp" => ImageFormat::Bmp,
        "ico" => ImageFormat::Ico,
        "tga" => ImageFormat::Tga,
        "ppm" => ImageFormat::Pnm,
        "qoi" => ImageFormat::Qoi,
        "exr" => ImageFormat::OpenExr,
        other => unreachable!("{other} is not an image crate format"),
    }
}

/// Decodes the first frame and applies EXIF orientation.
pub fn decode(format: FormatId, path: &Path) -> Result<DynamicImage, EngineError> {
    let file = BufReader::new(File::open(path)?);
    let mut reader = ImageReader::new(file);
    reader.set_format(image_format(format));
    reader.no_limits();
    let mut decoder = reader.into_decoder().map_err(|e| image_error("cannot read image", e))?;
    let orientation = decoder.orientation().map_err(|e| image_error("cannot read image", e))?;
    let mut img =
        DynamicImage::from_decoder(decoder).map_err(|e| image_error("cannot decode image", e))?;
    img.apply_orientation(orientation);
    Ok(img)
}

pub fn encode(
    format: FormatId,
    img: &DynamicImage,
    quality: u8,
    path: &Path,
) -> Result<(), EngineError> {
    let mut out = BufWriter::new(File::create(path)?);
    let err = |e| image_error("cannot encode image", e);
    match format.as_str() {
        "jpeg" => {
            img.write_with_encoder(JpegEncoder::new_with_quality(&mut out, quality)).map_err(err)?
        }
        "png" => img
            .write_with_encoder(PngEncoder::new_with_quality(
                &mut out,
                CompressionType::Default,
                FilterType::Adaptive,
            ))
            .map_err(err)?,
        "avif" => img
            .write_with_encoder(AvifEncoder::new_with_speed_quality(&mut out, AVIF_SPEED, quality))
            .map_err(err)?,
        "ppm" => img
            .write_with_encoder(
                PnmEncoder::new(&mut out).with_subtype(PnmSubtype::Pixmap(SampleEncoding::Binary)),
            )
            .map_err(err)?,
        _ => img.write_to(&mut out, image_format(format)).map_err(err)?,
    }
    out.flush()?;
    Ok(())
}
