//! The static format table (PRD section 7).

use super::{Family as F, Format, FormatId, Kind as K};

const fn f(
    id: &'static str,
    name: &'static str,
    kind: K,
    family: F,
    extensions: &'static [&'static str],
    lossy: bool,
) -> Format {
    Format { id: FormatId(id), name, kind, family, extensions, lossy, output: true }
}

pub static FORMATS: &[Format] = &[
    // Images
    f("jpeg", "JPEG", K::StillImage, F::Image, &["jpg", "jpeg", "jfif"], true),
    f("png", "PNG", K::StillImage, F::Image, &["png"], false),
    f("webp", "WebP", K::StillImage, F::Image, &["webp"], true),
    f("heic", "HEIC", K::StillImage, F::Image, &["heic", "heif"], true),
    f("avif", "AVIF", K::StillImage, F::Image, &["avif"], true),
    f("gif", "GIF", K::AnimatedImage, F::Image, &["gif"], true),
    f("tiff", "TIFF", K::StillImage, F::Image, &["tiff", "tif"], false),
    f("bmp", "BMP", K::StillImage, F::Image, &["bmp"], false),
    f("ico", "ICO", K::StillImage, F::Image, &["ico"], false),
    f("tga", "TGA", K::StillImage, F::Image, &["tga"], false),
    f("ppm", "PPM", K::StillImage, F::Image, &["ppm", "pgm", "pbm", "pnm"], false),
    f("qoi", "QOI", K::StillImage, F::Image, &["qoi"], false),
    f("exr", "OpenEXR", K::StillImage, F::Image, &["exr"], false),
    Format {
        id: FormatId("svg"),
        name: "SVG",
        kind: K::StillImage,
        family: F::Vector,
        extensions: &["svg"],
        lossy: false,
        output: false,
    },
    // Video
    f("mp4", "MP4", K::Video, F::Video, &["mp4", "m4v"], true),
    f("mov", "MOV", K::Video, F::Video, &["mov"], true),
    f("webm", "WebM", K::Video, F::Video, &["webm"], true),
    f("mkv", "MKV", K::Video, F::Video, &["mkv"], true),
    f("avi", "AVI", K::Video, F::Video, &["avi"], true),
    // Audio
    f("mp3", "MP3", K::Audio, F::Audio, &["mp3"], true),
    f("wav", "WAV", K::Audio, F::Audio, &["wav"], false),
    f("flac", "FLAC", K::Audio, F::Audio, &["flac"], false),
    f("aac", "AAC", K::Audio, F::Audio, &["aac"], true),
    f("m4a", "M4A", K::Audio, F::Audio, &["m4a"], true),
    f("ogg", "OGG", K::Audio, F::Audio, &["ogg", "oga"], true),
    f("opus", "Opus", K::Audio, F::Audio, &["opus"], true),
    // Documents
    f("pdf", "PDF", K::Document, F::Pdf, &["pdf"], false),
    f("docx", "DOCX", K::Document, F::WordProcessing, &["docx"], false),
    f("doc", "DOC", K::Document, F::WordProcessing, &["doc"], false),
    f("odt", "ODT", K::Document, F::WordProcessing, &["odt"], false),
    f("rtf", "RTF", K::Document, F::WordProcessing, &["rtf"], false),
    f("txt", "Plain text", K::Document, F::PlainText, &["txt"], false),
    f("html", "HTML", K::Document, F::Html, &["html", "htm"], false),
    f("md", "Markdown", K::Document, F::Markdown, &["md", "markdown"], false),
    f("pptx", "PPTX", K::Document, F::Presentation, &["pptx"], false),
    f("ppt", "PPT", K::Document, F::Presentation, &["ppt"], false),
    f("odp", "ODP", K::Document, F::Presentation, &["odp"], false),
    f("xlsx", "XLSX", K::Document, F::Spreadsheet, &["xlsx"], false),
    f("xls", "XLS", K::Document, F::Spreadsheet, &["xls"], false),
    f("ods", "ODS", K::Document, F::Spreadsheet, &["ods"], false),
    f("csv", "CSV", K::Document, F::Spreadsheet, &["csv"], false),
];
