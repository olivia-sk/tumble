//! The built-in image engine: every raster format it can read converts
//! directly, in memory, to every raster format it can write. The codecs
//! themselves live in `raster/`, shared with the HEIC and PDF engines.

use crate::raster::{self, READS, WRITES};
use std::path::{Path, PathBuf};
use tumble_core::{CancelToken, ConvertOptions, Engine, EngineError, FormatId, Progress, Step};

pub struct ImageEngine;

impl Engine for ImageEngine {
    fn name(&self) -> &'static str {
        "image"
    }

    fn available(&self) -> bool {
        true
    }

    fn steps(&self) -> Vec<Step> {
        let mut steps = Vec::new();
        for &from in READS {
            for &to in WRITES {
                // Same-format steps re-encode (resize, quality). Not GIF:
                // this engine keeps only the first frame.
                if from != "gif" || to != "gif" {
                    steps.push(Step { from: FormatId(from), to: FormatId(to) });
                }
            }
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
        if !READS.contains(&step.from.as_str()) || !WRITES.contains(&step.to.as_str()) {
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
