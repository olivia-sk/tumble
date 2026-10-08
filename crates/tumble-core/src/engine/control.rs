//! Progress reporting and cancellation shared between a job and its engines.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Receives progress from a running conversion.
pub trait Progress: Send + Sync {
    /// `fraction` is 0.0 to 1.0 for the current step.
    fn update(&self, fraction: f32);
}

/// A `Progress` that discards updates.
pub struct NoProgress;

impl Progress for NoProgress {
    fn update(&self, _fraction: f32) {}
}

/// Shared flag that asks running conversions to stop.
#[derive(Clone, Debug, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> CancelToken {
        CancelToken::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
