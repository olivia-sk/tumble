//! Shared entry point for `tumble` and, on Windows, `tumblew.exe`.

mod args;
mod commands;
pub mod exit;

use args::{Cli, Command};
use clap::Parser;
use std::process::ExitCode;

/// Parses `args` (including the program name) and runs the command.
pub fn run<I, T>(args: I) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(e) => {
            let code = if e.use_stderr() { exit::BAD_ARGS } else { exit::OK };
            let _ = e.print();
            return ExitCode::from(code);
        }
    };

    let code = match cli.command {
        Some(Command::Formats { json }) => commands::formats::run(json),
        Some(Command::Targets { files, menu, json }) => commands::targets::run(&files, menu, json),
        Some(Command::Engines { json }) => commands::engines::run(json),
        Some(Command::Presets) => commands::presets::run(),
        Some(Command::Menu { action }) => commands::menu::run(&action),
        Some(Command::Convert(args)) => commands::convert::run_menu(args),
        Some(Command::Pick { files }) => commands::convert::run_pick(files),
        None => commands::convert::run(cli.convert),
    };
    ExitCode::from(code)
}
