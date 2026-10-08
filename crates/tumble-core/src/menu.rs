//! Right-click short lists: at most `MENU_MAX` targets per input group,
//! most useful first. `tumble targets --menu` drops the input's own format
//! and anything the registry cannot reach.

use crate::format::{Family, Format, FormatId, Kind};
use crate::registry::Registry;

pub const MENU_MAX: usize = 8;

const STILL_IMAGE: &[&str] = &["jpeg", "png", "webp", "avif", "heic", "gif", "tiff", "ico"];
const ANIMATED_IMAGE: &[&str] = &["mp4", "webm", "png", "jpeg", "webp", "avif", "mov", "mkv"];
const VIDEO: &[&str] = &["mp4", "webm", "gif", "mp3", "mov", "mkv", "png", "jpeg"];
const AUDIO: &[&str] = &["mp3", "wav", "flac", "m4a", "ogg", "opus", "aac"];
const PDF: &[&str] = &["png", "jpeg", "webp", "tiff"];
const TEXT_DOCUMENT: &[&str] = &["pdf", "docx", "md", "txt", "html", "odt", "png", "jpeg"];
const PRESENTATION: &[&str] = &["pdf", "pptx", "odp", "png", "jpeg"];
const SPREADSHEET: &[&str] = &["pdf", "xlsx", "csv", "ods", "png", "jpeg"];

/// Name of the shared submenu a format uses; also the registry key name
/// under `menus\` in Phase 5.
pub fn menu_group(format: &Format) -> &'static str {
    match (format.kind, format.family) {
        (Kind::AnimatedImage, _) => "animated-image",
        (_, Family::Image | Family::Vector) => "image",
        (_, Family::Video) => "video",
        (_, Family::Audio) => "audio",
        (_, Family::Pdf) => "pdf",
        (_, Family::WordProcessing | Family::PlainText | Family::Html | Family::Markdown) => {
            "text-document"
        }
        (_, Family::Presentation) => "presentation",
        (_, Family::Spreadsheet) => "spreadsheet",
    }
}

/// Candidate menu targets for a format, before filtering.
pub fn menu_candidates(format: &Format) -> Vec<FormatId> {
    let list = match menu_group(format) {
        "animated-image" => ANIMATED_IMAGE,
        "image" => STILL_IMAGE,
        "video" => VIDEO,
        "audio" => AUDIO,
        "pdf" => PDF,
        "text-document" => TEXT_DOCUMENT,
        "presentation" => PRESENTATION,
        _ => SPREADSHEET,
    };
    list.iter().map(|&s| FormatId(s)).collect()
}

/// What the right-click menu shows for `format` on this machine: the
/// candidates the registry can reach, minus the format itself.
pub fn menu_targets(registry: &Registry, format: &Format) -> Vec<FormatId> {
    let reachable = registry.targets(format.id);
    menu_candidates(format)
        .into_iter()
        .filter(|t| *t != format.id && reachable.contains(t))
        .collect()
}

/// The menu targets every one of `formats` offers, in the first one's
/// order. What a menu shows for several selected files.
pub fn common_menu_targets(registry: &Registry, formats: &[&Format]) -> Vec<FormatId> {
    let Some((first, rest)) = formats.split_first() else { return Vec::new() };
    let others: Vec<Vec<FormatId>> = rest.iter().map(|f| menu_targets(registry, f)).collect();
    menu_targets(registry, first)
        .into_iter()
        .filter(|t| others.iter().all(|o| o.contains(t)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::FORMATS;

    #[test]
    fn lists_are_short_known_and_declared() {
        for f in FORMATS {
            let list = menu_candidates(f);
            assert!(list.len() <= MENU_MAX, "{} menu too long", f.id);
            let declared = f.declared_targets();
            for t in list.iter().filter(|t| **t != f.id) {
                assert!(Format::by_id(t.0).is_some(), "unknown id {t}");
                assert!(declared.contains(t), "{} menu offers undeclared {}", f.id, t);
            }
        }
    }
}
