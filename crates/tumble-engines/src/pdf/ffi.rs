//! The slice of the PDFium C API that Tumble uses, loaded at runtime from
//! `pdfium.dll`, `libpdfium.dylib` or `libpdfium.so`. Declarations follow PDFium's public/fpdfview.h.
//!
//! PDFium is not thread-safe: every call goes through `lock()`.

use crate::native;
use libloading::Library;
use std::ffi::{c_char, c_int, c_ulong, c_void};
use std::sync::{Mutex, MutexGuard, OnceLock};

#[cfg(windows)]
pub const DLL: &str = "pdfium.dll";
#[cfg(target_os = "macos")]
pub const DLL: &str = "libpdfium.dylib";
#[cfg(not(any(windows, target_os = "macos")))]
pub const DLL: &str = "libpdfium.so";

#[repr(C)]
pub struct Document {
    _private: [u8; 0],
}
#[repr(C)]
pub struct Page {
    _private: [u8; 0],
}
#[repr(C)]
pub struct Bitmap {
    _private: [u8; 0],
}

/// FPDF_GetLastError codes.
pub const ERR_FILE: c_ulong = 2;
pub const ERR_FORMAT: c_ulong = 3;
pub const ERR_PASSWORD: c_ulong = 4;
pub const ERR_SECURITY: c_ulong = 5;

/// FPDF_RenderPageBitmap flag: draw annotations (form fields, stamps).
pub const RENDER_ANNOTATIONS: c_int = 0x01;

pub struct Api {
    pub load_mem_document64:
        unsafe extern "C" fn(*const c_void, usize, *const c_char) -> *mut Document,
    pub get_last_error: unsafe extern "C" fn() -> c_ulong,
    pub close_document: unsafe extern "C" fn(*mut Document),
    pub get_page_count: unsafe extern "C" fn(*mut Document) -> c_int,
    pub load_page: unsafe extern "C" fn(*mut Document, c_int) -> *mut Page,
    pub close_page: unsafe extern "C" fn(*mut Page),
    pub page_width: unsafe extern "C" fn(*mut Page) -> f32,
    pub page_height: unsafe extern "C" fn(*mut Page) -> f32,
    pub bitmap_create: unsafe extern "C" fn(c_int, c_int, c_int) -> *mut Bitmap,
    pub bitmap_fill_rect:
        unsafe extern "C" fn(*mut Bitmap, c_int, c_int, c_int, c_int, c_ulong) -> c_int,
    pub render_page_bitmap:
        unsafe extern "C" fn(*mut Bitmap, *mut Page, c_int, c_int, c_int, c_int, c_int, c_int),
    pub bitmap_buffer: unsafe extern "C" fn(*mut Bitmap) -> *mut c_void,
    pub bitmap_stride: unsafe extern "C" fn(*mut Bitmap) -> c_int,
    pub bitmap_destroy: unsafe extern "C" fn(*mut Bitmap),
    _lib: Library,
}

// SAFETY: function pointers into a library that stays loaded; callers
// serialise every call through `lock()`.
unsafe impl Send for Api {}
unsafe impl Sync for Api {}

/// Exclusive access to PDFium for the duration of the guard.
pub struct Locked {
    pub api: &'static Api,
    _guard: MutexGuard<'static, ()>,
}

pub fn lock() -> Result<Locked, String> {
    static API: OnceLock<Result<Api, String>> = OnceLock::new();
    static LOCK: Mutex<()> = Mutex::new(());
    let api = API.get_or_init(load).as_ref().map_err(Clone::clone)?;
    let guard = LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    Ok(Locked { api, _guard: guard })
}

fn load() -> Result<Api, String> {
    let dir =
        native::find_dir(&[DLL]).ok_or_else(|| format!("{DLL} not found {}", native::WHERE))?;
    let lib = native::load(&dir.join(DLL))?;
    // SAFETY: each type matches the fpdfview.h declaration.
    unsafe {
        let init: unsafe extern "C" fn() = native::symbol(&lib, "FPDF_InitLibrary")?;
        let api = Api {
            load_mem_document64: native::symbol(&lib, "FPDF_LoadMemDocument64")?,
            get_last_error: native::symbol(&lib, "FPDF_GetLastError")?,
            close_document: native::symbol(&lib, "FPDF_CloseDocument")?,
            get_page_count: native::symbol(&lib, "FPDF_GetPageCount")?,
            load_page: native::symbol(&lib, "FPDF_LoadPage")?,
            close_page: native::symbol(&lib, "FPDF_ClosePage")?,
            page_width: native::symbol(&lib, "FPDF_GetPageWidthF")?,
            page_height: native::symbol(&lib, "FPDF_GetPageHeightF")?,
            bitmap_create: native::symbol(&lib, "FPDFBitmap_Create")?,
            bitmap_fill_rect: native::symbol(&lib, "FPDFBitmap_FillRect")?,
            render_page_bitmap: native::symbol(&lib, "FPDF_RenderPageBitmap")?,
            bitmap_buffer: native::symbol(&lib, "FPDFBitmap_GetBuffer")?,
            bitmap_stride: native::symbol(&lib, "FPDFBitmap_GetStride")?,
            bitmap_destroy: native::symbol(&lib, "FPDFBitmap_Destroy")?,
            _lib: lib,
        };
        // Once per process, before any other call; never torn down.
        init();
        Ok(api)
    }
}
