//! HEIC and PDF through `tumble.exe`, end to end. Skips when the vendor DLLs
//! are missing.

mod common;

use common::*;

/// A two-page PDF, each page one flat colour, built by hand.
fn two_page_pdf() -> Vec<u8> {
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 144 72] /Contents 4 0 R >>".to_string(),
        "<< /Length 25 >>\nstream\n1 0 0 rg 0 0 144 72 re f\nendstream".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 144 72] /Contents 6 0 R >>".to_string(),
        "<< /Length 25 >>\nstream\n0 0 1 rg 0 0 144 72 re f\nendstream".to_string(),
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{obj}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(b"xref\n0 7\n0000000000 65535 f \n");
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!("trailer\n<< /Size 7 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    out
}

#[test]
fn pdf_pages_land_numbered_in_the_output_folder() {
    if !vendor_dll("pdfium.dll") {
        eprintln!("skipped: pdfium.dll not next to tumble.exe");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let pdf = dir.path().join("my report.pdf");
    std::fs::write(&pdf, two_page_pdf()).unwrap();
    let out_dir = dir.path().join("pages");
    for _ in 0..2 {
        let out = tumble([
            pdf.as_os_str(),
            "--to".as_ref(),
            "png".as_ref(),
            "-o".as_ref(),
            out_dir.as_os_str(),
        ]);
        assert!(out.status.success(), "{}", stderr(&out));
    }
    assert_eq!(
        names(&out_dir),
        [
            "my report-p001 (1).png",
            "my report-p001.png",
            "my report-p002 (1).png",
            "my report-p002.png"
        ],
        "second run numbers each page; no staging folder left"
    );
    let first = image::open(out_dir.join("my report-p001.png")).unwrap().to_rgb8();
    assert_eq!(first.dimensions(), (300, 150));
    assert_eq!(first.get_pixel(150, 75).0, [255, 0, 0]);
    let second = image::open(out_dir.join("my report-p002.png")).unwrap().to_rgb8();
    assert_eq!(second.get_pixel(150, 75).0, [0, 0, 255]);
}

#[test]
fn heic_round_trip_through_the_cli() {
    if !vendor_dll("libx265.dll") {
        eprintln!("skipped: libheif with x265 not next to tumble.exe");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let png = image_file(&dir.path().join("photo.png"));
    let out = tumble([png.as_os_str(), "--to".as_ref(), "heic".as_ref()]);
    assert!(out.status.success(), "{}", stderr(&out));
    let heic = dir.path().join("photo.heic");
    let out = tumble([heic.as_os_str(), "--to".as_ref(), "jpg".as_ref()]);
    assert!(out.status.success(), "{}", stderr(&out));
    let img = image::open(dir.path().join("photo.jpg")).unwrap();
    assert_eq!((img.width(), img.height()), (20, 10));
}

#[test]
fn without_vendor_dlls_heic_and_pdf_disappear_cleanly() {
    // A copy of tumble.exe on its own, with no DLLs beside it.
    let dir = tempfile::tempdir().unwrap();
    let lone = dir.path().join("tumble.exe");
    std::fs::copy(env!("CARGO_BIN_EXE_tumble"), &lone).unwrap();
    let run = |args: &[&str]| std::process::Command::new(&lone).args(args).output().unwrap();

    assert_eq!(run(&["targets", "photo.heic"]).status.code(), Some(3));
    assert_eq!(run(&["targets", "report.pdf"]).status.code(), Some(3));
    let targets = String::from_utf8(run(&["targets", "photo.jpg"]).stdout).unwrap();
    assert!(!targets.contains("heic"), "{targets}");

    let engines = String::from_utf8(run(&["engines"]).stdout).unwrap();
    assert!(
        engines.contains("libheif [missing] heif.dll, libde265.dll, aom.dll not found"),
        "{engines}"
    );
    assert!(engines.contains("pdfium [missing] pdfium.dll not found"), "{engines}");
    assert!(engines.contains("image [ok]"), "{engines}");

    let png = image_file(&dir.path().join("a.png"));
    let out = std::process::Command::new(&lone)
        .args([png.as_os_str(), "--to".as_ref(), "heic".as_ref()])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3), "no route without libheif");
    assert!(String::from_utf8_lossy(&out.stderr).contains("cannot convert PNG to HEIC"));
}
