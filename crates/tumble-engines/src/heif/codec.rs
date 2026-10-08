//! HEIC decode and encode through libheif.
//!
//! Decoding applies the file's rotation, mirror and crop (libheif's default)
//! and assembles grid (tiled) images, which is how iPhones store photos.
//! Images with more than 8 bits per channel decode to 16-bit.
//! Encoding writes 8-bit HEVC; quality 100 means lossless.

use super::ffi::{self, Api};
use image::{DynamicImage, ImageBuffer, Rgb, Rgba};
use std::ffi::c_void;
use std::path::Path;
use std::ptr::{null, null_mut};
use tumble_core::EngineError;

fn api() -> Result<&'static Api, EngineError> {
    ffi::api().map_err(EngineError::Failed)
}

fn fail(msg: String) -> EngineError {
    EngineError::Failed(msg)
}

/// Frees a libheif object on drop.
struct Owned<T> {
    ptr: *mut T,
    free: unsafe extern "C" fn(*mut T),
}

impl<T> Drop for Owned<T> {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            // SAFETY: `ptr` came from the matching libheif allocator.
            unsafe { (self.free)(self.ptr) };
        }
    }
}

fn context(api: &Api) -> Result<Owned<ffi::Context>, EngineError> {
    // SAFETY: plain allocation.
    let ptr = unsafe { (api.context_alloc)() };
    if ptr.is_null() {
        return Err(fail("libheif could not allocate a context".into()));
    }
    Ok(Owned { ptr, free: api.context_free })
}

/// `heif_image_release` and `heif_image_handle_release` take const
/// pointers; adapt them to `Owned`.
fn release_image(api: &Api) -> unsafe extern "C" fn(*mut ffi::Image) {
    // SAFETY: *const and *mut have the same ABI.
    unsafe { std::mem::transmute(api.image_release) }
}

fn release_handle(api: &Api) -> unsafe extern "C" fn(*mut ffi::Handle) {
    // SAFETY: *const and *mut have the same ABI.
    unsafe { std::mem::transmute(api.handle_release) }
}

pub fn decode(path: &Path) -> Result<DynamicImage, EngineError> {
    let api = api()?;
    let data = std::fs::read(path)?;
    // `data` outlives `ctx`, as read_from_memory_without_copy requires.
    let ctx = context(api)?;
    // SAFETY: ctx is live; data stays valid until ctx is freed.
    unsafe {
        (api.read_from_memory_without_copy)(ctx.ptr, data.as_ptr().cast(), data.len(), null())
    }
    .check("cannot read HEIC file")
    .map_err(fail)?;

    let mut handle = null_mut();
    // SAFETY: ctx is live; libheif writes a handle we release below.
    unsafe { (api.get_primary_image_handle)(ctx.ptr, &mut handle) }
        .check("HEIC file has no primary image")
        .map_err(fail)?;
    let handle = Owned { ptr: handle, free: release_handle(api) };

    // SAFETY: handle is live.
    let (alpha, bits) =
        unsafe { ((api.handle_has_alpha)(handle.ptr) != 0, (api.handle_luma_bits)(handle.ptr)) };
    let deep = bits > 8;
    let chroma = match (alpha, deep) {
        (false, false) => ffi::CHROMA_RGB,
        (true, false) => ffi::CHROMA_RGBA,
        (false, true) => ffi::CHROMA_RRGGBB_LE,
        (true, true) => ffi::CHROMA_RRGGBBAA_LE,
    };

    let mut img = null_mut();
    // SAFETY: handle is live; null options mean libheif's defaults.
    unsafe { (api.decode_image)(handle.ptr, &mut img, ffi::COLORSPACE_RGB, chroma, null()) }
        .check("cannot decode HEIC image")
        .map_err(fail)?;
    let img = Owned { ptr: img, free: release_image(api) };

    // SAFETY: img is live; the plane is `height` rows of `stride` bytes.
    unsafe {
        let w = usize::try_from((api.image_width)(img.ptr)).unwrap_or(0);
        let h = usize::try_from((api.image_height)(img.ptr)).unwrap_or(0);
        let mut stride = 0usize;
        let plane = (api.plane_readonly)(img.ptr, ffi::CHANNEL_INTERLEAVED, &mut stride);
        if plane.is_null() || w == 0 || h == 0 {
            return Err(fail("libheif returned no pixels".into()));
        }
        let channels = if alpha { 4 } else { 3 };
        let row_bytes = w * channels * if deep { 2 } else { 1 };
        let rows = (0..h).map(|y| std::slice::from_raw_parts(plane.add(y * stride), row_bytes));
        let (w32, h32) = (w as u32, h as u32);
        Ok(if deep {
            // Samples hold `bits` significant bits; stretch them to 16 by
            // repeating the top bits (so 1023 at 10-bit becomes 65535).
            let bits = bits.clamp(9, 16) as u32;
            let shift = 16 - bits;
            let widen = |lo: u8, hi: u8| {
                let v = u16::from_le_bytes([lo, hi]);
                if shift == 0 { v } else { (v << shift) | (v >> (bits - shift)) }
            };
            let samples: Vec<u16> = rows
                .flat_map(|r| r.as_chunks::<2>().0.iter().map(|&[lo, hi]| widen(lo, hi)))
                .collect();
            if alpha {
                DynamicImage::ImageRgba16(
                    ImageBuffer::<Rgba<u16>, _>::from_raw(w32, h32, samples).unwrap(),
                )
            } else {
                DynamicImage::ImageRgb16(
                    ImageBuffer::<Rgb<u16>, _>::from_raw(w32, h32, samples).unwrap(),
                )
            }
        } else {
            let samples: Vec<u8> = rows.flatten().copied().collect();
            if alpha {
                DynamicImage::ImageRgba8(ImageBuffer::from_raw(w32, h32, samples).unwrap())
            } else {
                DynamicImage::ImageRgb8(ImageBuffer::from_raw(w32, h32, samples).unwrap())
            }
        })
    }
}

