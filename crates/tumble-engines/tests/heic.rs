//! HEIC through libheif (PRD phase 2 acceptance: HEIC <-> JPEG/PNG round
//! trips). Skips when the vendor DLLs are missing.

mod common;

use common::*;
use tumble_core::ConvertOptions;
use tumble_engines::raster::{READS, WRITES};

fn skip() -> bool {
    if engine_available("libheif") {
        return false;
    }
    eprintln!("skipped: libheif DLLs not found (run scripts/fetch-vendor.ps1)");
    true
}

#[test]
fn every_image_format_to_heic_and_back() {
    if skip() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let heic = dir.path().join("card.heic");
    let png = fixture(dir.path(), "png");
    convert("png", "heic", &png, &heic).unwrap();
    assert_magic("heic", &heic);

    let mut failures = Vec::new();
    for &from in READS {
        let src = fixture(dir.path(), from);
        let out = dir.path().join(format!("from-{from}.heic"));
        if let Err(e) = convert(from, "heic", &src, &out) {
            failures.push(e);
        }
    }
    for &to in WRITES {
        let out = dir.path().join(format!("from-heic.{to}"));
        match convert("heic", to, &heic, &out) {
            Ok(()) => {
                assert_magic(to, &out);
                let img = read_back(to, &out);
                assert_eq!(img.dimensions(), (W, H), "heic -> {to} size");
                let tol = tolerance(to).max(24);
                for ((x, y), want) in PROBES {
                    let p = img.get_pixel(x, y).0;
                    assert_close(
                        &format!("heic -> {to} at ({x},{y})"),
                        [p[0], p[1], p[2]],
                        want,
                        tol,
                    );
                }
            }
            Err(e) => failures.push(e),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn jpeg_and_png_round_trips_keep_size_colour_and_alpha() {
    if skip() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    for start in ["png", "jpeg"] {
        let src = fixture(dir.path(), start);
        let heic = dir.path().join(format!("{start}.heic"));
        let back = dir.path().join(format!("{start}-back.{start}"));
        convert(start, "heic", &src, &heic).unwrap();
        convert("heic", start, &heic, &back).unwrap();
        let img = read_back(start, &back);
        assert_eq!(img.dimensions(), (W, H));
        for ((x, y), want) in PROBES {
            let p = img.get_pixel(x, y).0;
            assert_close(&format!("{start} round trip"), [p[0], p[1], p[2]], want, 24);
        }
    }
    // PNG has alpha, and HEIC keeps it.
    let png = dir.path().join("png-back.png");
    let a = image::open(&png).unwrap().to_rgba8().get_pixel(45, 5).0[3];
    assert!(a.abs_diff(128) <= 8, "alpha {a}, want 128");
}

#[test]
fn quality_changes_heic_size() {
    if skip() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let noisy = image::RgbImage::from_fn(256, 256, |x, y| {
        let v = (x * 7919 + y * 104_729) % 251;
        image::Rgb([v as u8, (v * 3 % 256) as u8, (v * 5 % 256) as u8])
    });
    let png = dir.path().join("noise.png");
    noisy.save(&png).unwrap();
    let size = |q: u8| {
        let out = dir.path().join(format!("q{q}.heic"));
        let options = ConvertOptions { quality: Some(q), ..Default::default() };
        convert_with("png", "heic", &png, &out, &options).unwrap();
        std::fs::metadata(&out).unwrap().len()
    };
    let (low, high) = (size(20), size(95));
    assert!(low < high, "q20 {low} bytes should be smaller than q95 {high} bytes");
}

#[test]
fn garbage_heic_fails_cleanly() {
    if skip() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad.heic");
    std::fs::write(&bad, b"not a heic file").unwrap();
    let err = convert("heic", "png", &bad, &dir.path().join("x.png")).unwrap_err();
    assert!(err.contains("cannot read HEIC"), "{err}");
}
