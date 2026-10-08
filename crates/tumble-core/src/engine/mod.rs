//! The `Engine` trait every converter implements.

mod control;
mod error;
mod options;

pub use control::{CancelToken, NoProgress, Progress};
pub use error::EngineError;
pub use options::{ConvertOptions, Resize, parse_time};

use crate::format::FormatId;
use std::path::{Path, PathBuf};

/// One direct conversion an engine can perform.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Step {
    pub from: FormatId,
    pub to: FormatId,
}

impl Step {
    pub const fn new(from: &'static str, to: &'static str) -> Step {
        Step { from: FormatId(from), to: FormatId(to) }
    }
}

pub trait Engine: Send + Sync {
    fn name(&self) -> &'static str;

    /// Whether the engine can run on this machine (DLL present, tool found).
    fn available(&self) -> bool;

    /// Breaks routing ties: the route with the higher sum wins.
    fn priority(&self) -> i32 {
        0
    }

    fn steps(&self) -> Vec<Step>;

    /// Runs one step. `output` is the file to write; engines that produce
    /// several files (one per page) derive their names from it. Returns
    /// every file written.
    fn convert(
        &self,
        step: Step,
        input: &Path,
        output: &Path,
        options: &ConvertOptions,
        progress: &dyn Progress,
        cancel: &CancelToken,
    ) -> Result<Vec<PathBuf>, EngineError>;
}
