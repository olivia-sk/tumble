//! Routing (PRD section 8): the shortest chain of engine steps from one
//! format to another, at most `MAX_HOPS` long.
//!
//! Rules, in order:
//! 1. Fewest hops wins.
//! 2. A multi-hop route never turns a non-timed format (image, document)
//!    into video or audio. Only a direct step may.
//! 3. Among equally short routes, fewer lossy intermediates wins, so a lossy
//!    format is only passed through when no lossless path of that length exists.
//! 4. Then the higher sum of engine priorities wins.
//! 5. Then the first found, in engine registration order.
//!
//! Same-format conversion (MP4 to a smaller MP4, a resized PNG) is only ever
//! a single step an engine declares for it. `targets` never lists the
//! input's own format, so menus never offer "JPEG to JPEG".

use super::{Edge, Registry};
use crate::engine::Step;
use crate::format::{FORMATS, FormatId};
use std::collections::HashSet;

pub const MAX_HOPS: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hop {
    /// Index into the registry's engines.
    pub engine: usize,
    pub step: Step,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    pub hops: Vec<Hop>,
}

impl Route {
    pub fn from(&self) -> FormatId {
        self.hops[0].step.from
    }

    pub fn to(&self) -> FormatId {
        self.hops[self.hops.len() - 1].step.to
    }

    /// Human-readable form for logs: `svg -> png (image) -> jpeg (image)`.
    pub fn describe(&self, registry: &Registry) -> String {
        let mut s = self.from().to_string();
        for hop in &self.hops {
            s.push_str(&format!(" -> {} ({})", hop.step.to, registry.engine(hop.engine).name()));
        }
        s
    }
}

impl Registry {
    pub fn route(&self, from: FormatId, to: FormatId) -> Option<Route> {
        if from == to {
            let best =
                self.edges_from(from).iter().filter(|e| e.step.to == to).max_by_key(|e| {
                    (self.engine(e.engine).priority(), std::cmp::Reverse(e.engine))
                })?;
            return Some(Route { hops: vec![Hop { engine: best.engine, step: best.step }] });
        }
        for len in 1..=MAX_HOPS {
            let mut best: Option<((usize, i32), Vec<Edge>)> = None;
            self.walk(from, len, &mut Vec::new(), &mut |path| {
                if path[path.len() - 1].step.to != to {
                    return;
                }
                let lossy =
                    path[..path.len() - 1].iter().filter(|e| e.step.to.format().lossy).count();
                let priority: i32 = path.iter().map(|e| self.engine(e.engine).priority()).sum();
                // Lower is better: fewer lossy intermediates, then higher priority.
                let score = (lossy, -priority);
                if best.as_ref().is_none_or(|(b, _)| score < *b) {
                    best = Some((score, path.to_vec()));
                }
            });
            if let Some((_, edges)) = best {
                let hops =
                    edges.into_iter().map(|e| Hop { engine: e.engine, step: e.step }).collect();
                return Some(Route { hops });
            }
        }
        None
    }

    /// Every format `from` can be converted to, in table order.
    pub fn targets(&self, from: FormatId) -> Vec<FormatId> {
        let mut reached = HashSet::new();
        self.walk(from, MAX_HOPS, &mut Vec::new(), &mut |path| {
            reached.insert(path[path.len() - 1].step.to);
        });
        reached.remove(&from);
        FORMATS.iter().map(|f| f.id).filter(|id| reached.contains(id)).collect()
    }

    /// Calls `visit` with every valid simple path from `from` of length 1 up
    /// to `max_len`. `route` deepens one hop at a time, so by the time it
    /// walks length `n` no shorter path reaches its target.
    fn walk(
        &self,
        from: FormatId,
        max_len: usize,
        path: &mut Vec<Edge>,
        visit: &mut dyn FnMut(&[Edge]),
    ) {
        let here = path.last().map_or(from, |e| e.step.to);
        for &edge in self.edges_from(here) {
            let to = edge.step.to;
            let revisits = to == from || path.iter().any(|e| e.step.to == to);
            if revisits || !to.format().output {
                continue;
            }
            path.push(edge);
            if allowed(path) {
                visit(path);
                if path.len() < max_len {
                    self.walk(from, max_len, path, visit);
                }
            }
            path.pop();
        }
    }
}

/// Rule 2. Once a multi-hop path breaks it, every extension does too.
fn allowed(path: &[Edge]) -> bool {
    path.len() == 1
        || !path
            .iter()
            .any(|e| !e.step.from.format().kind.is_timed() && e.step.to.format().kind.is_timed())
}

#[cfg(test)]
mod tests {
    use crate::format::FormatId;
    use crate::registry::Registry;
    use crate::registry::tests::Fake;

