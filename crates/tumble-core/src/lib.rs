//! Platform-free core of Tumble: the format table, the `Engine` trait, the
//! registry with routing, and the job runner. This crate must never depend on
//! native libraries or Windows APIs.

pub mod brand;
pub mod config;
pub mod engine;
pub mod format;
pub mod job;
pub mod menu;
pub mod presets;
pub mod registry;

pub use engine::{
    CancelToken, ConvertOptions, Engine, EngineError, NoProgress, Progress, Resize, Step,
};
pub use format::{FORMATS, Format, FormatId, Kind};
pub use registry::{Registry, Route};
