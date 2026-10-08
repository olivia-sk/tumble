//! The PDFium engine: PDF to every image output, one image per page.
//! A one-page PDF gives `report.png`; longer ones give `report-p001.png`,
//! `report-p002.png`, ... (PRD section 7). pdfium.dll is loaded at runtime
//! from next to tumble.exe.

mod ffi;
mod render;

use crate::native;
use crate::raster::{self, WRITES};
use render::Pdf;
use std::path::{Path, PathBuf};
use tumble_core::{CancelToken, ConvertOptions, Engine, EngineError, FormatId, Progress, Step};

const PDF: FormatId = FormatId("pdf");

pub struct PdfEngine;

impl PdfEngine {
    fn present() -> bool {
        native::find_dir(&[ffi::DLL]).is_some()
    }

    /// One line for `tumble engines`.
    pub fn describe() -> (bool, String) {
        if !Self::present() {
            return (
                false,
                format!("{} not found next to tumble.exe (run scripts/fetch-vendor.ps1)", ffi::DLL),
            );
        }
        match ffi::lock() {
            Ok(_) => (true, "PDFium".to_string()),
            Err(e) => (false, e),
        }
    }
}

/// `report.png` -> `report-p007.png`, with at least three digits.
fn page_path(output: &Path, page: usize, pages: usize) -> PathBuf {
    let stem = output.file_stem().unwrap_or_default().to_string_lossy();
    let ext = output.extension().unwrap_or_default().to_string_lossy();
    let width = pages.to_string().len().max(3);
    output.with_file_name(format!("{stem}-p{page:0width$}.{ext}"))
}

impl Engine for PdfEngine {
    fn name(&self) -> &'static str {
        "pdfium"
    }

    fn available(&self) -> bool {
        Self::present()
    }

    fn steps(&self) -> Vec<Step> {
        let mut steps: Vec<Step> =
            WRITES.iter().map(|&to| Step { from: PDF, to: FormatId(to) }).collect();
        // Straight to HEIC as well, so a multi-page PDF needs no per-page
        // intermediate files.
        if crate::heif::codec_can_write() {
            steps.push(Step { from: PDF, to: FormatId("heic") });
        }
        steps
    }

    fn convert(
        &self,
        step: Step,
        input: &Path,
        output: &Path,
        options: &ConvertOptions,
        progress: &dyn Progress,
        cancel: &CancelToken,
    ) -> Result<Vec<PathBuf>, EngineError> {
        if step.from != PDF {
            return Err(EngineError::Unsupported(step));
        }
        let pdf = Pdf::open(input)?;
        if pdf.pages == 0 {
            return Err(EngineError::failed("the PDF has no pages"));
        }
        let mut written = Vec::with_capacity(pdf.pages);
        for index in 0..pdf.pages {
            if cancel.is_cancelled() {
                return Err(EngineError::Cancelled);
            }
            let img = pdf.render(index, options.resize)?;
            let path = if pdf.pages == 1 {
                output.to_path_buf()
            } else {
                page_path(output, index + 1, pdf.pages)
            };
            raster::save(img, PDF, step.to, options, &path)?;
            written.push(path);
            progress.update((index + 1) as f32 / pdf.pages as f32);
        }
        Ok(written)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_names() {
        let out = Path::new(r"C:\x\report.png");
        assert_eq!(page_path(out, 1, 3), Path::new(r"C:\x\report-p001.png"));
        assert_eq!(page_path(out, 12, 1500), Path::new(r"C:\x\report-p0012.png"));
    }
}