    fn reg(engines: Vec<Fake>) -> Registry {
        let mut r = Registry::new();
        for e in engines {
            r.register(Box::new(e));
        }
        r
    }

    fn path(r: &Registry, from: &'static str, to: &'static str) -> Option<Vec<&'static str>> {
        r.route(FormatId(from), FormatId(to)).map(|route| {
            let mut v = vec![route.from().0];
            v.extend(route.hops.iter().map(|h| h.step.to.0));
            v
        })
    }

    #[test]
    fn direct_beats_multi_hop() {
        let r = reg(vec![Fake::new("a", &[("svg", "png"), ("png", "jpeg"), ("svg", "jpeg")])]);
        assert_eq!(path(&r, "svg", "jpeg"), Some(vec!["svg", "jpeg"]));
    }

    #[test]
    fn same_format_only_by_a_declared_step() {
        let r = reg(vec![Fake::new("a", &[("png", "png"), ("png", "jpeg"), ("jpeg", "png")])]);
        assert_eq!(path(&r, "png", "png"), Some(vec!["png", "png"]));
        assert_eq!(path(&r, "jpeg", "jpeg"), None, "jpeg -> png -> jpeg is not a same-format step");
        let targets: Vec<_> = r.targets(FormatId("png")).iter().map(|f| f.0).collect();
        assert_eq!(targets, ["jpeg"], "targets never list the input's own format");
    }

    #[test]
    fn same_format_and_unknown_have_no_route() {
        let r = reg(vec![Fake::new("a", &[("png", "jpeg")])]);
        assert_eq!(path(&r, "png", "png"), None);
        assert_eq!(path(&r, "jpeg", "png"), None);
    }

    #[test]
    fn at_most_three_hops() {
        let r = reg(vec![Fake::new(
            "a",
            &[("svg", "png"), ("png", "bmp"), ("bmp", "tga"), ("tga", "qoi")],
        )]);
        assert_eq!(path(&r, "svg", "tga"), Some(vec!["svg", "png", "bmp", "tga"]));
        assert_eq!(path(&r, "svg", "qoi"), None);
        let targets: Vec<_> = r.targets(FormatId("svg")).iter().map(|f| f.0).collect();
        assert_eq!(targets, ["png", "bmp", "tga"]);
    }

    #[test]
    fn lossless_intermediate_beats_lossy_even_with_lower_priority() {
        let mut lossy = Fake::new("lossy", &[("svg", "jpeg"), ("jpeg", "bmp")]);
        lossy.priority = 10;
        let lossless = Fake::new("lossless", &[("svg", "png"), ("png", "bmp")]);
        let r = reg(vec![lossy, lossless]);
        assert_eq!(path(&r, "svg", "bmp"), Some(vec!["svg", "png", "bmp"]));
    }

    #[test]
    fn priority_breaks_ties() {
        let low = Fake::new("low", &[("png", "jpeg")]);
        let mut high = Fake::new("high", &[("png", "jpeg")]);
        high.priority = 5;
        let r = reg(vec![low, high]);
        let route = r.route(FormatId("png"), FormatId("jpeg")).unwrap();
        assert_eq!(r.engine(route.hops[0].engine).name(), "high");
    }

    #[test]
    fn still_to_video_only_as_a_direct_step() {
        let r = reg(vec![Fake::new(
            "a",
            &[("png", "gif"), ("gif", "mp4"), ("mp4", "mp3"), ("jpeg", "mp4")],
        )]);
        assert_eq!(path(&r, "gif", "mp4"), Some(vec!["gif", "mp4"]));
        assert_eq!(path(&r, "jpeg", "mp4"), Some(vec!["jpeg", "mp4"]));
        assert_eq!(path(&r, "png", "mp4"), None, "png -> gif -> mp4 is two hops");
        assert_eq!(path(&r, "gif", "mp3"), None, "gif -> mp4 -> mp3 is two hops");
        let targets: Vec<_> = r.targets(FormatId("png")).iter().map(|f| f.0).collect();
        assert_eq!(targets, ["gif"]);
    }

    #[test]
    fn video_to_audio_chains_are_fine() {
        let r = reg(vec![Fake::new("a", &[("mkv", "mp4"), ("mp4", "mp3")])]);
        assert_eq!(path(&r, "mkv", "mp3"), Some(vec!["mkv", "mp4", "mp3"]));
    }

    #[test]
    fn input_only_formats_are_never_intermediates() {
        let r = reg(vec![Fake::new("a", &[("png", "svg"), ("svg", "jpeg")])]);
        assert_eq!(path(&r, "png", "jpeg"), None);
    }
}
