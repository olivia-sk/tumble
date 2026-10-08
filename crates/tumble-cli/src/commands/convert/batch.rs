//! Running a planned batch in parallel and turning the tally into an exit
//! code. Shared by the command line and the right-click menu.

use super::inputs::Plan;
use super::report::Reporter;
use super::setup::Setup;
use crate::exit;
use rayon::prelude::*;
use std::sync::atomic::Ordering;
use tumble_core::job::{self, OutputNamer, Request};
use tumble_core::{CancelToken, Registry};

pub fn execute(
    plan: &Plan,
    setup: &Setup,
    overwrite: bool,
    registry: &Registry,
    reporter: &Reporter,
    cancel: &CancelToken,
) {
    for path in &plan.missing {
        reporter.error(path, "file or folder not found", false);
    }
    let namer = OutputNamer::new();
    for job in &plan.jobs {
        namer.protect(&job.input);
    }
    let work = || {
        plan.jobs.par_iter().for_each(|planned| {
            let request = Request {
                input: &planned.input,
                to: setup.target.id,
                out_dir: &planned.out_dir,
                options: &setup.options,
                overwrite,
            };
            reporter.start(&planned.input);
            let progress = reporter.progress(&planned.input);
            match job::convert_file(registry, &request, &namer, &progress, cancel) {
                Ok(outcome) => reporter.done(
                    &planned.input,
                    &outcome.outputs,
                    &outcome.route.describe(registry),
                ),
                Err(e) => reporter.error(&planned.input, &e.to_string(), e.is_unsupported()),
            }
        })
    };
    match rayon::ThreadPoolBuilder::new().num_threads(setup.threads).build() {
        Ok(pool) => pool.install(work),
        Err(_) => work(),
    }
}

/// 1 if anything failed, else 3 if anything had no route, else 0.
pub fn exit_code(reporter: &Reporter) -> u8 {
    let tally = &reporter.tally;
    if tally.failed.load(Ordering::Relaxed) > 0 {
        exit::SOME_FAILED
    } else if tally.unsupported.load(Ordering::Relaxed) > 0 {
        exit::NO_ROUTE
    } else {
        exit::OK
    }
}
