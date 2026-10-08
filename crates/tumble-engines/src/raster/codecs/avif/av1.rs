//! Decoding one AV1 image item with rav1d (dav1d ported to Rust), and
//! turning its YCbCr planes into RGB.

use image::{DynamicImage, ImageBuffer, Rgba};
use rav1d::include::dav1d::data::Dav1dData;
use rav1d::include::dav1d::dav1d::{Dav1dContext, Dav1dSettings};
use rav1d::include::dav1d::headers::{
    DAV1D_MC_BT709, DAV1D_MC_BT2020_CL, DAV1D_MC_BT2020_NCL, DAV1D_MC_IDENTITY,
    DAV1D_PIXEL_LAYOUT_I400, DAV1D_PIXEL_LAYOUT_I420, DAV1D_PIXEL_LAYOUT_I422,
};
use rav1d::include::dav1d::picture::Dav1dPicture;
use rav1d::src::lib as dav1d;
use std::mem::MaybeUninit;
use std::ptr::NonNull;
use tumble_core::EngineError;

/// dav1d returns negative errno values; EAGAIN means "call me again".
const EAGAIN: i32 = -11;

/// One decoded AV1 frame, samples widened to u16.
pub struct Frame {
    pub width: usize,
    pub height: usize,
    bits: u32,
    layout: u32,
    matrix: u32,
    full_range: bool,
    planes: [Vec<u16>; 3],
    /// Width of the chroma planes (0 for monochrome).
    chroma_width: usize,
}

/// Closes the decoder however we leave `decode_av1`.
struct Context(Option<Dav1dContext>);

impl Drop for Context {
    fn drop(&mut self) {
        // SAFETY: the context came from `dav1d_open` and is closed only here.
        unsafe { dav1d::dav1d_close(NonNull::new(&mut self.0)) };
    }
}

fn check(result: i32, what: &str) -> Result<(), EngineError> {
    if result < 0 {
        Err(EngineError::failed(format!("AV1 decoder failed to {what} (error {result})")))
    } else {
        Ok(())
    }
}

pub fn decode_av1(obu: &[u8]) -> Result<Frame, EngineError> {
    let mut settings = MaybeUninit::<Dav1dSettings>::uninit();
    // SAFETY: `dav1d_default_settings` writes a complete Dav1dSettings.
    let mut settings = unsafe {
        dav1d::dav1d_default_settings(NonNull::new_unchecked(settings.as_mut_ptr()));
        settings.assume_init()
    };
    settings.max_frame_delay = 1;

    let mut ctx = Context(None);
    // SAFETY: both pointers are valid for the duration of the call.
    let r = unsafe { dav1d::dav1d_open(NonNull::new(&mut ctx.0), NonNull::new(&mut settings)) };
    check(r.0, "start")?;

    let mut data = Dav1dData::default();
    // SAFETY: `data` is a valid Dav1dData; dav1d allocates `obu.len()` bytes
    // and returns a pointer to them, which we fill before sending.
    unsafe {
        let buf = dav1d::dav1d_data_create(NonNull::new(&mut data), obu.len());
        if buf.is_null() {
            return Err(EngineError::failed("AV1 decoder could not allocate input"));
        }
        std::ptr::copy_nonoverlapping(obu.as_ptr(), buf, obu.len());
    }

    let mut picture = Dav1dPicture::default();
    let mut got_picture = false;
    for _ in 0..10_000 {
        if data.sz > 0 {
            // SAFETY: context is open; `data` is valid.
            let r = unsafe { dav1d::dav1d_send_data(ctx.0, NonNull::new(&mut data)) };
            if r.0 != EAGAIN {
                check(r.0, "read the image data")?;
            }
        }
        // SAFETY: context is open; `picture` is a valid, empty Dav1dPicture.
        let r = unsafe { dav1d::dav1d_get_picture(ctx.0, NonNull::new(&mut picture)) };
        if r.0 == 0 {
            got_picture = true;
            break;
        }
        if r.0 != EAGAIN {
            check(r.0, "decode the image")?;
        }
    }
    if data.sz > 0 {
        // SAFETY: `data` still owns its buffer.
        unsafe { dav1d::dav1d_data_unref(NonNull::new(&mut data)) };
    }
    if !got_picture {
        return Err(EngineError::failed("AV1 decoder produced no picture"));
    }

    let frame = copy_frame(&picture);
    // SAFETY: `picture` was filled by `dav1d_get_picture` and is released once.
    unsafe { dav1d::dav1d_picture_unref(NonNull::new(&mut picture)) };
    frame
}

