//! LibreOffice engine (PRD phase 4 acceptance): DOCX, XLSX and PPTX each
//! convert to PDF and PNG, and every document pair in section 7 converts.
//! Skips when LibreOffice is missing. Fixtures are flat ODF XML written
//! here and turned into the other formats by LibreOffice itself.

mod common;

use common::*;
use std::path::{Path, PathBuf};
use tumble_core::{CancelToken, ConvertOptions, Engine, NoProgress, Step};
use tumble_engines::office::{OfficeEngine, filters};

const MARK: &str = "Tumble héllo ✓";

fn skip() -> bool {
    if tumble_engines::office::soffice().is_some() {
        return false;
    }
    eprintln!("skipped: LibreOffice not found (winget install TheDocumentFoundation.LibreOffice)");
    true
}

const NS: &str = r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" xmlns:presentation="urn:oasis:names:tc:opendocument:xmlns:presentation:1.0" office:version="1.3""#;

/// Two pages of text.
fn fodt() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document {NS} office:mimetype="application/vnd.oasis.opendocument.text">
 <office:automatic-styles>
  <style:style style:name="Break" style:family="paragraph"><style:paragraph-properties fo:break-before="page"/></style:style>
 </office:automatic-styles>
 <office:body><office:text>
  <text:p>{MARK} page one</text:p>
  <text:p text:style-name="Break">Page two</text:p>
 </office:text></office:body>
</office:document>"#
    )
}

/// One sheet: a header and three rows.
fn fods() -> String {
    let row = |a: &str, b: u32| {
        format!(
            r#"<table:table-row><table:table-cell office:value-type="string"><text:p>{a}</text:p></table:table-cell><table:table-cell office:value-type="float" office:value="{b}"><text:p>{b}</text:p></table:table-cell></table:table-row>"#
        )
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document {NS} office:mimetype="application/vnd.oasis.opendocument.spreadsheet">
 <office:body><office:spreadsheet><table:table table:name="Fruit">
  <table:table-row><table:table-cell office:value-type="string"><text:p>{MARK}</text:p></table:table-cell><table:table-cell office:value-type="string"><text:p>Count</text:p></table:table-cell></table:table-row>
  {}{}{}
 </table:table></office:spreadsheet></office:body>
</office:document>"#,
        row("Apple", 3),
        row("Pear", 5),
        row("Plum", 8)
    )
}

/// Three slides, each with a text box.
fn fodp() -> String {
    let slide = |n: u32| {
        format!(
            r#"<draw:page draw:name="Slide{n}"><draw:frame svg:x="2cm" svg:y="2cm" svg:width="20cm" svg:height="3cm"><draw:text-box><text:p>{MARK} slide {n}</text:p></draw:text-box></draw:frame></draw:page>"#
        )
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document {NS} office:mimetype="application/vnd.oasis.opendocument.presentation">
 <office:body><office:presentation>{}{}{}</office:presentation></office:body>
</office:document>"#,
        slide(1),
        slide(2),
        slide(3)
    )
}

fn office(from: &'static str, to: &'static str, input: &Path, output: &Path) -> Result<(), String> {
    OfficeEngine
        .convert(
            Step::new(from, to),
            input,
            output,
            &ConvertOptions::default(),
            &NoProgress,
            &CancelToken::new(),
        )
        .map(|_| ())
        .map_err(|e| format!("{from} -> {to}: {e}"))
}

