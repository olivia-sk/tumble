//! The FFmpeg engine: video and audio between each other, video to images
//! (one frame, or an animated GIF), and GIF to video. Runs the user's own
//! `ffmpeg.exe` and `ffprobe.exe` (see `tools.rs` for how they are found);
//! Tumble never installs or downloads them.

pub mod plan;
pub mod probe;
mod process;

use crate::raster;
use crate::tools::{self, Tool};
use plan::Plan;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tumble_core::format::FORMATS;
use tumble_core::{
    CancelToken, ConvertOptions, Engine, EngineError, FormatId, Kind, Progress, Step,
};

pub const INSTALL_HINT: &str = "winget install Gyan.FFmpeg";

const FFMPEG: Tool = Tool { exe: "ffmpeg.exe", env: "FFMPEG" };
const FFPROBE: Tool = Tool { exe: "ffprobe.exe", env: "FFPROBE" };

/// Where winget, scoop, Chocolatey and manual installs usually put FFmpeg.
fn known_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        let winget = local.join(r"Microsoft\WinGet");
        for package in tools::glob_dirs(&winget.join("Packages"), "Gyan.FFmpeg", &[]) {
            dirs.extend(tools::glob_dirs(&package, "ffmpeg-", &["bin"]));
        }
        dirs.push(winget.join("Links"));
    }
    if let Some(home) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
        dirs.push(home.join(r"scoop\apps\ffmpeg\current\bin"));
    }
    dirs.push(PathBuf::from(r"C:\ProgramData\chocolatey\bin"));
    dirs.push(PathBuf::from(r"C:\Program Files\ffmpeg\bin"));
    dirs.push(PathBuf::from(r"C:\ffmpeg\bin"));
    dirs
}

/// ffmpeg.exe and ffprobe.exe, found once per process.
pub fn tools() -> Option<&'static (PathBuf, PathBuf)> {
    static FOUND: OnceLock<Option<(PathBuf, PathBuf)>> = OnceLock::new();
    FOUND
        .get_or_init(|| {
            let cfg = &tumble_core::config::current().tools;
            let known = known_dirs();
            let ffmpeg = FFMPEG.find(cfg.ffmpeg.as_deref(), &known)?;
            // Prefer the ffprobe that sits beside the ffmpeg we found.
            let beside = ffmpeg.with_file_name(FFPROBE.exe);
            let ffprobe = std::env::var_os(tumble_core::brand::env_var(FFPROBE.env))
                .map(PathBuf::from)
                .filter(|p| p.is_file())
                .or_else(|| cfg.ffprobe.clone().filter(|p| p.is_file()))
                .or_else(|| beside.is_file().then_some(beside))
                .or_else(|| FFPROBE.find(None, &known))?;
            Some((ffmpeg, ffprobe))
        })
        .as_ref()
}

fn ids(kind: Kind) -> impl Iterator<Item = FormatId> {
    FORMATS.iter().filter(move |f| f.kind == kind).map(|f| f.id)
}

pub struct FfmpegEngine;

impl FfmpegEngine {
    /// One line for `tumble engines`: version and path, or how to install.
    pub fn describe() -> (bool, String) {
        let Some((ffmpeg, _)) = tools() else {
            return (
                false,
                format!("ffmpeg.exe and ffprobe.exe not found; install with: {INSTALL_HINT}"),
            );
        };
        let version = tools::command(ffmpeg)
            .arg("-version")
            .stdin(std::process::Stdio::null())
            .output()
            .ok()
            .and_then(|o| String::from_utf8_lossy(&o.stdout).lines().next().map(str::to_string))
            .and_then(|l| l.split_whitespace().nth(2).map(str::to_string))
            .unwrap_or_else(|| "unknown version".into());
        (true, format!("FFmpeg {version} ({})", ffmpeg.display()))
    }
}

impl Engine for FfmpegEngine {
    fn name(&self) -> &'static str {
        "ffmpeg"
    }

    fn available(&self) -> bool {
        tools().is_some()
    }

    fn steps(&self) -> Vec<Step> {
        // Same-format steps (MP4 to MP4) re-encode with the given options,
        // or copy the streams when there are none.
        let mut steps = Vec::new();
        let mut add = |from: FormatId, to: FormatId| steps.push(Step { from, to });
        // Video to video, audio and every image output (HEIC too when
        // libheif can write it).
        let mut images: Vec<FormatId> = raster::WRITES.iter().map(|&s| FormatId(s)).collect();
        if crate::heif::codec_can_write() {
            images.push(FormatId("heic"));
        }
        for from in ids(Kind::Video) {
            for to in ids(Kind::Video).chain(ids(Kind::Audio)).chain(images.iter().copied()) {
                add(from, to);
            }
        }
        for to in ids(Kind::Video) {
            add(FormatId("gif"), to);
        }
        for from in ids(Kind::Audio) {
            for to in ids(Kind::Audio) {
                add(from, to);
            }
        }
        steps
    }

    fn convert(
        &self,
        step: Step,
        input: &Path,
        output: &Path,
        options: &ConvertOptions,
        progress: &dyn Progress,
        cancel: &CancelToken,
    ) -> Result<Vec<PathBuf>, EngineError> {
        let (ffmpeg, ffprobe) = tools().ok_or_else(|| {
            EngineError::failed(format!("FFmpeg is not installed ({INSTALL_HINT})"))
        })?;
        let media = probe::probe(ffprobe, input)?;
        let still = step.to.format().kind == Kind::StillImage;
        // A frame is grabbed as PNG next to the output, then converted.
        let target =
            if still { output.with_file_name(".tumble-frame.png") } else { output.to_path_buf() };
        let plan = plan::build(step.from, step.to, &media, options, input, &target)?;
        match plan {
            Plan::Encode(args) => {
                process::run(ffmpeg, &args, media.duration, progress, cancel)?;
            }
            Plan::Frame(args) => {
                process::run(ffmpeg, &args, None, progress, cancel)?;
                let result = raster::decode(FormatId("png"), &target, options)
                    .and_then(|img| raster::save(img, step.from, step.to, options, output));
                let _ = std::fs::remove_file(&target);
                result?;
            }
        }
        if !output.is_file() {
            return Err(EngineError::failed("FFmpeg finished without writing a file"));
        }
        Ok(vec![output.to_path_buf()])
    }
}
