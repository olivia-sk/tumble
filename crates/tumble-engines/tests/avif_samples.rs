//! AVIF decoding against real-world files from the AOMediaCodec conformance
//! set (github.com/AOMediaCodec/av1-avif, testFiles/Link-U and Microsoft).
//! The files are not committed; set TUMBLE_AVIF_SAMPLES to a folder holding
//! them. Without it the test skips.
//!
//! A file with a reference PNG (Link-U names it by dropping the encoding
//! tokens: `fox.profile0.8bpc.yuv420.odd-width.avif` -> `fox.odd-width.png`;
//! rotate and mirror samples undo a transform baked into the pixels, so they
//! must display as the untransformed `kimono.png`)
//! must match its size exactly and score at least 30 dB PSNR, which catches
//! wrong matrices, ranges, bit depths, crops, rotations and mirrors. Other
//! files only need to decode, or fail with a clear error.

mod common;

use common::convert;
use image::{GenericImageView, RgbImage};
use std::path::Path;

fn psnr(a: &RgbImage, b: &RgbImage) -> f64 {
    let mse: f64 = a
        .as_raw()
        .iter()
        .zip(b.as_raw())
        .map(|(&x, &y)| (f64::from(x) - f64::from(y)).powi(2))
        .sum::<f64>()
        / a.as_raw().len() as f64;
    10.0 * (255.0f64 * 255.0 / mse.max(1e-9)).log10()
}

fn luma(img: &RgbImage) -> RgbImage {
    image::DynamicImage::ImageRgb8(img.clone()).grayscale().to_rgb8()
}

fn reference_name(avif: &str) -> String {
    let stem = avif.trim_end_matches(".avif");
    let is_encoding = |t: &str| {
        t.starts_with("profile")
            || t.ends_with("bpc")
            || t.starts_with("yuv")
            || t.starts_with("rotate")
            || t.starts_with("mirror-")
            || matches!(t, "monochrome" | "no-cdef" | "no-restoration")
    };
    let kept: Vec<&str> = stem.split('.').filter(|t| !is_encoding(t)).collect();
    format!("{}.png", kept.join("."))
}

#[test]
fn reference_names() {
    assert_eq!(reference_name("fox.profile0.8bpc.yuv420.odd-width.avif"), "fox.odd-width.png");
    assert_eq!(reference_name("fox.profile0.8bpc.yuv420.monochrome.avif"), "fox.png");
    assert_eq!(reference_name("kimono.rotate90.avif"), "kimono.png");
    assert_eq!(reference_name("kimono.mirror-vertical.rotate270.avif"), "kimono.png");
    assert_eq!(reference_name("kimono.crop.avif"), "kimono.crop.png");
}

#[test]
fn real_world_avif_files() {
    let Ok(dir) = std::env::var("TUMBLE_AVIF_SAMPLES") else {
        eprintln!("skipped: set TUMBLE_AVIF_SAMPLES to a folder of AVIF conformance files");
        return;
    };
    let dir = Path::new(&dir);
    let out = tempfile::tempdir().unwrap();
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "avif"))
        .collect();
    files.sort();

    let mut problems = Vec::new();
    for file in files {
        let name = file.file_name().unwrap().to_string_lossy().into_owned();
        let png = out.path().join(format!("{name}.png"));
        if let Err(e) = convert("avif", "png", &file, &png) {
            println!("{name:<55} ERROR {e}");
            continue;
        }
        let img = image::open(&png).unwrap();
        let (w, h) = img.dimensions();
        let mut line = format!("{name:<55} {w}x{h} {:?}", img.color());
        if let Ok(reference) = image::open(dir.join(reference_name(&name))) {
            let want = reference.to_rgb8();
            if want.dimensions() != (w, h) {
                problems.push(format!("{name}: {w}x{h}, reference is {:?}", want.dimensions()));
            } else {
                let got = img.to_rgb8();
                let score = if name.contains("monochrome") {
                    psnr(&luma(&got), &luma(&want))
                } else {
                    psnr(&got, &want)
                };
                line.push_str(&format!("  PSNR {score:.1} dB"));
                if score < 30.0 {
                    problems.push(format!("{name}: PSNR {score:.1} dB"));
                }
            }
        }
        println!("{line}");
    }
    assert!(problems.is_empty(), "poor decodes:\n{}", problems.join("\n"));
}
