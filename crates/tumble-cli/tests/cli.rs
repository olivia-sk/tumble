//! Non-converting commands: formats, targets, engines, argument errors.

mod common;

use common::*;
use std::time::Instant;

#[test]
fn formats_prints_every_section() {
    let out = tumble(["formats"]);
    assert!(out.status.success());
    let text = stdout(&out);
    for needle in [
        "Images",
        "Video",
        "Audio",
        "Documents",
        ".jpg .jpeg .jfif",
        "all other image outputs, plus AVI, MKV, MOV, MP4, WebM",
        "all other video outputs, all image outputs, all audio outputs",
        "all image outputs (SVG is input only)",
        "Image outputs: AVIF, BMP, GIF, HEIC, ICO, JPEG, OpenEXR, PNG, PPM, QOI, TGA, TIFF, WebP",
    ] {
        assert!(text.contains(needle), "missing {needle:?} in:\n{text}");
    }
}

#[test]
fn formats_json_lists_all_formats() {
    let out = tumble(["formats", "--json"]);
    assert!(out.status.success());
    let rows: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), tumble_core::FORMATS.len());
    assert_eq!(rows[0]["id"], "jpeg");
}

#[test]
fn targets_lists_reachable_formats() {
    let out = tumble(["targets", "holiday.JPG"]);
    assert!(out.status.success());
    let ids: Vec<String> =
        stdout(&out).lines().map(|l| l.split_whitespace().next().unwrap().to_string()).collect();
    let mut want = vec![
        "png", "webp", "heic", "avif", "gif", "tiff", "bmp", "ico", "tga", "ppm", "qoi", "exr",
    ];
    if !vendor_dll("libx265.dll") {
        want.retain(|f| *f != "heic");
    }
    assert_eq!(ids, want);

    let menu = tumble(["targets", "holiday.jpg", "--menu", "--json"]);
    let rows: serde_json::Value = serde_json::from_slice(&menu.stdout).unwrap();
    let ids: Vec<&str> =
        rows.as_array().unwrap().iter().map(|r| r["id"].as_str().unwrap()).collect();
    let mut want = vec!["png", "webp", "avif", "heic", "gif", "tiff", "ico"];
    if !vendor_dll("libx265.dll") {
        want.retain(|f| *f != "heic");
    }
    assert_eq!(ids, want);
}

#[test]
fn targets_for_unsupported_files_exit_3() {
    assert_eq!(tumble(["targets", "notes.xyz"]).status.code(), Some(3));
}

#[test]
fn targets_is_fast() {
    tumble(["targets", "warmup.png"]);
    let start = Instant::now();
    let out = tumble(["targets", "photo.png", "--menu"]);
    let took = start.elapsed();
    assert!(out.status.success());
    // The 50 ms budget is for release builds; debug builds get slack.
    let limit = if cfg!(debug_assertions) { 500 } else { 50 };
    assert!(took.as_millis() < limit, "targets took {took:?}");
}

#[test]
fn engines_lists_the_image_engine() {
    let out = tumble(["engines"]);
    assert!(out.status.success());
    let text = stdout(&out);
    assert!(text.contains("image [ok] built in"), "{text}");
    for (engine, dll) in [("libheif", "heif.dll"), ("pdfium", "pdfium.dll")] {
        let state = if vendor_dll(dll) { "[ok]" } else { "[missing]" };
        assert!(text.contains(&format!("{engine} {state}")), "{text}");
    }
    let json = tumble(["engines", "--json"]);
    let v: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(v["engines"][0]["name"], "image");
    assert!(v["engines"][0]["reads"].as_array().unwrap().iter().any(|f| f == "svg"));
}

#[test]
fn bad_arguments_exit_2() {
    assert_eq!(tumble(["--bogus"]).status.code(), Some(2));
    assert_eq!(tumble(["photo.png"]).status.code(), Some(2), "missing --to");
    assert_eq!(tumble(["menu", "bogus"]).status.code(), Some(2), "unknown menu action");
}
