//! PDF pages through PDFium (PRD phase 2 acceptance: a 3 page PDF gives 3
//! numbered PNGs). Skips when pdfium.dll is missing.

mod common;

use common::*;
use tumble_core::{ConvertOptions, Resize};

fn skip() -> bool {
    if engine_available("pdfium") {
        return false;
    }
    eprintln!("skipped: pdfium.dll not found (run scripts/fetch-vendor.ps1)");
    true
}

const RED: [u8; 3] = [220, 30, 30];
const GREEN: [u8; 3] = [30, 180, 60];
const BLUE: [u8; 3] = [30, 60, 200];

fn three_pages() -> Vec<u8> {
    make_pdf(&[
        PdfPage { width: 612, height: 792, rgb: RED, rotate: 0 }, // US Letter
        PdfPage { width: 595, height: 842, rgb: GREEN, rotate: 0 }, // A4
        PdfPage { width: 612, height: 792, rgb: BLUE, rotate: 90 }, // landscape
    ])
}

#[test]
fn three_pages_give_three_numbered_images() {
    if skip() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let pdf = dir.path().join("report.pdf");
    std::fs::write(&pdf, three_pages()).unwrap();
    let out = dir.path().join("report.png");
    let written = convert_all("pdf", "png", &pdf, &out, &ConvertOptions::default()).unwrap();

    let names: Vec<String> =
        written.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["report-p001.png", "report-p002.png", "report-p003.png"]);
    // 150 DPI: points * 150 / 72. The third page is rotated 90 degrees.
    let expected = [((1275, 1650), RED), ((1240, 1754), GREEN), ((1650, 1275), BLUE)];
    for (path, ((w, h), colour)) in written.iter().zip(expected) {
        assert_magic("png", path);
        let img = image::open(path).unwrap().to_rgb8();
        assert_eq!(img.dimensions(), (w, h), "{}", path.display());
        let p = img.get_pixel(w / 2, h / 2).0;
        assert_close(&path.display().to_string(), p, colour, 2);
    }
}

#[test]
fn single_page_keeps_the_plain_name() {
    if skip() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let pdf = dir.path().join("one.pdf");
    std::fs::write(&pdf, make_pdf(&[PdfPage { width: 300, height: 200, rgb: RED, rotate: 0 }]))
        .unwrap();
    let out = dir.path().join("one.jpg");
    let written = convert_all("pdf", "jpeg", &pdf, &out, &ConvertOptions::default()).unwrap();
    assert_eq!(written, std::slice::from_ref(&out));
    assert_magic("jpeg", &out);
}

#[test]
fn every_image_output() {
    if skip() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let pdf = dir.path().join("p.pdf");
    std::fs::write(&pdf, make_pdf(&[PdfPage { width: 72, height: 36, rgb: GREEN, rotate: 0 }]))
        .unwrap();
    let registry = tumble_engines::default_registry();
    for to in registry.targets(tumble_core::FormatId("pdf")) {
        let to = to.as_str();
        let out = dir.path().join(format!("p.{to}"));
        convert("pdf", to, &pdf, &out).unwrap();
        assert_magic(to, &out);
        let img = read_back(to, &out);
        assert_eq!(img.dimensions(), (150, 75), "pdf -> {to}");
        let p = img.get_pixel(75, 37).0;
        assert_close(&format!("pdf -> {to}"), [p[0], p[1], p[2]], GREEN, tolerance(to).max(24));
    }
}

#[test]
fn resize_renders_pages_at_the_box_size() {
    if skip() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let pdf = dir.path().join("big.pdf");
    std::fs::write(&pdf, three_pages()).unwrap();
    let options = ConvertOptions {
        resize: Some(Resize { max_width: 3000, max_height: 3000 }),
        ..Default::default()
    };
    let written = convert_all("pdf", "bmp", &pdf, &dir.path().join("big.bmp"), &options).unwrap();
    let first = image::open(&written[0]).unwrap();
    assert_eq!((first.width(), first.height()), (2318, 3000), "vector pages scale up");
}

#[test]
fn bad_pdfs_fail_with_a_clear_message() {
    if skip() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad.pdf");
    std::fs::write(&bad, b"%PDF-1.4 this is not really a pdf").unwrap();
    let err = convert("pdf", "png", &bad, &dir.path().join("bad.png")).unwrap_err();
    assert!(err.contains("not a valid PDF"), "{err}");
}
