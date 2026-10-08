use crate::engine::{EngineError, Step};
use crate::format::FormatId;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum JobError {
    UnknownFormat(PathBuf),
    NoRoute { from: FormatId, to: FormatId },
    Cancelled,
    Io { context: String, source: std::io::Error },
    Engine { engine: &'static str, step: Step, error: EngineError },
    Other(String),
}

impl JobError {
    pub fn io(context: impl Into<String>, source: std::io::Error) -> JobError {
        JobError::Io { context: context.into(), source }
    }

    /// No route or no engine: the conversion was never possible here, as
    /// opposed to failing while running.
    pub fn is_unsupported(&self) -> bool {
        matches!(self, JobError::UnknownFormat(_) | JobError::NoRoute { .. })
    }
}

impl fmt::Display for JobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JobError::UnknownFormat(path) => {
                write!(f, "unsupported file type: {}", path.display())
            }
            JobError::NoRoute { from, to } => {
                write!(f, "cannot convert {} to {}", from.format().name, to.format().name)
            }
            JobError::Cancelled => f.write_str("cancelled"),
            JobError::Io { context, source } => write!(f, "{context}: {source}"),
            JobError::Engine { engine, step, error } => {
                write!(f, "{engine} ({} -> {}): {error}", step.from, step.to)
            }
            JobError::Other(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for JobError {}
