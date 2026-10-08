//! Formats: identifiers, kinds and lookups. The table itself lives in
//! `table.rs`; the PRD section 7 conversion spec lives in `targets.rs`.

mod table;
mod targets;

pub use table::FORMATS;

use serde::Serialize;
use std::fmt;
use std::path::Path;

/// Stable identifier of a format, e.g. `jpeg` or `mp4`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct FormatId(pub &'static str);

impl FormatId {
    pub fn as_str(self) -> &'static str {
        self.0
    }

    /// The table entry for this id. Panics on an id that is not in `FORMATS`,
    /// which is a programming error.
    pub fn format(self) -> &'static Format {
        Format::by_id(self.0).unwrap_or_else(|| panic!("unknown format id {:?}", self.0))
    }
}

impl fmt::Debug for FormatId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl fmt::Display for FormatId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    StillImage,
    AnimatedImage,
    Video,
    Audio,
    Document,
}

impl Kind {
    /// Video or audio: formats with a time axis.
    pub fn is_timed(self) -> bool {
        matches!(self, Kind::Video | Kind::Audio)
    }
}

/// Finer grouping than `Kind`, used for the conversion tables and the
/// right-click short lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Family {
    Image,
    Vector,
    Video,
    Audio,
    Pdf,
    WordProcessing,
    PlainText,
    Html,
    Markdown,
    Presentation,
    Spreadsheet,
}

#[derive(Debug, Serialize)]
pub struct Format {
    pub id: FormatId,
    pub name: &'static str,
    pub kind: Kind,
    pub family: Family,
    /// Lower-case, without the dot. The first one is used for output files.
    pub extensions: &'static [&'static str],
    /// Whether encoding to this format can discard information. Routing
    /// avoids lossy intermediates when a lossless path of equal length exists.
    pub lossy: bool,
    /// False for input-only formats such as SVG.
    pub output: bool,
}

impl Format {
    pub fn by_id(id: &str) -> Option<&'static Format> {
        FORMATS.iter().find(|f| f.id.0 == id)
    }

    /// Looks up a format by extension, with or without a leading dot.
    pub fn by_extension(ext: &str) -> Option<&'static Format> {
        let ext = ext.trim_start_matches('.').to_ascii_lowercase();
        FORMATS.iter().find(|f| f.extensions.contains(&ext.as_str()))
    }

    /// The format of a file, judged by its extension.
    pub fn of_path(path: &Path) -> Option<&'static Format> {
        Format::by_extension(path.extension()?.to_str()?)
    }

    /// Accepts what a user types after `--to`: an id (`jpeg`) or an
    /// extension (`jpg`, `.jpg`).
    pub fn parse(s: &str) -> Option<&'static Format> {
        let lower = s.trim_start_matches('.').to_ascii_lowercase();
        Format::by_id(&lower).or_else(|| Format::by_extension(&lower))
    }

    /// Extension used for output files.
    pub fn primary_extension(&self) -> &'static str {
        self.extensions[0]
    }
}

/// Every format an image can be written as (PRD: "Image outputs").
pub fn image_outputs() -> Vec<FormatId> {
    FORMATS.iter().filter(|f| f.family == Family::Image && f.output).map(|f| f.id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_and_extensions_are_unique() {
        let mut ids = HashSet::new();
        let mut exts = HashSet::new();
        for f in FORMATS {
            assert!(ids.insert(f.id), "duplicate id {}", f.id);
            assert!(!f.extensions.is_empty(), "{} has no extensions", f.id);
            for e in f.extensions {
                assert_eq!(*e, e.to_ascii_lowercase(), "extension {e} must be lower case");
                assert!(!e.starts_with('.'), "extension {e} must not start with a dot");
                assert!(exts.insert(*e), "duplicate extension {e}");
            }
        }
    }

    #[test]
    fn image_outputs_match_prd() {
        let mut got: Vec<_> = image_outputs().iter().map(|f| f.0).collect();
        got.sort();
        assert_eq!(
            got,
            [
                "avif", "bmp", "exr", "gif", "heic", "ico", "jpeg", "png", "ppm", "qoi", "tga",
                "tiff", "webp"
            ]
        );
    }

    #[test]
    fn parse_accepts_ids_and_extensions() {
        assert_eq!(Format::parse("jpeg").unwrap().id.0, "jpeg");
        assert_eq!(Format::parse(".JPG").unwrap().id.0, "jpeg");
        assert_eq!(Format::parse("htm").unwrap().id.0, "html");
        assert!(Format::parse("xyz").is_none());
    }

    #[test]
    fn of_path_uses_the_extension() {
        assert_eq!(Format::of_path(Path::new(r"C:\a b\Photo.JPG")).unwrap().id.0, "jpeg");
        assert!(Format::of_path(Path::new("README")).is_none());
    }
}
