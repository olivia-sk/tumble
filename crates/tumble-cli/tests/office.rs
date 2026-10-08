//! Documents through `tumble`. Skips when LibreOffice is missing.

mod common;

use common::*;

fn skip() -> bool {
    if tumble_engines::office::soffice().is_some() {
        return false;
    }
    eprintln!("skipped: LibreOffice not found");
    true
}

#[test]
fn document_to_numbered_pages() {
    if skip() || !vendor_dll(PDFIUM) {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let html = dir.path().join("-notes ✓.html");
    std::fs::write(
        &html,
        "<html><body><p>one</p><p style=\"page-break-before: always\">two</p></body></html>",
    )
    .unwrap();
    let out = tumble(["--to".as_ref(), "png".as_ref(), "--".as_ref(), html.as_os_str()]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(names(dir.path()), ["-notes ✓-p001.png", "-notes ✓-p002.png", "-notes ✓.html"]);

    let json = tumble([html.as_os_str(), "--to".as_ref(), "docx".as_ref(), "--json".as_ref()]);
    assert!(json.status.success(), "{}", stderr(&json));
    assert!(
        stdout(&json).contains("\"route\":\"html -> docx (libreoffice)\""),
        "{}",
        stdout(&json)
    );
}
