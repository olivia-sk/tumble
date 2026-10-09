//! The slice of the libheif C API (1.23) that Tumble uses, loaded at
//! runtime from `heif.dll` (Windows), `libheif.1.dylib` (macOS) or
//! `libheif.so.1` (Linux). Declarations follow libheif's public headers.

use crate::native;
use libloading::Library;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::sync::OnceLock;

/// The libheif library itself.
#[cfg(windows)]
pub const LIB: &str = "heif.dll";
#[cfg(target_os = "macos")]
pub const LIB: &str = "libheif.1.dylib";
#[cfg(not(any(windows, target_os = "macos")))]
pub const LIB: &str = "libheif.so.1";

/// libheif and the libraries it loads. The x265 encoder is only present in
/// the GPL build and only needed for writing, so it is checked separately.
#[cfg(windows)]
pub const REQUIRED: &[&str] = &[LIB, "libde265.dll", "aom.dll"];
#[cfg(windows)]
pub const ENCODER: &str = "libx265.dll";
#[cfg(target_os = "macos")]
pub const REQUIRED: &[&str] = &[LIB, "libde265.0.dylib"];
#[cfg(target_os = "macos")]
pub const ENCODER: &str = "libx265.199.dylib";
#[cfg(not(any(windows, target_os = "macos")))]
pub const REQUIRED: &[&str] = &[LIB, "libde265.so.0"];
#[cfg(not(any(windows, target_os = "macos")))]
pub const ENCODER: &str = "libx265.so.199";

#[repr(C)]
pub struct Context {
    _private: [u8; 0],
}
#[repr(C)]
pub struct Handle {
    _private: [u8; 0],
}
#[repr(C)]
pub struct Image {
    _private: [u8; 0],
}
#[repr(C)]
pub struct Encoder {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Error {
    pub code: c_int,
    pub subcode: c_int,
    pub message: *const c_char,
}

impl Error {
    pub fn check(self, what: &str) -> Result<(), String> {
        if self.code == 0 {
            return Ok(());
        }
        let msg = if self.message.is_null() {
            String::new()
        } else {
            // SAFETY: libheif guarantees a NUL-terminated message string.
            unsafe { CStr::from_ptr(self.message) }.to_string_lossy().into_owned()
        };
        Err(format!("{what}: {msg} (libheif error {}.{})", self.code, self.subcode))
    }
}

/// `heif_writer`: libheif calls `write` with the encoded file.
#[repr(C)]
pub struct Writer {
    pub api_version: c_int,
    pub write: unsafe extern "C" fn(*mut Context, *const c_void, usize, *mut c_void) -> Error,
}

pub const COLORSPACE_RGB: c_int = 1;
pub const CHROMA_RGB: c_int = 10;
pub const CHROMA_RGBA: c_int = 11;
pub const CHROMA_RRGGBB_LE: c_int = 14;
pub const CHROMA_RRGGBBAA_LE: c_int = 15;
pub const CHANNEL_INTERLEAVED: c_int = 10;
pub const COMPRESSION_HEVC: c_int = 1;

#[allow(clippy::type_complexity)]
pub struct Api {
    pub get_version: unsafe extern "C" fn() -> *const c_char,
    pub context_alloc: unsafe extern "C" fn() -> *mut Context,
    pub context_free: unsafe extern "C" fn(*mut Context),
    pub read_from_memory_without_copy:
        unsafe extern "C" fn(*mut Context, *const c_void, usize, *const c_void) -> Error,
    pub get_primary_image_handle: unsafe extern "C" fn(*mut Context, *mut *mut Handle) -> Error,
    pub handle_release: unsafe extern "C" fn(*const Handle),
    pub handle_has_alpha: unsafe extern "C" fn(*const Handle) -> c_int,
    pub handle_luma_bits: unsafe extern "C" fn(*const Handle) -> c_int,
    pub decode_image:
        unsafe extern "C" fn(*const Handle, *mut *mut Image, c_int, c_int, *const c_void) -> Error,
    pub image_width: unsafe extern "C" fn(*const Image) -> c_int,
    pub image_height: unsafe extern "C" fn(*const Image) -> c_int,
    pub plane_readonly: unsafe extern "C" fn(*const Image, c_int, *mut usize) -> *const u8,
    pub plane: unsafe extern "C" fn(*mut Image, c_int, *mut usize) -> *mut u8,
    pub image_release: unsafe extern "C" fn(*const Image),
    pub image_create: unsafe extern "C" fn(c_int, c_int, c_int, c_int, *mut *mut Image) -> Error,
    pub image_add_plane: unsafe extern "C" fn(*mut Image, c_int, c_int, c_int, c_int) -> Error,
    pub have_encoder: unsafe extern "C" fn(c_int) -> c_int,
    pub get_encoder: unsafe extern "C" fn(*mut Context, c_int, *mut *mut Encoder) -> Error,
    pub encoder_lossy_quality: unsafe extern "C" fn(*mut Encoder, c_int) -> Error,
    pub encoder_lossless: unsafe extern "C" fn(*mut Encoder, c_int) -> Error,
    pub encoder_release: unsafe extern "C" fn(*mut Encoder),
    pub encode_image: unsafe extern "C" fn(
        *mut Context,
        *const Image,
        *mut Encoder,
        *const c_void,
        *mut *mut Handle,
    ) -> Error,
    pub context_write: unsafe extern "C" fn(*mut Context, *mut Writer, *mut c_void) -> Error,
    /// Keeps the DLL loaded for as long as the pointers above are used.
    _lib: Library,
}

// SAFETY: the table only holds function pointers into a library that stays
// loaded; libheif is safe to call from several threads on separate contexts.
unsafe impl Send for Api {}
unsafe impl Sync for Api {}

/// The loaded API, or why it could not be loaded. Loaded once per process.
pub fn api() -> Result<&'static Api, String> {
    static API: OnceLock<Result<Api, String>> = OnceLock::new();
    API.get_or_init(load).as_ref().map_err(Clone::clone)
}

