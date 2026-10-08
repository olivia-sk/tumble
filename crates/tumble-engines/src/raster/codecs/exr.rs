//! OpenEXR output through the `exr` crate: half-float channels with PIZ
//! compression, the usual choice for images. The `image` crate's own writer
//! stores uncompressed 32-bit floats, about 12 bytes per pixel.
//! Decoding goes through the `image` crate.

use exr::prelude::{
    Encoding, Image, Layer, LayerAttributes, SpecificChannels, Vec2, WritableImage, f16,
};
use image::DynamicImage;
use std::path::Path;
use tumble_core::EngineError;

pub fn encode(img: &DynamicImage, path: &Path) -> Result<(), EngineError> {
    let size = (img.width() as usize, img.height() as usize);
    let h = |v: f32| f16::from_f32(v);
    let result = match img {
        DynamicImage::ImageRgba32F(buf) => {
            let channels = SpecificChannels::rgba(|Vec2(x, y): Vec2<usize>| {
                let p = buf.get_pixel(x as u32, y as u32).0;
                (h(p[0]), h(p[1]), h(p[2]), h(p[3]))
            });
            let layer = Layer::new(
                size,
                LayerAttributes::default(),
                Encoding::SMALL_FAST_LOSSLESS,
                channels,
            );
            Image::from_layer(layer).write().to_file(path)
        }
        DynamicImage::ImageRgb32F(buf) => {
            let channels = SpecificChannels::rgb(|Vec2(x, y): Vec2<usize>| {
                let p = buf.get_pixel(x as u32, y as u32).0;
                (h(p[0]), h(p[1]), h(p[2]))
            });
            let layer = Layer::new(
                size,
                LayerAttributes::default(),
                Encoding::SMALL_FAST_LOSSLESS,
                channels,
            );
            Image::from_layer(layer).write().to_file(path)
        }
        other => {
            unreachable!("exr input must be prepared as 32-bit float, got {:?}", other.color())
        }
    };
    result.map_err(|e| match e {
        exr::error::Error::Io(io) => EngineError::Io(io),
        other => EngineError::failed(format!("cannot encode OpenEXR: {other}")),
    })
}
