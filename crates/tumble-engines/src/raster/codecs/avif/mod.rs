//! AVIF input: `avif-parse` pulls the AV1 data out of the container,
//! rav1d decodes it (`av1.rs`), and the container's crop, rotation and
//! mirror properties are applied (`props.rs`). Encoding goes through the
//! `image` crate (rav1e), see `standard.rs`.
//!
//! Not handled: grid (tiled) images, which fail with a clear error.

mod av1;
mod props;

use image::DynamicImage;
use props::Transform;
use std::path::Path;
use tumble_core::EngineError;

pub fn decode(path: &Path) -> Result<DynamicImage, EngineError> {
    let data = std::fs::read(path)?;
    let avif = avif_parse::read_avif(&mut data.as_slice())
        .map_err(|e| EngineError::failed(format!("cannot read AVIF container: {e:?}")))?;
    let color = av1::decode_av1(&avif.primary_item)?;
    let alpha = match &avif.alpha_item {
        Some(item) => Some(av1::decode_av1(item)?),
        None => None,
    };
    if let Some(a) = &alpha
        && (a.width, a.height) != (color.width, color.height)
    {
        return Err(EngineError::failed("AVIF alpha plane size does not match the image"));
    }
    let mut img = av1::to_rgba(&color, alpha.as_ref(), avif.premultiplied_alpha);
    for t in props::transforms(&data, img.width(), img.height()) {
        img = apply(img, t);
    }
    Ok(img)
}

fn apply(img: DynamicImage, t: Transform) -> DynamicImage {
    match t {
        Transform::Crop { x, y, width, height } => img.crop_imm(x, y, width, height),
        // Anti-clockwise quarter turns.
        Transform::Rotate { quarter_turns: 1 } => img.rotate270(),
        Transform::Rotate { quarter_turns: 2 } => img.rotate180(),
        Transform::Rotate { quarter_turns: 3 } => img.rotate90(),
        Transform::Rotate { .. } => img,
        Transform::Mirror { vertical: true } => img.flipv(),
        Transform::Mirror { vertical: false } => img.fliph(),
    }
}