fn copy_frame(p: &Dav1dPicture) -> Result<Frame, EngineError> {
    let width = usize::try_from(p.p.w).unwrap_or(0);
    let height = usize::try_from(p.p.h).unwrap_or(0);
    let bits = u32::try_from(p.p.bpc).unwrap_or(0);
    if width == 0 || height == 0 || !(8..=12).contains(&bits) {
        return Err(EngineError::failed("AV1 picture has an unsupported size or bit depth"));
    }
    let layout = p.p.layout;
    let (chroma_width, chroma_height) = match layout {
        DAV1D_PIXEL_LAYOUT_I400 => (0, 0),
        DAV1D_PIXEL_LAYOUT_I420 => (width.div_ceil(2), height.div_ceil(2)),
        DAV1D_PIXEL_LAYOUT_I422 => (width.div_ceil(2), height),
        _ => (width, height),
    };
    let seq = p.seq_hdr.ok_or_else(|| EngineError::failed("AV1 picture has no sequence header"))?;
    // SAFETY: dav1d keeps the sequence header alive while the picture is referenced.
    let seq = unsafe { seq.as_ref() };

    let plane = |index: usize, w: usize, h: usize| -> Vec<u16> {
        let Some(base) = p.data[index] else { return Vec::new() };
        let stride = p.stride[index.min(1)];
        let mut out = Vec::with_capacity(w * h);
        for row in 0..h {
            // SAFETY: dav1d guarantees `h` rows of `stride` bytes per plane,
            // each holding at least `w` samples of 1 (8-bit) or 2 bytes.
            unsafe {
                let start = base.as_ptr().cast::<u8>().offset(row as isize * stride);
                if bits == 8 {
                    out.extend(std::slice::from_raw_parts(start, w).iter().map(|&s| u16::from(s)));
                } else {
                    out.extend_from_slice(std::slice::from_raw_parts(start.cast::<u16>(), w));
                }
            }
        }
        out
    };

    Ok(Frame {
        width,
        height,
        bits,
        layout,
        matrix: seq.mtrx,
        full_range: seq.color_range != 0,
        planes: [
            plane(0, width, height),
            plane(1, chroma_width, chroma_height),
            plane(2, chroma_width, chroma_height),
        ],
        chroma_width,
    })
}

/// YCbCr to RGB, with the matrix and range from the AV1 sequence header.
pub fn to_rgba(color: &Frame, alpha: Option<&Frame>, premultiplied: bool) -> DynamicImage {
    let max = ((1u32 << color.bits) - 1) as f32;
    let scale = (1u32 << (color.bits - 8)) as f32;
    // Luma weights (Kr, Kb). Unspecified falls back to BT.601, as libavif does.
    let (kr, kb) = match color.matrix {
        DAV1D_MC_BT709 => (0.2126, 0.0722),
        DAV1D_MC_BT2020_NCL | DAV1D_MC_BT2020_CL => (0.2627, 0.0593),
        _ => (0.299, 0.114),
    };
    let kg = 1.0 - kr - kb;
    let identity = color.matrix == DAV1D_MC_IDENTITY;
    let mono = color.layout == DAV1D_PIXEL_LAYOUT_I400;
    let (sub_x, sub_y) = match color.layout {
        DAV1D_PIXEL_LAYOUT_I420 => (1, 1),
        DAV1D_PIXEL_LAYOUT_I422 => (1, 0),
        _ => (0, 0),
    };

    let luma = |v: u16| {
        if color.full_range { v as f32 / max } else { (v as f32 - 16.0 * scale) / (219.0 * scale) }
    };
    let chroma = |v: u16| {
        if color.full_range {
            (v as f32 - (max + 1.0) / 2.0) / max
        } else {
            (v as f32 - 128.0 * scale) / (224.0 * scale)
        }
    };
    let alpha_at = |i: usize| -> f32 {
        let Some(a) = alpha else { return 1.0 };
        let amax = ((1u32 << a.bits) - 1) as f32;
        let v = a.planes[0][i] as f32;
        let f = if a.full_range {
            v / amax
        } else {
            let s = (1u32 << (a.bits - 8)) as f32;
            (v - 16.0 * s) / (219.0 * s)
        };
        f.clamp(0.0, 1.0)
    };

    let (w, h) = (color.width, color.height);
    let mut out = ImageBuffer::<Rgba<u16>, Vec<u16>>::new(w as u32, h as u32);
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let yv = color.planes[0][i];
            let (r, g, b) = if mono {
                let l = luma(yv);
                (l, l, l)
            } else {
                let ci = (y >> sub_y) * color.chroma_width + (x >> sub_x);
                let (u, v) = (color.planes[1][ci], color.planes[2][ci]);
                if identity {
                    // GBR: Y holds green, U blue, V red.
                    (luma(v), luma(yv), luma(u))
                } else {
                    let l = luma(yv);
                    let (cb, cr) = (chroma(u), chroma(v));
                    let r = l + 2.0 * (1.0 - kr) * cr;
                    let b = l + 2.0 * (1.0 - kb) * cb;
                    let g = (l - kr * r - kb * b) / kg;
                    (r, g, b)
                }
            };
            let a = alpha_at(i);
            let unpremultiply = |c: f32| if premultiplied && a > 0.0 { c / a } else { c };
            let px = |c: f32| (unpremultiply(c).clamp(0.0, 1.0) * 65535.0).round() as u16;
            out.put_pixel(x as u32, y as u32, Rgba([px(r), px(g), px(b), px(a)]));
        }
    }
    let img = DynamicImage::ImageRgba16(out);
    if color.bits == 8 { DynamicImage::ImageRgba8(img.to_rgba8()) } else { img }
}
