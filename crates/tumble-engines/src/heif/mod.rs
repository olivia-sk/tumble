//! The libheif engine: HEIC to every image output, and every image input
//! to HEIC when the encoder (x265) is present. libheif and its
//! dependencies are loaded at runtime from next to tumble.

pub mod codec;
mod ffi;

use crate::native;
use crate::raster::{self, READS, WRITES};
use std::path::{Path, PathBuf};
use tumble_core::{CancelToken, ConvertOptions, Engine, EngineError, FormatId, Progress, Step};

const HEIC: FormatId = FormatId("heic");

/// Whether HEIC can be written here (the DLLs, x265 included, are present).
/// File checks only, so other engines can call it while listing steps.
pub fn codec_can_write() -> bool {
    HeifEngine::can_write()
}

pub struct HeifEngine;

impl HeifEngine {
    /// Cheap check (file existence only) so building the registry stays fast.
    fn present() -> bool {
        native::find_dir(ffi::REQUIRED).is_some()
    }

    fn can_write() -> bool {
        native::find_dir(&[ffi::REQUIRED, &[ffi::ENCODER]].concat()).is_some()
    }

    /// One line for `tumble engines`. Loads the DLL to report its version.
    pub fn describe() -> (bool, String) {
        if !Self::present() {
            return (
                false,
                format!(
                    "{} not found {} (run {})",
                    ffi::REQUIRED.join(", "),
                    native::WHERE,
                    native::FETCH
                ),
            );
        }
        match ffi::api() {
            Ok(api) if codec::can_encode() => (true, format!("libheif {}", api.version())),
            Ok(api) => {
                (true, format!("libheif {} (read only: {} missing)", api.version(), ffi::ENCODER))
            }
            Err(e) => (false, e),
        }
    }
}

impl Engine for HeifEngine {
    fn name(&self) -> &'static str {
        "libheif"
    }

    fn available(&self) -> bool {
        Self::present()
    }

    fn steps(&self) -> Vec<Step> {
        let mut steps: Vec<Step> =
            WRITES.iter().map(|&to| Step { from: HEIC, to: FormatId(to) }).collect();
        if Self::can_write() {
            steps.extend(READS.iter().map(|&from| Step { from: FormatId(from), to: HEIC }));
            steps.push(Step { from: HEIC, to: HEIC });
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
        if step.from != HEIC && step.to != HEIC {
            return Err(EngineError::Unsupported(step));
        }
        let img = raster::decode(step.from, input, options)?;
        progress.update(0.5);
        if cancel.is_cancelled() {
            return Err(EngineError::Cancelled);
        }
        raster::save(img, step.from, step.to, options, output)?;
        Ok(vec![output.to_path_buf()])
    }
}