/// Whether this libheif build can write HEIC (the GPL build with x265).
pub fn can_encode() -> bool {
    // SAFETY: plain query.
    ffi::api().is_ok_and(|api| unsafe { (api.have_encoder)(ffi::COMPRESSION_HEVC) } != 0)
}

/// Collects the encoded file from `heif_context_write`.
unsafe extern "C" fn collect(
    _ctx: *mut ffi::Context,
    data: *const c_void,
    size: usize,
    user: *mut c_void,
) -> ffi::Error {
    // SAFETY: `user` is the Vec passed to context_write; `data` holds `size` bytes.
    unsafe {
        let out = &mut *user.cast::<Vec<u8>>();
        out.extend_from_slice(std::slice::from_raw_parts(data.cast::<u8>(), size));
    }
    ffi::Error { code: 0, subcode: 0, message: c"Success".as_ptr() }
}

/// Writes `img`, which `pixels::prepare` has made Rgb8 or Rgba8.
pub fn encode(img: &DynamicImage, quality: u8, path: &Path) -> Result<(), EngineError> {
    let api = api()?;
    if !can_encode() {
        return Err(fail(format!(
            "writing HEIC needs {} (the GPL libheif build); run scripts/fetch-vendor.ps1 without -Lgpl",
            ffi::ENCODER
        )));
    }
    let (bytes, alpha) = match img {
        DynamicImage::ImageRgb8(b) => (b.as_raw(), false),
        DynamicImage::ImageRgba8(b) => (b.as_raw(), true),
        other => {
            unreachable!("heic input must be prepared as Rgb8 or Rgba8, got {:?}", other.color())
        }
    };
    let (w, h) = (img.width() as usize, img.height() as usize);
    let (wi, hi) = (w as i32, h as i32);

    let ctx = context(api)?;
    let mut encoder = null_mut();
    // SAFETY: ctx is live.
    unsafe { (api.get_encoder)(ctx.ptr, ffi::COMPRESSION_HEVC, &mut encoder) }
        .check("no HEVC encoder")
        .map_err(fail)?;
    let encoder = Owned { ptr: encoder, free: api.encoder_release };
    // SAFETY: encoder is live.
    let set = unsafe {
        if quality >= 100 {
            (api.encoder_lossless)(encoder.ptr, 1)
        } else {
            (api.encoder_lossy_quality)(encoder.ptr, i32::from(quality))
        }
    };
    set.check("cannot set HEIC quality").map_err(fail)?;

    let chroma = if alpha { ffi::CHROMA_RGBA } else { ffi::CHROMA_RGB };
    let mut image = null_mut();
    // SAFETY: plain allocation; released by `image` below.
    unsafe { (api.image_create)(wi, hi, ffi::COLORSPACE_RGB, chroma, &mut image) }
        .check("cannot create HEIC image")
        .map_err(fail)?;
    let image = Owned { ptr: image, free: release_image(api) };
    // SAFETY: image is live; the plane holds `h` rows of `stride` bytes and
    // each source row is `w * channels` bytes.
    unsafe {
        (api.image_add_plane)(image.ptr, ffi::CHANNEL_INTERLEAVED, wi, hi, 8)
            .check("cannot create HEIC plane")
            .map_err(fail)?;
        let mut stride = 0usize;
        let plane = (api.plane)(image.ptr, ffi::CHANNEL_INTERLEAVED, &mut stride);
        if plane.is_null() {
            return Err(fail("libheif returned no plane".into()));
        }
        let row = w * if alpha { 4 } else { 3 };
        for y in 0..h {
            std::ptr::copy_nonoverlapping(bytes.as_ptr().add(y * row), plane.add(y * stride), row);
        }
        (api.encode_image)(ctx.ptr, image.ptr, encoder.ptr, null(), null_mut())
            .check("cannot encode HEIC")
            .map_err(fail)?;
    }

    let mut out: Vec<u8> = Vec::new();
    let mut writer = ffi::Writer { api_version: 1, write: collect };
    // SAFETY: ctx is live; `out` outlives the call.
    unsafe { (api.context_write)(ctx.ptr, &mut writer, (&mut out as *mut Vec<u8>).cast()) }
        .check("cannot write HEIC")
        .map_err(fail)?;
    std::fs::write(path, out)?;
    Ok(())
}
