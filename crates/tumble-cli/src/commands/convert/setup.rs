//! Turning conversion arguments into a target and options. Typed options
//! win, then the preset, then config.toml.

use crate::args::ConvertArgs;
use crate::exit;
use std::path::PathBuf;
use tumble_core::{ConvertOptions, Format, presets};

pub struct Setup {
    pub target: &'static Format,
    pub options: ConvertOptions,
    /// Fixed output folder (from -o or config.toml), absolute.
    pub out: Option<PathBuf>,
    pub threads: usize,
}

/// Parses `args`; on failure returns the message and exit code.
pub fn from_args(args: &ConvertArgs) -> Result<Setup, (String, u8)> {
    let config = tumble_core::config::current();
    let preset = match args.preset.as_deref() {
        None => None,
        Some(name) => Some(presets::find(name).ok_or_else(|| {
            (format!("unknown preset {name:?}; run `tumble presets` for the list"), exit::BAD_ARGS)
        })?),
    };
    let target = match (args.to.as_deref(), &preset) {
        (Some(to), _) => match Format::parse(to) {
            Some(f) if f.output => f,
            Some(f) => {
                return Err((format!("{} can be read but not written", f.name), exit::BAD_ARGS));
            }
            None => {
                return Err((
                    format!("unknown format {to:?}; run `tumble formats` for the list"),
                    exit::BAD_ARGS,
                ));
            }
        },
        (None, Some(p)) => p.to.format(),
        (None, None) => unreachable!("clap requires --to or --preset"),
    };

    let mut options = ConvertOptions {
        quality: args.quality,
        resize: args.resize,
        at: args.at,
        ..Default::default()
    };
    if let Some(p) = &preset {
        p.apply(&mut options);
    }
    options.quality = options.quality.or(config.quality);

    let out = args
        .out
        .as_deref()
        .or(config.output_dir())
        .map(|o| std::path::absolute(o).unwrap_or_else(|_| o.to_path_buf()));
    let threads =
        args.jobs.or(config.jobs).filter(|&j| j > 0).map_or_else(default_jobs, usize::from);
    Ok(Setup { target, options, out, threads })
}

/// Half the logical CPUs, at least one.
fn default_jobs() -> usize {
    std::thread::available_parallelism().map_or(1, |n| (n.get() / 2).max(1))
}
