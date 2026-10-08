//! Shared helpers for engine integration tests.

#![allow(dead_code)] // each test file uses a different subset

use image::{Rgba, RgbaImage};
use std::path::{Path, PathBuf};
use tumble_core::{CancelToken, ConvertOptions, FormatId, NoProgress, Step};

pub const W: u32 = 61;
pub const H: u32 = 37;

/// Sample points and the colour expected there (in opaque regions).
pub const PROBES: [((u32, u32), [u8; 3]); 2] =
    [((10, 10), [200, 60, 20]), ((45, 30), [240, 240, 240])];

/// Odd-sized test card with three flat blocks, one of them half transparent.
/// Odd sizes exercise chroma subsampling edges.
pub fn test_card() -> RgbaImage {
    RgbaImage::from_fn(W, H, |x, y| {
        if x < 30 {
            Rgba([200, 60, 20, 255])
        } else if y < 18 {
            Rgba([20, 120, 220, 128])
        } else {
            Rgba([240, 240, 240, 255])
        }
    })
}

pub fn test_svg() -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}">
  <rect width="30" height="{H}" fill="#c83c14"/>
  <rect x="30" width="31" height="18" fill="#1478dc" fill-opacity="0.5"/>
  <rect x="30" y="18" width="31" height="19" fill="#f0f0f0"/>
</svg>"##
    )
}

pub fn convert(
    from: &'static str,
    to: &'static str,
    input: &Path,
    output: &Path,
) -> Result<(), String> {
    convert_with(from, to, input, output, &ConvertOptions::default())
}

pub fn convert_with(
    from: &'static str,
    to: &'static str,
    input: &Path,
    output: &Path,
    options: &ConvertOptions,
) -> Result<(), String> {
    convert_all(from, to, input, output, options).map(|_| ())
}

/// Runs the single engine step `from -> to` through whichever registered
/// engine provides it, returning every file written.
pub fn convert_all(
    from: &'static str,
    to: &'static str,
    input: &Path,
    output: &Path,
    options: &ConvertOptions,
) -> Result<Vec<PathBuf>, String> {
    let registry = tumble_engines::default_registry();
    let route = registry
        .route(FormatId(from), FormatId(to))
        .ok_or_else(|| format!("{from} -> {to}: no route"))?;
    assert_eq!(route.hops.len(), 1, "{from} -> {to} should be a direct step");
    registry
        .engine(route.hops[0].engine)
        .convert(Step::new(from, to), input, output, options, &NoProgress, &CancelToken::new())
        .map_err(|e| format!("{from} -> {to}: {e}"))
}

/// Writes the test card as `fmt` into `dir` and returns its path.
pub fn fixture(dir: &Path, fmt: &'static str) -> PathBuf {
    let ext = tumble_core::FormatId(fmt).format().primary_extension();
    let path = dir.join(format!("card.{ext}"));
    match fmt {
        "png" => test_card().save(&path).unwrap(),
        "svg" => std::fs::write(&path, test_svg()).unwrap(),
        _ => {
            let png = fixture(dir, "png");
            convert("png", fmt, &png, &path).unwrap();
        }
    }
    path
}

/// A PDF page for `make_pdf`: size in points, fill colour, /Rotate.
pub struct PdfPage {
    pub width: u32,
    pub height: u32,
    pub rgb: [u8; 3],
    pub rotate: u32,
}

/// Builds a minimal valid PDF by hand: each page is filled with one colour.
pub fn make_pdf(pages: &[PdfPage]) -> Vec<u8> {
    let mut objects: Vec<String> = Vec::new();
    let n = pages.len();
    // 1: catalog, 2: page tree, then a page and a content stream per page.
    objects.push("<< /Type /Catalog /Pages 2 0 R >>".into());
    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 3 + i * 2)).collect();
    objects.push(format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" ")));
    for (i, p) in pages.iter().enumerate() {
        let [r, g, b] = p.rgb.map(|c| f32::from(c) / 255.0);
        let content = format!("{r:.3} {g:.3} {b:.3} rg 0 0 {} {} re f", p.width, p.height);
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Rotate {} /Contents {} 0 R >>",
            p.width,
            p.height,
            p.rotate,
            4 + i * 2
        ));
        objects.push(format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()));
    }
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{obj}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

/// Whether the vendor DLLs an engine needs sit next to the test binary.
pub fn engine_available(name: &str) -> bool {
    tumble_engines::default_registry().engines().any(|e| e.name() == name)
}

/// Checks the file starts the way `fmt` files do.
pub fn assert_magic(fmt: &str, path: &Path) {
    let b = std::fs::read(path).unwrap();
    let ok = match fmt {
        "jpeg" => b.starts_with(&[0xFF, 0xD8, 0xFF]),
        "png" => b.starts_with(b"\x89PNG\r\n\x1a\n"),
        "webp" => b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP",
        "avif" => {
            b.len() > 12 && &b[4..8] == b"ftyp" && (&b[8..12] == b"avif" || &b[8..12] == b"avis")
        }
        "gif" => b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a"),
        "tiff" => b.starts_with(b"II*\0") || b.starts_with(b"MM\0*"),
        "bmp" => b.starts_with(b"BM"),
        "ico" => b.starts_with(&[0, 0, 1, 0]),
        // TGA has no leading magic; check the header's image type and that it
        // has a size (the caller compares the real dimensions).
        "tga" => {
            b.len() > 18
                && matches!(b[2], 2 | 3 | 10 | 11)
                && u16::from_le_bytes([b[12], b[13]]) > 0
        }
        "ppm" => b.starts_with(b"P6"),
        "qoi" => b.starts_with(b"qoif"),
        "exr" => b.starts_with(&[0x76, 0x2F, 0x31, 0x01]),
        "heic" => {
            b.len() > 12 && &b[4..8] == b"ftyp" && matches!(&b[8..12], b"heic" | b"heix" | b"mif1")
        }
        other => panic!("no magic check for {other}"),
    };
    assert!(ok, "{} does not look like {fmt}: {:02x?}", path.display(), &b[..b.len().min(16)]);
}

/// Decodes any output back to RGBA through the engine (so AVIF and friends
/// go through our own decoders) and returns it.
pub fn read_back(fmt: &'static str, path: &Path) -> RgbaImage {
    let png = if fmt == "png" {
        path.to_path_buf()
    } else {
        let png = path.with_extension("readback.png");
        convert(fmt, "png", path, &png).unwrap();
        png
    };
    image::open(&png).unwrap().to_rgba8()
}

/// Lossy formats and palettes get more slack.
pub fn tolerance(fmt: &str) -> u8 {
    match fmt {
        "jpeg" | "webp" | "avif" | "gif" => 24,
        _ => 2,
    }
}

pub fn assert_close(label: &str, got: [u8; 3], want: [u8; 3], tol: u8) {
    let off = got.iter().zip(want).map(|(&g, w)| g.abs_diff(w)).max().unwrap();
    assert!(off <= tol, "{label}: got {got:?}, want {want:?} (tolerance {tol})");
}
