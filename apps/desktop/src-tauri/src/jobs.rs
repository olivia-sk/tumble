//! The desktop window's conversion logic, as plain functions so it is
//! tested without a window. Every conversion goes through the same
//! `default_registry()` and `job::convert_file` as the CLI and the menu.

use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tumble_core::job::{self, OutputNamer, Request};
use tumble_core::{CancelToken, ConvertOptions, Format, Progress, Registry, presets};

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct Target {
    pub id: &'static str,
    pub name: &'static str,
}

#[derive(Debug, Serialize, Clone)]
pub struct FileInfo {
    pub path: String,
    pub name: String,
    /// Display name of the detected format, if any.
    pub format: Option<&'static str>,
    pub targets: Vec<Target>,
}

#[derive(Debug, Serialize)]
pub struct PresetInfo {
    pub name: String,
    pub description: String,
    pub to: &'static str,
}

/// What the window keeps between calls.
pub struct Session {
    pub registry: Registry,
    /// Shared by every job, so parallel jobs never pick the same output
    /// name and never write over a queued input.
    pub namer: OutputNamer,
    pub running: Mutex<HashMap<u32, CancelToken>>,
}

impl Session {
    pub fn new() -> Session {
        Session {
            registry: tumble_engines::default_registry(),
            namer: OutputNamer::new(),
            running: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for Session {
    fn default() -> Self {
        Session::new()
    }
}

/// Files under `paths`, folders expanded recursively. Unknown file types
/// inside folders are left out; files dropped directly are always listed,
/// so the window can say why they cannot be converted.
pub fn expand(paths: &[PathBuf]) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let Ok(kind) = e.file_type() else { continue };
            if kind.is_dir() && !e.file_name().to_string_lossy().starts_with(job::STAGING_PREFIX) {
                walk(&e.path(), out);
            } else if kind.is_file() && Format::of_path(&e.path()).is_some() {
                out.push(e.path());
            }
        }
    }
    let mut out = Vec::new();
    for p in paths {
        if p.is_dir() {
            walk(p, &mut out);
        } else {
            out.push(p.clone());
        }
    }
    out
}

pub fn inspect(session: &Session, paths: &[PathBuf]) -> Vec<FileInfo> {
    expand(paths)
        .into_iter()
        .map(|path| {
            let format = Format::of_path(&path);
            let targets = format
                .map(|f| session.registry.targets(f.id))
                .unwrap_or_default()
                .into_iter()
                .map(|t| Target { id: t.as_str(), name: t.format().name })
                .collect();
            session.namer.protect(&path);
            FileInfo {
                name: path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                path: path.display().to_string(),
                format: format.map(|f| f.name),
                targets,
            }
        })
        .collect()
}

pub fn preset_list() -> Vec<PresetInfo> {
    presets::all()
        .0
        .into_iter()
        .map(|p| PresetInfo { name: p.name, description: p.description, to: p.to.as_str() })
        .collect()
}

/// Options from the window, a preset and config.toml, in that order.
pub fn options(
    quality: Option<u8>,
    resize: Option<&str>,
    preset: Option<&str>,
) -> Result<ConvertOptions, String> {
    let resize = match resize.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => Some(s.parse().map_err(|e: String| e)?),
        None => None,
    };
    let mut o = ConvertOptions { quality, resize, ..Default::default() };
    if let Some(name) = preset.filter(|s| !s.is_empty()) {
        presets::find(name).ok_or_else(|| format!("unknown preset {name}"))?.apply(&mut o);
    }
    o.quality = o.quality.or(tumble_core::config::current().quality);
    Ok(o)
}

/// Converts one file. `out_dir` defaults to the file's own folder.
pub fn convert(
    session: &Session,
    input: &Path,
    to: &str,
    options: &ConvertOptions,
    out_dir: Option<&Path>,
    progress: &dyn Progress,
    cancel: &CancelToken,
) -> Result<Vec<PathBuf>, String> {
    let to =
        Format::parse(to).filter(|f| f.output).ok_or_else(|| format!("unknown format {to}"))?;
    let out_dir = match out_dir {
        Some(d) => d.to_path_buf(),
        None => input.parent().map(Path::to_path_buf).unwrap_or_default(),
    };
    let request = Request { input, to: to.id, out_dir: &out_dir, options, overwrite: false };
    job::convert_file(&session.registry, &request, &session.namer, progress, cancel)
        .map(|o| o.outputs)
        .map_err(|e| e.to_string())
}

/// Targets every given file can reach, in table order.
pub fn common_targets(files: &[FileInfo]) -> Vec<Target> {
    let Some(first) = files.first() else { return Vec::new() };
    first.targets.iter().filter(|t| files.iter().all(|f| f.targets.contains(t))).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tumble_core::NoProgress;

    fn png(path: &Path) {
        std::fs::write(
            path,
            // A 1x1 red PNG.
            [
                0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13, 0x49, 0x48, 0x44,
                0x52, 0, 0, 0, 1, 0, 0, 0, 1, 8, 2, 0, 0, 0, 0x90, 0x77, 0x53, 0xDE, 0, 0, 0, 12,
                0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0, 0, 0x03, 0x01, 0x01,
                0, 0x18, 0xDD, 0x8D, 0xB0, 0, 0, 0, 0, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60,
                0x82,
            ],
        )
        .unwrap();
    }

    #[test]
    fn inspect_expands_folders_and_lists_targets() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        png(&dir.path().join("a.png"));
        png(&dir.path().join("sub").join("b.png"));
        std::fs::write(dir.path().join("notes.xyz"), b"?").unwrap();
        let session = Session::new();
        let files = inspect(&session, &[dir.path().to_path_buf()]);
        let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["a.png", "b.png"], "unknown types in folders are skipped");
        assert!(files[0].targets.iter().any(|t| t.id == "webp"));
        assert!(!files[0].targets.iter().any(|t| t.id == "png"), "never its own format");

        let odd = inspect(&session, &[dir.path().join("notes.xyz")]);
        assert_eq!(odd[0].format, None, "dropped directly: listed, with no targets");
        assert!(odd[0].targets.is_empty());
    }

    #[test]
    fn convert_writes_next_to_the_input_and_numbers_names() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("a.png");
        png(&input);
        let session = Session::new();
        inspect(&session, std::slice::from_ref(&input));
        let o = options(None, None, None).unwrap();
        let first =
            convert(&session, &input, "bmp", &o, None, &NoProgress, &CancelToken::new()).unwrap();
        let second =
            convert(&session, &input, "bmp", &o, None, &NoProgress, &CancelToken::new()).unwrap();
        assert_eq!(first, [dir.path().join("a.bmp")]);
        assert_eq!(second, [dir.path().join("a (1).bmp")]);
    }

    #[test]
    fn options_merge_and_validate() {
        let o = options(Some(50), Some("800x600"), Some("web")).unwrap();
        assert_eq!(o.quality, Some(50), "typed beats preset");
        assert_eq!(o.resize.unwrap().max_width, 800);
        assert!(options(None, Some("huge"), None).is_err());
        assert!(options(None, None, Some("nope")).is_err());
        assert!(preset_list().iter().any(|p| p.name == "web"));
    }

    #[test]
    fn common_targets_are_the_intersection() {
        let t = |id| Target { id, name: id };
        let f =
            |targets| FileInfo { path: String::new(), name: String::new(), format: None, targets };
        let files = [f(vec![t("png"), t("webp"), t("gif")]), f(vec![t("webp"), t("png")])];
        assert_eq!(common_targets(&files), [t("png"), t("webp")]);
        assert!(common_targets(&[]).is_empty());
    }
}
