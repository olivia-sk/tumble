//! Turning command-line paths into a list of files to convert.
//!
//! - A file is always attempted, so an unsupported one is reported.
//! - A folder contributes the files in it that can reach the target and are
//!   the same kind of thing (see `same_kind`); others are skipped quietly.
//!   `-r` descends into subfolders (symlinked folders are not followed).
//! - Outputs go next to each input, or under `-o`. Files found by walking a
//!   folder keep their subfolder layout under `-o`.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use tumble_core::job::{STAGING_PREFIX, path_key};
use tumble_core::{Format, FormatId, Kind, Registry};

pub struct Planned {
    pub input: PathBuf,
    pub out_dir: PathBuf,
}

#[derive(Default)]
pub struct Plan {
    pub jobs: Vec<Planned>,
    /// Command-line paths that do not exist.
    pub missing: Vec<PathBuf>,
    /// Files in folders that cannot become the target.
    pub skipped: usize,
}

pub fn collect(
    paths: &[PathBuf],
    recursive: bool,
    out: Option<&Path>,
    to: FormatId,
    registry: &Registry,
) -> Plan {
    let mut plan = Plan::default();
    let mut seen = HashSet::new();
    for path in paths {
        let path = std::path::absolute(path).unwrap_or_else(|_| path.clone());
        let Ok(meta) = fs::metadata(&path) else {
            plan.missing.push(path);
            continue;
        };
        if meta.is_dir() {
            let mut files = Vec::new();
            walk(&path, recursive, &mut files);
            files.sort();
            for file in files {
                let convertible = Format::of_path(&file).is_some_and(|f| {
                    same_kind(f.kind, to.format().kind) && registry.route(f.id, to).is_some()
                });
                if !convertible {
                    plan.skipped += 1;
                    continue;
                }
                let parent = file.parent().unwrap_or(&path);
                let out_dir = match out {
                    Some(o) => o.join(parent.strip_prefix(&path).unwrap_or(Path::new(""))),
                    None => parent.to_path_buf(),
                };
                push(&mut plan, &mut seen, file, out_dir);
            }
        } else {
            let out_dir = match out {
                Some(o) => o.to_path_buf(),
                None => path.parent().map(Path::to_path_buf).unwrap_or_default(),
            };
            push(&mut plan, &mut seen, path, out_dir);
        }
    }
    plan
}

/// Whether a file found in a folder belongs in a batch converting to a
/// target of `to` kind. Converting a folder of photos to BMP should not also
/// render its text files or grab frames from its videos; files named on the
/// command line are always attempted. Video to audio is the one crossover
/// kept, for pulling soundtracks out of a folder of clips.
fn same_kind(from: Kind, to: Kind) -> bool {
    let group = |k: Kind| match k {
        Kind::StillImage | Kind::AnimatedImage => 0,
        Kind::Video => 1,
        Kind::Audio => 2,
        Kind::Document => 3,
    };
    group(from) == group(to) || (from == Kind::Video && to == Kind::Audio)
}

fn push(plan: &mut Plan, seen: &mut HashSet<String>, input: PathBuf, out_dir: PathBuf) {
    if seen.insert(path_key(&input)) {
        plan.jobs.push(Planned { input, out_dir });
    }
}

fn walk(dir: &Path, recursive: bool, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else { continue };
        let path = entry.path();
        if kind.is_file() {
            files.push(path);
        } else if kind.is_dir()
            && recursive
            && !entry.file_name().to_string_lossy().starts_with(STAGING_PREFIX)
        {
            walk(&path, recursive, files);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_batches_stay_within_a_kind() {
        assert!(same_kind(Kind::StillImage, Kind::StillImage));
        assert!(same_kind(Kind::AnimatedImage, Kind::StillImage), "GIF is an image");
        assert!(same_kind(Kind::Video, Kind::Audio), "soundtracks from clips");
        assert!(!same_kind(Kind::Document, Kind::StillImage), "no rendering stray text files");
        assert!(!same_kind(Kind::Video, Kind::StillImage), "no frame grabs in photo batches");
        assert!(!same_kind(Kind::Audio, Kind::Video));
    }
}
