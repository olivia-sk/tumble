//! Every image input converts to every image output (PRD phase 1 acceptance).
//! Each result is checked for magic bytes, dimensions and colour.

mod common;

use common::*;
use tumble_core::{ConvertOptions, Resize};
use tumble_engines::raster::{READS, WRITES};

#[test]
fn every_image_pair_converts() {
    let dir = tempfile::tempdir().unwrap();
    let mut failures = Vec::new();
    let mut count = 0;
    for &from in READS {
        let src_dir = dir.path().join(format!("from-{from}"));
        std::fs::create_dir(&src_dir).unwrap();
        let input = fixture(&src_dir, from);
        for &to in WRITES {
            if from == to {
                continue;
            }
            count += 1;
            let output = src_dir.join(format!("out.{to}"));
            if let Err(e) = convert(from, to, &input, &output) {
                failures.push(e);
                continue;
            }
            assert_magic(to, &output);
            let img = read_back(to, &output);
            assert_eq!(img.dimensions(), (W, H), "{from} -> {to} size");
            // The slack is the larger of the two lossy steps.
            let tol = tolerance(from).max(tolerance(to));
            for ((x, y), want) in PROBES {
                let p = img.get_pixel(x, y).0;
                assert_close(
                    &format!("{from} -> {to} at ({x},{y})"),
                    [p[0], p[1], p[2]],
                    want,
                    tol,
                );
            }
        }
    }
    assert!(failures.is_empty(), "{} of {count} failed:\n{}", failures.len(), failures.join("\n"));
    println!("{count} conversions verified");
}

#[test]
fn transparency_survives_where_the_format_has_alpha() {
    let dir = tempfile::tempdir().unwrap();
    let png = fixture(dir.path(), "png");
    for to in ["webp", "avif", "tiff", "bmp", "ico", "tga", "qoi", "exr"] {
        let out = dir.path().join(format!("alpha.{to}"));
        convert("png", to, &png, &out).unwrap();
        let a = read_back(to, &out).get_pixel(45, 5).0[3];
        assert!(a.abs_diff(128) <= tolerance(to), "{to}: alpha {a}, want 128");
    }
    for to in ["jpeg", "ppm"] {
        let out = dir.path().join(format!("flat.{to}"));
        convert("png", to, &png, &out).unwrap();
        let p = read_back(to, &out).get_pixel(45, 5).0;
        assert_eq!(p[3], 255, "{to} has no alpha");
        // 50% blue over white.
        assert_close(to, [p[0], p[1], p[2]], [137, 187, 237], tolerance(to));
    }
}

#[test]
fn resize_fits_the_box_and_svg_renders_at_size() {
    let dir = tempfile::tempdir().unwrap();
    let options = ConvertOptions {
        resize: Some(Resize { max_width: 30, max_height: 30 }),
        ..Default::default()
    };
    let png = fixture(dir.path(), "png");
    let out = dir.path().join("small.jpg");
    convert_with("png", "jpeg", &png, &out, &options).unwrap();
    assert_eq!(read_back("jpeg", &out).dimensions(), (30, 18));

    let svg = fixture(dir.path(), "svg");
    let big = ConvertOptions {
        resize: Some(Resize { max_width: 610, max_height: 610 }),
        ..Default::default()
    };
    let out = dir.path().join("big.png");
    convert_with("svg", "png", &svg, &out, &big).unwrap();
    assert_eq!(read_back("png", &out).dimensions(), (610, 370), "SVG scales up");

    let out = dir.path().join("same.png");
    let tga = fixture(dir.path(), "tga");
    convert_with("tga", "png", &tga, &out, &big).unwrap();
    assert_eq!(read_back("png", &out).dimensions(), (W, H), "raster never upscales");
}

#[test]
fn quality_changes_lossy_output_size() {
    let dir = tempfile::tempdir().unwrap();
    // A noisy image so quality has something to throw away.
    let noisy = image::RgbImage::from_fn(256, 256, |x, y| {
        let v = (x * 7919 + y * 104_729) % 251;
        image::Rgb([v as u8, (v * 3 % 256) as u8, (v * 5 % 256) as u8])
    });
    let png = dir.path().join("noise.png");
    noisy.save(&png).unwrap();
    for to in ["jpeg", "webp", "avif"] {
        let size = |q: u8| {
            let out = dir.path().join(format!("q{q}.{to}"));
            let options = ConvertOptions { quality: Some(q), ..Default::default() };
            convert_with("png", to, &png, &out, &options).unwrap();
            std::fs::metadata(&out).unwrap().len()
        };
        let (low, high) = (size(20), size(95));
        assert!(low < high, "{to}: q20 {low} bytes should be smaller than q95 {high} bytes");
    }
}

#[test]
fn webp_quality_100_is_lossless() {
    let dir = tempfile::tempdir().unwrap();
    let png = fixture(dir.path(), "png");
    let out = dir.path().join("lossless.webp");
    let options = ConvertOptions { quality: Some(100), ..Default::default() };
    convert_with("png", "webp", &png, &out, &options).unwrap();
    assert_eq!(read_back("webp", &out), test_card());
}

#[test]
fn corrupt_input_fails_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    for from in READS {
        let ext = tumble_core::FormatId(from).format().primary_extension();
        let bad = dir.path().join(format!("bad.{ext}"));
        std::fs::write(&bad, b"this is not an image").unwrap();
        let to = if *from == "png" { "jpeg" } else { "png" };
        let out = dir.path().join(format!("bad-out-{from}.{to}"));
        assert!(convert(from, to, &bad, &out).is_err(), "{from} accepted garbage");
    }
}
