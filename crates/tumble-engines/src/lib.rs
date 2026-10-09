//! Converter engines. Every client gets conversions from `default_registry()`;
//! nothing converts a file without going through it.
//!
//! - `image/`  built-in raster engine (always available)
//! - `heif/`   libheif, HEIC in and out (vendor library)
//! - `pdf/`    PDFium, PDF pages to images (vendor library)
//! - `ffmpeg/` FFmpeg, video and audio (the user's ffmpeg)
//! - `office/` LibreOffice, documents (the user's soffice)
//! - `raster/` codecs and pixel handling the engines share
//! - `native`  finding and loading vendor libraries
//! - `tools`   finding external programs
//! - `process_tree` running a program so its whole process tree can be killed

pub mod ffmpeg;
pub mod heif;
pub mod image;
mod native;
pub mod office;
pub mod pdf;
mod process_tree;
pub mod raster;
mod tools;

use tumble_core::{Engine, FormatId, Registry, Step};

fn all_engines() -> Vec<Box<dyn Engine>> {
    vec![
        Box::new(image::ImageEngine),
        Box::new(heif::HeifEngine),
        Box::new(pdf::PdfEngine),
        Box::new(ffmpeg::FfmpegEngine),
        Box::new(office::OfficeEngine),
    ]
}

/// Every engine that is available on this machine.
pub fn default_registry() -> Registry {
    let mut registry = Registry::new();
    for engine in all_engines() {
        registry.register(engine);
    }
    registry
}

/// One row of `tumble engines`.
pub struct EngineReport {
    pub name: &'static str,
    pub available: bool,
    /// Version, or why it is missing and how to get it.
    pub detail: String,
    pub reads: Vec<FormatId>,
    pub writes: Vec<FormatId>,
}

/// Status of every engine Tumble knows, available or not. Slower than
/// `default_registry()` because it loads the DLLs to read their versions.
pub fn engine_report() -> Vec<EngineReport> {
    all_engines()
        .into_iter()
        .map(|engine| {
            let (available, detail) = match engine.name() {
                "libheif" => heif::HeifEngine::describe(),
                "pdfium" => pdf::PdfEngine::describe(),
                "ffmpeg" => ffmpeg::FfmpegEngine::describe(),
                "libreoffice" => office::OfficeEngine::describe(),
                _ => (engine.available(), "built in".to_string()),
            };
            let steps = if available { engine.steps() } else { Vec::new() };
            let side = |pick: fn(&Step) -> FormatId| {
                tumble_core::FORMATS
                    .iter()
                    .map(|f| f.id)
                    .filter(|id| steps.iter().any(|s| pick(s) == *id))
                    .collect()
            };
            EngineReport {
                name: engine.name(),
                available,
                detail,
                reads: side(|s| s.from),
                writes: side(|s| s.to),
            }
        })
        .collect()
}
