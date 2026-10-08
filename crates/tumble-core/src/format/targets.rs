//! What PRD section 7 says each format converts to. This is the specification
//! routing is tested against; it never decides what the registry offers.

use super::{FORMATS, Family as F, Format, FormatId, image_outputs};

impl Format {
    pub fn declared_targets(&self) -> Vec<FormatId> {
        let me = self.id;
        let others = |family: F| -> Vec<FormatId> {
            FORMATS
                .iter()
                .filter(|f| f.family == family && f.id != me && f.output)
                .map(|f| f.id)
                .collect()
        };
        let ids = |list: &[&'static str]| list.iter().map(|&s| FormatId(s)).collect::<Vec<_>>();

        let mut out: Vec<FormatId> = match self.family {
            F::Image => {
                let mut v = others(F::Image);
                if me.0 == "gif" {
                    v.extend(others(F::Video));
                }
                v
            }
            F::Vector => image_outputs(),
            F::Video => {
                let mut v = others(F::Video);
                v.extend(image_outputs());
                v.extend(others(F::Audio));
                v
            }
            F::Audio => others(F::Audio),
            F::Pdf => image_outputs(),
            F::WordProcessing => {
                let mut v = ids(&["pdf", "html", "txt", "md"]);
                v.extend(others(F::WordProcessing));
                v.extend(image_outputs());
                v
            }
            F::PlainText => {
                let mut v = ids(&["pdf", "doc", "docx", "html", "odt", "rtf", "md"]);
                v.extend(image_outputs());
                v
            }
            F::Html => {
                let mut v = ids(&["pdf", "doc", "docx", "odt", "txt", "rtf", "md"]);
                v.extend(image_outputs());
                v
            }
            F::Markdown => {
                let mut v = ids(&["pdf", "doc", "docx", "html", "odt", "rtf", "txt"]);
                v.extend(image_outputs());
                v
            }
            F::Presentation | F::Spreadsheet => {
                let mut v = ids(&["pdf"]);
                v.extend(others(self.family));
                v.extend(image_outputs());
                v
            }
        };
        out.sort();
        out.dedup();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_self_or_input_only() {
        for f in FORMATS {
            for t in f.declared_targets() {
                assert_ne!(t, f.id, "{} lists itself", f.id);
                assert!(t.format().output, "{} lists input-only {}", f.id, t);
            }
        }
    }

    #[test]
    fn spot_checks() {
        let has = |from: &str, to: &'static str| {
            Format::by_id(from).unwrap().declared_targets().contains(&FormatId(to))
        };
        assert!(has("gif", "mp4"));
        assert!(!has("png", "mp4"), "still images must not offer video");
        assert!(has("svg", "png"));
        assert!(has("mp4", "mp3"));
        assert!(has("mp4", "gif"));
        assert!(!has("mp3", "mp4"));
        assert!(has("docx", "doc"));
        assert!(has("csv", "xlsx"));
        assert!(!has("pdf", "docx"));
        assert!(!has("pptx", "docx"));
        assert!(has("md", "pdf") && has("md", "docx") && has("md", "png"));
        assert!(has("docx", "md") && has("html", "md") && has("txt", "md"));
        assert!(!has("xlsx", "md") && !has("pdf", "md"));
    }
}