/// Writes one fixture per document format into `dir`.
fn fixtures(dir: &Path) -> Vec<(&'static str, PathBuf)> {
    // Flat ODF opens as odt/ods/odp; LibreOffice makes the rest.
    let flat = [("odt", "fodt", fodt()), ("ods", "fods", fods()), ("odp", "fodp", fodp())];
    let mut out = Vec::new();
    for (fmt, ext, xml) in flat {
        let flat_path = dir.join(format!("flat.{ext}"));
        std::fs::write(&flat_path, xml).unwrap();
        let path = dir.join(format!("src.{fmt}"));
        // The engine copies its input to in.<odt|ods|odp>; flat XML under that
        // name still opens, since LibreOffice detects by content.
        office(fmt, "pdf", &flat_path, &dir.join(format!("probe-{fmt}.pdf"))).unwrap();
        std::fs::copy(&flat_path, &path).unwrap();
        out.push((fmt, path));
    }
    let derive = [
        ("odt", ["docx", "doc", "rtf"].as_slice()),
        ("odp", &["pptx", "ppt"]),
        ("ods", &["xlsx", "xls", "csv"]),
    ];
    for (base, targets) in derive {
        let src = out.iter().find(|(f, _)| *f == base).unwrap().1.clone();
        for &t in targets {
            let path = dir.join(format!("src.{t}"));
            office(base, t, &src, &path).unwrap();
            out.push((t, path));
        }
    }
    // Replace the flat-XML stand-ins with real (zipped) ODF files.
    for (odf, from) in [("odt", "docx"), ("ods", "xlsx"), ("odp", "pptx")] {
        let src = out.iter().find(|(f, _)| *f == from).unwrap().1.clone();
        let path = dir.join(format!("real.{odf}"));
        office(from, odf, &src, &path).unwrap();
        assert_document(odf, &path);
        out.iter_mut().find(|(f, _)| *f == odf).unwrap().1 = path;
    }
    let txt = dir.join("src.txt");
    std::fs::write(&txt, format!("{MARK} plain text\n")).unwrap();
    out.push(("txt", txt));
    let html = dir.join("src.html");
    std::fs::write(
        &html,
        format!("<!doctype html><html><body><h1>{MARK}</h1><p>html</p></body></html>"),
    )
    .unwrap();
    out.push(("html", html));
    let md = dir.join("src.md");
    std::fs::write(&md, format!("# {MARK}\n\nSome **bold** text.\n")).unwrap();
    out.push(("md", md));
    out
}

/// Checks the file is the format it claims to be.
fn assert_document(fmt: &str, path: &Path) {
    let b = std::fs::read(path).unwrap();
    let text = String::from_utf8_lossy(&b);
    let zip_has = |needle: &str| b.starts_with(b"PK\x03\x04") && text.contains(needle);
    let ok = match fmt {
        "pdf" => b.starts_with(b"%PDF"),
        "docx" => zip_has("word/"),
        "xlsx" => zip_has("xl/"),
        "pptx" => zip_has("ppt/"),
        "odt" => zip_has("opendocument.text"),
        "ods" => zip_has("opendocument.spreadsheet"),
        "odp" => zip_has("opendocument.presentation"),
        "doc" | "xls" | "ppt" => b.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]),
        "rtf" => b.starts_with(b"{\\rtf"),
        "html" => text.to_ascii_lowercase().contains("<html") && text.contains(MARK),
        "txt" | "csv" | "md" => text.contains(MARK),
        other => panic!("no check for {other}"),
    };
    assert!(ok, "{} does not look like {fmt}", path.display());
}

#[test]
fn every_document_pair() {
    if skip() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let sources = fixtures(dir.path());
    let jobs: Vec<(&'static str, &'static str, PathBuf)> = filters::pairs()
        .into_iter()
        .map(|(from, to)| {
            let src = sources
                .iter()
                .find(|(f, _)| *f == from)
                .unwrap_or_else(|| panic!("no fixture for {from}"));
            (from, to, src.1.clone())
        })
        .collect();
    // Four LibreOffice instances at a time, each with its own profile.
    let failures = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for chunk in jobs.chunks(jobs.len().div_ceil(4)) {
            let (dir, failures) = (dir.path(), &failures);
            s.spawn(move || {
                for (from, to, src) in chunk {
                    let out = dir.join(format!("{from}-to.{to}"));
                    match office(from, to, src, &out) {
                        Ok(()) => assert_document(to, &out),
                        Err(e) => failures.lock().unwrap().push(e),
                    }
                }
            });
        }
    });
    let failures = failures.into_inner().unwrap();
    assert!(
        failures.is_empty(),
        "{} of {} failed:\n{}",
        failures.len(),
        jobs.len(),
        failures.join("\n")
    );
    println!("{} document conversions verified", jobs.len());
}

