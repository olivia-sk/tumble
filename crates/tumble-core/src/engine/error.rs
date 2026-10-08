use super::Step;
use std::fmt;

#[derive(Debug)]
pub enum EngineError {
    Cancelled,
    Unsupported(Step),
    Io(std::io::Error),
    /// Anything else, with a message suitable for the log.
    Failed(String),
}

impl EngineError {
    pub fn failed(msg: impl fmt::Display) -> EngineError {
        EngineError::Failed(msg.to_string())
    }
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::Cancelled => f.write_str("cancelled"),
            EngineError::Unsupported(s) => write!(f, "unsupported step {} -> {}", s.from, s.to),
            EngineError::Io(e) => write!(f, "I/O error: {e}"),
            EngineError::Failed(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for EngineError {}

impl From<std::io::Error> for EngineError {
    fn from(e: std::io::Error) -> Self {
        EngineError::Io(e)
    }
}