fn load() -> Result<Api, String> {
    let dir = native::find_dir(REQUIRED)
        .ok_or_else(|| format!("{} not found {}", REQUIRED.join(", "), native::WHERE))?;
    let lib = native::load(&dir.join(LIB))?;
    // SAFETY: each type matches the libheif 1.23 header declaration.
    unsafe {
        Ok(Api {
            get_version: native::symbol(&lib, "heif_get_version")?,
            context_alloc: native::symbol(&lib, "heif_context_alloc")?,
            context_free: native::symbol(&lib, "heif_context_free")?,
            read_from_memory_without_copy: native::symbol(
                &lib,
                "heif_context_read_from_memory_without_copy",
            )?,
            get_primary_image_handle: native::symbol(
                &lib,
                "heif_context_get_primary_image_handle",
            )?,
            handle_release: native::symbol(&lib, "heif_image_handle_release")?,
            handle_has_alpha: native::symbol(&lib, "heif_image_handle_has_alpha_channel")?,
            handle_luma_bits: native::symbol(&lib, "heif_image_handle_get_luma_bits_per_pixel")?,
            decode_image: native::symbol(&lib, "heif_decode_image")?,
            image_width: native::symbol(&lib, "heif_image_get_primary_width")?,
            image_height: native::symbol(&lib, "heif_image_get_primary_height")?,
            plane_readonly: native::symbol(&lib, "heif_image_get_plane_readonly2")?,
            plane: native::symbol(&lib, "heif_image_get_plane2")?,
            image_release: native::symbol(&lib, "heif_image_release")?,
            image_create: native::symbol(&lib, "heif_image_create")?,
            image_add_plane: native::symbol(&lib, "heif_image_add_plane")?,
            have_encoder: native::symbol(&lib, "heif_have_encoder_for_format")?,
            get_encoder: native::symbol(&lib, "heif_context_get_encoder_for_format")?,
            encoder_lossy_quality: native::symbol(&lib, "heif_encoder_set_lossy_quality")?,
            encoder_lossless: native::symbol(&lib, "heif_encoder_set_lossless")?,
            encoder_release: native::symbol(&lib, "heif_encoder_release")?,
            encode_image: native::symbol(&lib, "heif_context_encode_image")?,
            context_write: native::symbol(&lib, "heif_context_write")?,
            _lib: lib,
        })
    }
}

impl Api {
    pub fn version(&self) -> String {
        // SAFETY: returns a static NUL-terminated string.
        unsafe { CStr::from_ptr((self.get_version)()) }.to_string_lossy().into_owned()
    }
}
