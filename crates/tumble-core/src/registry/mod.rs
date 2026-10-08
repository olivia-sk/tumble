//! The set of available engines, and routing between formats.

mod route;

pub use route::{Hop, MAX_HOPS, Route};

use crate::engine::{Engine, Step};
use crate::format::FormatId;
use std::collections::HashMap;

/// An engine's step, as an edge in the format graph.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Edge {
    pub engine: usize,
    pub step: Step,
}

#[derive(Default)]
pub struct Registry {
    engines: Vec<Box<dyn Engine>>,
    /// Outgoing edges per source format, in registration order.
    edges: HashMap<FormatId, Vec<Edge>>,
}

impl Registry {
    pub fn new() -> Registry {
        Registry::default()
    }

    /// Adds an engine if it reports `available()`; otherwise drops it.
    pub fn register(&mut self, engine: Box<dyn Engine>) {
        if !engine.available() {
            return;
        }
        let index = self.engines.len();
        for step in engine.steps() {
            self.edges.entry(step.from).or_default().push(Edge { engine: index, step });
        }
        self.engines.push(engine);
    }

    pub fn engines(&self) -> impl Iterator<Item = &dyn Engine> {
        self.engines.iter().map(|e| e.as_ref())
    }

    pub fn engine(&self, index: usize) -> &dyn Engine {
        self.engines[index].as_ref()
    }

    pub fn is_empty(&self) -> bool {
        self.engines.is_empty()
    }

    /// Whether any engine reads this format.
    pub fn reads(&self, format: FormatId) -> bool {
        self.edges.contains_key(&format)
    }

    pub(crate) fn edges_from(&self, format: FormatId) -> &[Edge] {
        self.edges.get(&format).map_or(&[], Vec::as_slice)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::engine::*;
    use std::path::{Path, PathBuf};

    /// An engine that declares steps and never converts anything.
    pub struct Fake {
        pub name: &'static str,
        pub available: bool,
        pub priority: i32,
        pub steps: Vec<Step>,
    }

    impl Fake {
        pub fn new(name: &'static str, steps: &[(&'static str, &'static str)]) -> Fake {
            Fake {
                name,
                available: true,
                priority: 0,
                steps: steps.iter().map(|&(a, b)| Step::new(a, b)).collect(),
            }
        }
    }

    impl Engine for Fake {
        fn name(&self) -> &'static str {
            self.name
        }
        fn available(&self) -> bool {
            self.available
        }
        fn priority(&self) -> i32 {
            self.priority
        }
        fn steps(&self) -> Vec<Step> {
            self.steps.clone()
        }
        fn convert(
            &self,
            step: Step,
            _: &Path,
            _: &Path,
            _: &ConvertOptions,
            _: &dyn Progress,
            _: &CancelToken,
        ) -> Result<Vec<PathBuf>, EngineError> {
            Err(EngineError::Unsupported(step))
        }
    }

    #[test]
    fn unavailable_engines_are_dropped() {
        let mut r = Registry::new();
        assert!(r.is_empty());
        let mut off = Fake::new("off", &[("png", "jpeg")]);
        off.available = false;
        r.register(Box::new(off));
        assert!(r.is_empty());
        assert!(!r.reads(FormatId("png")));
        r.register(Box::new(Fake::new("on", &[("png", "jpeg")])));
        assert_eq!(r.engines().count(), 1);
        assert!(r.reads(FormatId("png")));
    }
}
