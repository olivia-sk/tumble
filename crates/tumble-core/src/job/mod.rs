//! Running one conversion: route, scratch folders, engine steps, and moving
//! finished files into place (PRD sections 8 and 10).
//!
//! - Intermediate files go in a scratch folder under the system temp dir.
//! - The last step writes into a staging folder inside the output folder,
//!   so the final move is a same-volume rename and a failed job never leaves
//!   a half-written file under the real name.
//! - Both folders are removed when the job ends, whatever the outcome.

mod error;
mod naming;
mod scratch;

pub use error::JobError;
pub use naming::OutputNamer;
pub use scratch::ScratchDir;

use crate::engine::{CancelToken, ConvertOptions, Progress};
use crate::format::{Format, FormatId};
use crate::registry::{Registry, Route};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

/// Prefix of the staging folders created inside output folders.
pub const STAGING_PREFIX: &str = ".tumble-";

pub struct Request<'a> {
    pub input: &'a Path,
    pub to: FormatId,
    /// Must exist or be creatable. Absolute paths keep engine subprocess
    /// arguments unambiguous.
    pub out_dir: &'a Path,
    pub options: &'a ConvertOptions,
    pub overwrite: bool,
}

#[derive(Debug)]
pub struct Outcome {
    pub outputs: Vec<PathBuf>,
    pub route: Route,
}

/// Converts one file. `progress` receives the fraction of the whole route.
pub fn convert_file(
    registry: &Registry,
    request: &Request,
    namer: &OutputNamer,
    progress: &dyn Progress,
    cancel: &CancelToken,
) -> Result<Outcome, JobError> {
    let input = request.input;
    let from =
        Format::of_path(input).ok_or_else(|| JobError::UnknownFormat(input.to_path_buf()))?.id;
    let route =
        registry.route(from, request.to).ok_or(JobError::NoRoute { from, to: request.to })?;
    if !input.is_file() {
        return Err(JobError::io(
            format!("cannot read {}", input.display()),
            std::io::Error::from(std::io::ErrorKind::NotFound),
        ));
    }

    fs::create_dir_all(request.out_dir)
        .map_err(|e| JobError::io(format!("cannot create {}", request.out_dir.display()), e))?;
    let scratch = if route.hops.len() > 1 {
        Some(
            ScratchDir::new_in(&std::env::temp_dir(), "tumble-")
                .map_err(|e| JobError::io("cannot create a temp folder", e))?,
        )
    } else {
        None
    };
    let staging = ScratchDir::new_in(request.out_dir, STAGING_PREFIX)
        .map_err(|e| JobError::io(format!("cannot write to {}", request.out_dir.display()), e))?;

    let stem = input.file_stem().unwrap_or_default();
    let count = route.hops.len();
    let mut current = input.to_path_buf();
    let mut produced = Vec::new();
    for (i, hop) in route.hops.iter().enumerate() {
        if cancel.is_cancelled() {
            return Err(JobError::Cancelled);
        }
        let ext = hop.step.to.format().primary_extension();
        let output = if i + 1 == count {
            let mut name = OsString::from(stem);
            name.push(".");
            name.push(ext);
            staging.path().join(name)
        } else {
            let dir = scratch.as_ref().expect("multi-hop route has scratch");
            dir.path().join(format!("step{}.{ext}", i + 1))
        };
        let engine = registry.engine(hop.engine);
        let step_progress = StepProgress { inner: progress, index: i, count };
        step_progress.update(0.0);
        let outputs = engine
            .convert(hop.step, &current, &output, request.options, &step_progress, cancel)
            .map_err(|error| JobError::Engine { engine: engine.name(), step: hop.step, error })?;
        step_progress.update(1.0);

        if i + 1 == count {
            produced = outputs;
        } else {
            let [single] = outputs.as_slice() else {
                return Err(JobError::Other(format!(
                    "{} produced {} files where one intermediate was expected",
                    engine.name(),
                    outputs.len()
                )));
            };
            current = single.clone();
        }
    }

    if cancel.is_cancelled() {
        return Err(JobError::Cancelled);
    }
    let mut finals = Vec::with_capacity(produced.len());
    for staged in produced {
        let name = staged.file_name().unwrap_or_default();
        let target = namer.claim(request.out_dir, name, request.overwrite);
        fs::rename(&staged, &target)
            .map_err(|e| JobError::io(format!("cannot write {}", target.display()), e))?;
        finals.push(target);
    }
    drop(staging);
    drop(scratch);
    Ok(Outcome { outputs: finals, route })
}

/// Maps one step's 0..1 progress onto the whole route.
struct StepProgress<'a> {
    inner: &'a dyn Progress,
    index: usize,
    count: usize,
}

impl Progress for StepProgress<'_> {
    fn update(&self, fraction: f32) {
        let f = fraction.clamp(0.0, 1.0);
        self.inner.update((self.index as f32 + f) / self.count as f32);
    }
}

#[cfg(test)]
mod tests;
