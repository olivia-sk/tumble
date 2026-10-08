//! Command-line definitions.

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use tumble_core::brand;

#[derive(Parser)]
#[command(
    name = brand::CLI_BIN,
    version,
    about = "Convert files locally. Nothing leaves this computer.",
    after_help = "To convert a file whose name starts with a dash, put it after --:\n  tumble --to png -- -draft.jpg",
    args_conflicts_with_subcommands = true,
    subcommand_negates_reqs = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    #[command(flatten)]
    pub convert: ConvertArgs,
}

#[derive(Subcommand)]
pub enum Command {
    /// List every supported format.
    Formats {
        #[arg(long)]
        json: bool,
    },
    /// List the formats a file (or every one of several files) can be
    /// converted to.
    Targets {
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Only the short list shown in the right-click menu.
        #[arg(long)]
        menu: bool,
        #[arg(long)]
        json: bool,
    },
    /// Show which conversion engines are available.
    Engines {
        #[arg(long)]
        json: bool,
    },
    /// List conversion presets.
    Presets,
    /// Manage the right-click menu.
    Menu {
        #[command(subcommand)]
        action: MenuAction,
    },
    /// Convert files (the form the right-click menu uses).
    #[command(hide = true)]
    Convert(ConvertArgs),
    /// Ask which format to convert files to, then convert them (the macOS
    /// Quick Action).
    #[command(hide = true)]
    Pick {
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum MenuAction {
    Install,
    Uninstall,
    Status,
}

#[derive(Args)]
pub struct ConvertArgs {
    /// Files or folders to convert.
    #[arg(required = true)]
    pub inputs: Vec<PathBuf>,
    /// Target format id or extension (webp, .webp). Optional with --preset.
    #[arg(long, required_unless_present = "preset", value_name = "FORMAT")]
    pub to: Option<String>,
    /// Output folder (default: next to each input).
    #[arg(short, long, value_name = "DIR")]
    pub out: Option<PathBuf>,
    /// Recurse into folders.
    #[arg(short = 'r')]
    pub recursive: bool,
    /// Parallel jobs (default: half the CPU cores).
    #[arg(short = 'j', value_name = "N", value_parser = clap::value_parser!(u16).range(1..))]
    pub jobs: Option<u16>,
    /// Quality for lossy outputs (0-100; WebP at 100 is lossless).
    #[arg(short, long, value_parser = clap::value_parser!(u8).range(0..=100))]
    pub quality: Option<u8>,
    /// Shrink images to fit: WxH (1920x1080) or the longest side (2048).
    #[arg(long, value_name = "SIZE", value_parser = clap::value_parser!(tumble_core::Resize))]
    pub resize: Option<tumble_core::Resize>,
    /// Frame time for video to image: seconds or [hh:]mm:ss.
    #[arg(long, value_name = "TIME", value_parser = tumble_core::engine::parse_time)]
    pub at: Option<std::time::Duration>,
    /// Apply a named preset.
    #[arg(long, value_name = "NAME")]
    pub preset: Option<String>,
    /// Replace existing outputs instead of numbering.
    #[arg(long)]
    pub overwrite: bool,
    /// One JSON progress event per line on stdout.
    #[arg(long)]
    pub json: bool,
}
