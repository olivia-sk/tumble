//! Conversion: `tumble <files or folders...> --to <format>` on the command
//! line, and `tumble convert ...` (hidden), which the right-click menu runs
//! through tumblew.exe (see `menu.rs`).
//!
//! - `setup.rs`   arguments to target and options
//! - `inputs.rs`  paths to a list of jobs
//! - `batch.rs`   running the jobs
//! - `report.rs`  printing (or quietly recording) results

mod batch;
mod inputs;
#[cfg(windows)]
mod menu;
mod report;
mod setup;

use crate::args::ConvertArgs;
use crate::exit;
use report::{Mode, Reporter};
use tumble_core::CancelToken;

#[cfg(windows)]
pub use menu::run_menu;

pub fn run(args: ConvertArgs) -> u8 {
    if let Some(warning) = tumble_core::config::load_warning() {
        eprintln!("warning: {warning}");
    }
    let setup = match setup::from_args(&args) {
        Ok(s) => s,
        Err((msg, code)) => {
            eprintln!("{msg}");
            return code;
        }
    };
    let registry = tumble_engines::default_registry();
    let plan = inputs::collect(
        &args.inputs,
        args.recursive,
        setup.out.as_deref(),
        setup.target.id,
        &registry,
    );

    let reporter = Reporter::new(if args.json { Mode::Json } else { Mode::Human }, None);
    if plan.jobs.is_empty() && plan.missing.is_empty() {
        reporter.summary(plan.skipped);
        eprintln!("nothing to convert to {}", setup.target.name);
        return exit::NO_ROUTE;
    }
    let cancel = CancelToken::new();
    install_ctrl_c(cancel.clone());
    batch::execute(&plan, &setup, args.overwrite, &registry, &reporter, &cancel);
    reporter.summary(plan.skipped);
    batch::exit_code(&reporter)
}

/// First Ctrl+C stops new work and lets running files finish cleanly so
/// temp folders are removed. A second one exits at once.
fn install_ctrl_c(cancel: CancelToken) {
    let _ = ctrlc::set_handler(move || {
        if cancel.is_cancelled() {
            std::process::exit(130);
        }
        cancel.cancel();
        eprintln!("cancelling; press Ctrl+C again to stop immediately");
    });
}