#[test]
fn docx_xlsx_pptx_to_pdf_and_png() {
    if skip() || !engine_available("pdfium") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let sources = fixtures(dir.path());
    let registry = tumble_engines::default_registry();
    for (fmt, pages) in [("docx", Some(2)), ("pptx", Some(3)), ("xlsx", None)] {
        let src = &sources.iter().find(|(f, _)| *f == fmt).unwrap().1;
        let pdf = dir.path().join(format!("{fmt}.pdf"));
        office(fmt, "pdf", src, &pdf).unwrap();
        assert_document("pdf", &pdf);

        // PNG goes document -> PDF -> pages: two routed steps.
        let route =
            registry.route(tumble_core::FormatId(fmt), tumble_core::FormatId("png")).unwrap();
        assert_eq!(
            route.describe(&registry),
            format!("{fmt} -> pdf (libreoffice) -> png (pdfium)")
        );
        let pngs = convert_all(
            "pdf",
            "png",
            &pdf,
            &dir.path().join(format!("{fmt}.png")),
            &ConvertOptions::default(),
        )
        .unwrap();
        if let Some(n) = pages {
            assert_eq!(pngs.len(), n, "{fmt} pages");
        }
        for png in &pngs {
            assert_magic("png", png);
        }
    }
}

#[test]
fn text_and_csv_keep_unicode() {
    if skip() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let txt = dir.path().join("u.txt");
    std::fs::write(&txt, format!("{MARK}\n")).unwrap();
    let docx = dir.path().join("u.docx");
    office("txt", "docx", &txt, &docx).unwrap();
    let back = dir.path().join("u-back.txt");
    office("docx", "txt", &docx, &back).unwrap();
    assert!(std::fs::read_to_string(&back).unwrap().contains(MARK));

    let csv = dir.path().join("u.csv");
    std::fs::write(&csv, format!("Name,Count\n{MARK},1\n\"a, b\",2\n")).unwrap();
    let xlsx = dir.path().join("u.xlsx");
    office("csv", "xlsx", &csv, &xlsx).unwrap();
    let back = dir.path().join("u-back.csv");
    office("xlsx", "csv", &xlsx, &back).unwrap();
    let text = std::fs::read_to_string(&back).unwrap();
    assert!(text.contains(MARK) && text.contains("\"a, b\",2"), "{text}");
}

#[test]
fn garbage_documents_fail_cleanly() {
    if skip() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad.docx");
    std::fs::write(&bad, b"not a word document").unwrap();
    // LibreOffice may read garbage as plain text; either it converts or it
    // reports a clear error, but it never hangs or leaves a stray file.
    match office("docx", "pdf", &bad, &dir.path().join("bad.pdf")) {
        Ok(()) => assert_document("pdf", &dir.path().join("bad.pdf")),
        Err(e) => assert!(e.contains("LibreOffice"), "{e}"),
    }
}

#[test]
fn markdown_keeps_its_formatting() {
    if skip() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("notes.md");
    std::fs::write(
        &md,
        format!(
            "# {MARK}\n\nSome **bold** text and a [link](https://example.com).\n\n- one\n- two\n\n| a | b |\n|---|---|\n| 1 | 2 |\n"
        ),
    )
    .unwrap();

    // Markdown is read as Markdown, not as plain text.
    let html = dir.path().join("notes.html");
    office("md", "html", &md, &html).unwrap();
    let page = std::fs::read_to_string(&html).unwrap().to_lowercase();
    assert!(page.contains("<h1") && page.contains("<table") && page.contains("<li"), "{page}");
    assert!(!page.contains("**bold**"), "the asterisks were rendered, not kept as text");

    // And written back as Markdown.
    let docx = dir.path().join("notes.docx");
    office("md", "docx", &md, &docx).unwrap();
    let back = dir.path().join("back.md");
    office("docx", "md", &docx, &back).unwrap();
    let text = std::fs::read_to_string(&back).unwrap();
    assert!(text.contains(&format!("# {MARK}")), "heading survives:\n{text}");
    assert!(text.contains("**bold**"), "bold survives:\n{text}");
    // LibreOffice may normalise the URL (a trailing slash).
    assert!(text.contains("](https://example.com"), "link survives:\n{text}");
}
