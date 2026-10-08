//! Which LibreOffice filters to use (PRD section 7, documents). Pure, so it
//! is unit-tested without LibreOffice.

/// Word-processing family: each converts to the others, PDF, HTML and text.
pub const WORD: &[&str] = &["docx", "doc", "odt", "rtf"];
pub const SLIDES: &[&str] = &["pptx", "ppt", "odp"];
pub const SHEETS: &[&str] = &["xlsx", "xls", "ods", "csv"];

/// Every (from, to) pair LibreOffice handles for Tumble. Images are not
/// listed: documents reach them through PDF and PDFium.
pub fn pairs() -> Vec<(&'static str, &'static str)> {
    let mut v = Vec::new();
    let mut within = |family: &[&'static str], extra: &[&'static str]| {
        for &from in family {
            for &to in family.iter().chain(extra) {
                if from != to {
                    v.push((from, to));
                }
            }
        }
    };
    within(WORD, &["pdf", "html", "txt", "md"]);
    within(SLIDES, &["pdf"]);
    within(SHEETS, &["pdf"]);
    for to in ["pdf", "doc", "docx", "html", "odt", "rtf", "md"] {
        v.push(("txt", to));
    }
    for to in ["pdf", "doc", "docx", "odt", "txt", "rtf", "md"] {
        v.push(("html", to));
    }
    for to in ["pdf", "doc", "docx", "html", "odt", "rtf", "txt"] {
        v.push(("md", to));
    }
    v
}

/// `--convert-to` value: extension, then the export filter and options.
pub fn export(to: &str) -> &'static str {
    match to {
        "pdf" => "pdf",
        "docx" => "docx:MS Word 2007 XML",
        "doc" => "doc:MS Word 97",
        "odt" => "odt:writer8",
        "rtf" => "rtf:Rich Text Format",
        // Images are embedded so the page is one self-contained file.
        "html" => r#"html:HTML (StarWriter):{"EmbedImages":{"type":"boolean","value":"true"}}"#,
        "txt" => "txt:Text (encoded):UTF8",
        "md" => "md:Markdown",
        "pptx" => "pptx:Impress MS PowerPoint 2007 XML",
        "ppt" => "ppt:MS PowerPoint 97",
        "odp" => "odp:impress8",
        "xlsx" => "xlsx:Calc MS Excel 2007 XML",
        "xls" => "xls:MS Excel 97",
        "ods" => "ods:calc8",
        // Comma-separated, double quotes, UTF-8 (76), first line kept.
        "csv" => "csv:Text - txt - csv (StarCalc):44,34,76,1",
        other => unreachable!("no LibreOffice export for {other}"),
    }
}

/// `--infilter` for inputs LibreOffice would otherwise guess at: text and
/// CSV as UTF-8, HTML as a Writer document (not Writer/Web, which cannot
/// save as Word formats), and Markdown as Markdown (LibreOffice 25.8+), not
/// as plain text.
pub fn import(from: &str) -> Option<&'static str> {
    match from {
        "txt" => Some("Text (encoded):UTF8"),
        "csv" => Some("Text - txt - csv (StarCalc):44,34,76,1"),
        "html" => Some("HTML (StarWriter)"),
        "md" => Some("Markdown"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tumble_core::{Format, FormatId};

    #[test]
    fn pairs_match_section_7() {
        let pairs = pairs();
        for &(from, to) in &pairs {
            let declared = Format::by_id(from).unwrap().declared_targets();
            assert!(declared.contains(&FormatId(to)), "{from} -> {to} is not in section 7");
            let _ = export(to);
        }
        // Every non-image target in section 7 for these inputs is covered.
        for from in WORD.iter().chain(SLIDES).chain(SHEETS).chain(&["txt", "html", "md"]) {
            for t in Format::by_id(from).unwrap().declared_targets() {
                if t.format().family != tumble_core::format::Family::Image {
                    assert!(pairs.contains(&(from, t.as_str())), "missing {from} -> {t}");
                }
            }
        }
    }

    #[test]
    fn text_inputs_are_read_as_utf8() {
        assert!(import("txt").unwrap().contains("UTF8"));
        assert!(import("csv").unwrap().contains(",76,"));
        assert_eq!(import("docx"), None);
    }
}
