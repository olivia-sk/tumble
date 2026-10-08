//! SVG input through resvg. With `--resize` the drawing is rendered at the
//! requested size (vector input scales up cleanly); otherwise at its own size.

use image::{DynamicImage, RgbaImage};
use resvg::{tiny_skia, usvg};
use std::path::Path;
use std::sync::{Arc, OnceLock};
use tumble_core::{EngineError, Resize};

/// Refuse renders larger than this per side.
const MAX_SIDE: u32 = 16384;

/// System fonts, loaded once per process and only when an SVG is converted.
fn fonts() -> Arc<usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = usvg::fontdb::Database::new();
            db.load_system_fonts();
            Arc::new(db)
        })
        .clone()
}

pub fn decode(path: &Path, resize: Option<Resize>) -> Result<DynamicImage, EngineError> {
    let data = std::fs::read(path)?;
    let options = usvg::Options {
        resources_dir: path.parent().map(Path::to_path_buf),
        fontdb: fonts(),
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_data(&data, &options)
        .map_err(|e| EngineError::failed(format!("cannot read SVG: {e}")))?;

    let size = tree.size();
    let natural = (size.width().ceil() as u32, size.height().ceil() as u32);
    let (w, h) = match resize {
        Some(r) => r.fit(natural.0, natural.1, true),
        None => natural,
    };
    if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
        return Err(EngineError::failed(format!(
            "cannot render SVG at {w}x{h}; use --resize to pick a size up to {MAX_SIDE} px"
        )));
    }

    let mut pixmap = tiny_skia::Pixmap::new(w, h)
        .ok_or_else(|| EngineError::failed("cannot allocate SVG canvas"))?;
    let transform =
        tiny_skia::Transform::from_scale(w as f32 / size.width(), h as f32 / size.height());
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // tiny-skia stores premultiplied alpha; image formats expect straight alpha.
    let mut out = RgbaImage::new(w, h);
    for (dst, src) in out.pixels_mut().zip(pixmap.pixels()) {
        let c = src.demultiply();
        *dst = image::Rgba([c.red(), c.green(), c.blue(), c.alpha()]);
    }
    Ok(DynamicImage::ImageRgba8(out))
}
