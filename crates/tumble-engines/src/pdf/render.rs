//! Rendering PDF pages to RGB images with PDFium.

use super::ffi::{self, Document};
use image::{DynamicImage, RgbImage};
use std::path::Path;
use tumble_core::{EngineError, Resize};

/// Resolution when no `--resize` is given. An A4 page becomes 1240x1754.
pub const DEFAULT_DPI: f32 = 150.0;

/// Refuse renders larger than this per side, or in total.
const MAX_SIDE: u32 = 16384;
const MAX_PIXELS: u64 = 200_000_000;

/// An open PDF. The bytes stay alive with it, as PDFium requires.
pub struct Pdf {
    doc: *mut Document,
    _data: Vec<u8>,
    pub pages: usize,
}

// SAFETY: the document handle is only used while holding the PDFium lock.
unsafe impl Send for Pdf {}

impl Pdf {
    pub fn open(path: &Path) -> Result<Pdf, EngineError> {
        let data = std::fs::read(path)?;
        let pdfium = ffi::lock().map_err(EngineError::Failed)?;
        let api = pdfium.api;
        // SAFETY: `data` outlives the document (both live in `Pdf`); no password.
        let doc = unsafe {
            (api.load_mem_document64)(data.as_ptr().cast(), data.len(), std::ptr::null())
        };
        if doc.is_null() {
            // SAFETY: plain query, still under the lock.
            let msg = match unsafe { (api.get_last_error)() } {
                ffi::ERR_PASSWORD => "the PDF is password-protected, which is not supported",
                ffi::ERR_SECURITY => "the PDF uses an unsupported security handler",
                ffi::ERR_FORMAT | ffi::ERR_FILE => "the file is not a valid PDF",
                _ => "PDFium could not open the PDF",
            };
            return Err(EngineError::failed(msg));
        }
        // SAFETY: doc is open.
        let pages = unsafe { (api.get_page_count)(doc) };
        Ok(Pdf { doc, _data: data, pages: usize::try_from(pages).unwrap_or(0) })
    }

    /// Renders page `index` (0-based) on white. With `resize` the page is
    /// fitted into the box (scaling up if needed); otherwise rendered at
    /// `DEFAULT_DPI`.
    pub fn render(
        &self,
        index: usize,
        resize: Option<Resize>,
    ) -> Result<DynamicImage, EngineError> {
        let pdfium = ffi::lock().map_err(EngineError::Failed)?;
        let api = pdfium.api;
        // SAFETY: doc is open; the page is closed before returning.
        unsafe {
            let page = (api.load_page)(self.doc, index as i32);
            if page.is_null() {
                return Err(EngineError::failed(format!("cannot load page {}", index + 1)));
            }
            // Points are 1/72 inch; the size already reflects /Rotate.
            let (pw, ph) = ((api.page_width)(page), (api.page_height)(page));
            let scale = DEFAULT_DPI / 72.0;
            let natural =
                ((pw * scale).round().max(1.0) as u32, (ph * scale).round().max(1.0) as u32);
            let (w, h) = match resize {
                Some(r) => r.fit(natural.0, natural.1, true),
                None => natural,
            };
            if w > MAX_SIDE || h > MAX_SIDE || u64::from(w) * u64::from(h) > MAX_PIXELS {
                (api.close_page)(page);
                return Err(EngineError::failed(format!(
                    "page {} would be {w}x{h} pixels; use --resize to make it smaller",
                    index + 1
                )));
            }

            let bitmap = (api.bitmap_create)(w as i32, h as i32, 0);
            if bitmap.is_null() {
                (api.close_page)(page);
                return Err(EngineError::failed("PDFium could not allocate a page bitmap"));
            }
            (api.bitmap_fill_rect)(bitmap, 0, 0, w as i32, h as i32, 0xFFFF_FFFF);
            (api.render_page_bitmap)(
                bitmap,
                page,
                0,
                0,
                w as i32,
                h as i32,
                0,
                ffi::RENDER_ANNOTATIONS,
            );
            (api.close_page)(page);

            // FPDFBitmap_Create without alpha gives BGRx, 4 bytes per pixel.
            let buffer = (api.bitmap_buffer)(bitmap).cast::<u8>();
            let stride = (api.bitmap_stride)(bitmap) as usize;
            let mut out = RgbImage::new(w, h);
            for y in 0..h as usize {
                let row = std::slice::from_raw_parts(buffer.add(y * stride), w as usize * 4);
                for (x, px) in row.as_chunks::<4>().0.iter().enumerate() {
                    out.put_pixel(x as u32, y as u32, image::Rgb([px[2], px[1], px[0]]));
                }
            }
            (api.bitmap_destroy)(bitmap);
            Ok(DynamicImage::ImageRgb8(out))
        }
    }
}

impl Drop for Pdf {
    fn drop(&mut self) {
        if let Ok(pdfium) = ffi::lock() {
            // SAFETY: doc is open and closed only here.
            unsafe { (pdfium.api.close_document)(self.doc) };
        }
    }
}
