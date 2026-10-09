//! Turning a step into FFmpeg arguments. Pure, so it is unit-tested
//! without FFmpeg.
//!
//! | Output     | Video                 | Audio            |
//! |------------|-----------------------|------------------|
//! | MP4, MOV   | H.264 (libx264)       | AAC              |
//! | MKV        | H.264 (libx264)       | AAC              |
//! | WebM       | VP9 (libvpx-vp9)      | Opus             |
//! | AVI        | MPEG-4 part 2         | MP3              |
//! | MP3, WAV, FLAC, AAC, M4A, OGG, Opus | - | LAME, PCM, FLAC, AAC, AAC, Vorbis, Opus |
//!
//! When the source streams already suit the target container and nothing
//! asks for re-encoding, streams are copied (`-c copy`), which is lossless
//! and near-instant. Arguments are always an array, never a shell string;
//! paths go in as `file:` URLs so no file name is read as an option or a
//! protocol.

use super::probe::Media;
use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;
use tumble_core::{ConvertOptions, EngineError, FormatId, Kind};

/// Animated GIF defaults: frame rate and widest side when no `--resize`.
const GIF_FPS: u32 = 15;
const GIF_MAX_WIDTH: u32 = 640;

/// What to run for one step.
#[derive(Debug, PartialEq)]
pub enum Plan {
    /// Run FFmpeg; it writes the output itself.
    Encode(Vec<OsString>),
    /// Run FFmpeg to grab one frame as PNG into `frame`, then convert that
    /// PNG to the target with the built-in codecs.
    Frame(Vec<OsString>),
}

/// `file:C:\path\to\clip.mp4`, absolute.
pub fn file_url(path: &Path) -> OsString {
    let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut url = OsString::from("file:");
    url.push(abs.as_os_str());
    url
}

fn args(list: &[&str]) -> Vec<OsString> {
    list.iter().map(OsString::from).collect()
}

/// Arguments every run starts with: quiet, never prompt, progress on stdout.
fn prelude() -> Vec<OsString> {
    args(&[
        "-hide_banner",
        "-nostdin",
        "-loglevel",
        "error",
        "-y",
        "-progress",
        "pipe:1",
        "-nostats",
    ])
}

fn muxer(to: &str) -> &'static str {
    match to {
        "mp4" => "mp4",
        "mov" => "mov",
        "mkv" => "matroska",
        "webm" => "webm",
        "avi" => "avi",
        "mp3" => "mp3",
        "wav" => "wav",
        "flac" => "flac",
        "aac" => "adts",
        "m4a" => "ipod",
        "ogg" => "ogg",
        "opus" => "opus",
        "gif" => "gif",
        other => unreachable!("no muxer for {other}"),
    }
}

/// Video codecs each container takes as-is when copying.
fn copyable_video(to: &str, codec: &str) -> bool {
    match to {
        "mp4" | "mov" => matches!(codec, "h264" | "hevc" | "av1" | "mpeg4"),
        "mkv" => matches!(codec, "h264" | "hevc" | "vp8" | "vp9" | "av1" | "mpeg4"),
        "webm" => matches!(codec, "vp8" | "vp9" | "av1"),
        "avi" => codec == "mpeg4",
        _ => false,
    }
}

fn copyable_audio(to: &str, codec: &str) -> bool {
    match to {
        "mp4" | "mov" => matches!(codec, "aac" | "mp3" | "ac3"),
        "mkv" => matches!(codec, "aac" | "mp3" | "opus" | "vorbis" | "flac" | "ac3" | "pcm_s16le"),
        "webm" => matches!(codec, "opus" | "vorbis"),
        "avi" => matches!(codec, "mp3" | "ac3"),
        "mp3" => codec == "mp3",
        "aac" | "m4a" => codec == "aac",
        "ogg" => codec == "vorbis",
        "opus" => codec == "opus",
        "flac" => codec == "flac",
        "wav" => codec == "pcm_s16le",
        _ => false,
    }
}

/// Audio encoder arguments for an output format.
fn audio_encoder(to: &str, options: &ConvertOptions) -> Vec<OsString> {
    let kbps = |default: u32| {
        let k = options.audio_kbps.unwrap_or_else(|| match options.quality {
            Some(q) => 64 + u32::from(q) * 256 / 100,
            None => default,
        });
        format!("{k}k")
    };
    let mut v = match to {
        "mp4" | "mov" | "mkv" | "aac" | "m4a" => args(&["-c:a", "aac", "-b:a", &kbps(192)]),
        "webm" | "opus" => args(&["-c:a", "libopus", "-b:a", &kbps(128)]),
        "avi" | "mp3" => args(&["-c:a", "libmp3lame", "-b:a", &kbps(192)]),
        "ogg" if super::has_encoder("libvorbis") => {
            args(&["-c:a", "libvorbis", "-b:a", &kbps(192)])
        }
        // Homebrew's FFmpeg is built without libvorbis. FFmpeg's own Vorbis
        // encoder is marked experimental and only writes stereo.
        "ogg" => {
            return args(&[
                "-c:a",
                "vorbis",
                "-strict",
                "experimental",
                "-b:a",
                &kbps(192),
                "-ac",
                "2",
            ]);
        }
        "flac" => args(&["-c:a", "flac"]),
        "wav" => args(&["-c:a", "pcm_s16le"]),
        other => unreachable!("no audio encoder for {other}"),
    };
    if let Some(ch) = options.audio_channels {
        v.extend(args(&["-ac", &ch.to_string()]));
    }
    v
}

/// Video encoder arguments. `quality` 0-100 maps onto each encoder's own
/// scale; a preset's `crf` (x264 scale) wins over it.
fn video_encoder(to: &str, options: &ConvertOptions) -> Vec<OsString> {
    let scaled =
        |max: f32| options.quality.map(|q| (max - max * f32::from(q) / 100.0).round() as u32);
    match to {
        "webm" => {
            let crf =
                options.crf.map(|c| (u32::from(c) + 9).min(63)).or(scaled(63.0)).unwrap_or(32);
            args(&[
                "-c:v",
                "libvpx-vp9",
                "-crf",
                &crf.to_string(),
                "-b:v",
                "0",
                "-row-mt",
                "1",
                "-deadline",
                "good",
                "-cpu-used",
                "4",
                "-pix_fmt",
                "yuv420p",
            ])
        }
        "avi" => {
            let q = options
                .crf
                .map(|c| (u32::from(c) / 6).max(2))
                .or(options.quality.map(|q| (31 - u32::from(q) * 29 / 100).clamp(2, 31)))
                .unwrap_or(4);
            args(&["-c:v", "mpeg4", "-q:v", &q.to_string(), "-pix_fmt", "yuv420p"])
        }
        _ => {
            let crf = options.crf.map(u32::from).or(scaled(51.0)).unwrap_or(23);
            args(&[
                "-c:v",
                "libx264",
                "-preset",
                "medium",
                "-crf",
                &crf.to_string(),
                "-pix_fmt",
                "yuv420p",
            ])
        }
    }
}

/// Scale filter: fit `--resize` (never upscaling) and keep both sides even,
/// which 4:2:0 encoders require.
fn scale_filter(options: &ConvertOptions) -> String {
    match options.resize {
        Some(r) => format!(
            "scale='min({w},iw)':'min({h},ih)':force_original_aspect_ratio=decrease:force_divisible_by=2",
            w = r.max_width,
            h = r.max_height
        ),
        None => "scale=trunc(iw/2)*2:trunc(ih/2)*2".to_string(),
    }
}

fn seconds(d: Duration) -> String {
    format!("{:.3}", d.as_secs_f64())
}

/// Builds the plan for `from -> to`. `output` is where FFmpeg writes (for
/// `Plan::Frame`, the PNG frame).
pub fn build(
    from: FormatId,
    to: FormatId,
    media: &Media,
    options: &ConvertOptions,
    input: &Path,
    output: &Path,
) -> Result<Plan, EngineError> {
    let (from_kind, to_kind) = (from.format().kind, to.format().kind);
    let to_s = to.as_str();
    let mut a = prelude();

    match (from_kind, to_kind) {
        // Video or GIF to video.
        (Kind::Video | Kind::AnimatedImage, Kind::Video) => {
            let video = media
                .video
                .as_ref()
                .ok_or_else(|| EngineError::failed("the file has no video stream"))?;
            a.extend([OsString::from("-i"), file_url(input)]);
            a.extend(args(&["-map", "0:v:0", "-map", "0:a:0?", "-sn", "-dn"]));
            let reencode =
                options.resize.is_some() || options.quality.is_some() || options.crf.is_some();
            let audio_ok = media.audio.as_ref().is_none_or(|au| copyable_audio(to_s, &au.codec));
            if !reencode
                && from_kind == Kind::Video
                && copyable_video(to_s, &video.codec)
                && audio_ok
            {
                a.extend(args(&["-c", "copy"]));
            } else {
                a.extend(args(&["-vf", &scale_filter(options)]));
                a.extend(video_encoder(to_s, options));
                if media.audio.is_some() {
                    a.extend(audio_encoder(to_s, options));
                } else {
                    a.push("-an".into());
                }
            }
            if matches!(to_s, "mp4" | "mov") {
                a.extend(args(&["-movflags", "+faststart"]));
            }
            a.extend(args(&["-f", muxer(to_s)]));
            a.push(file_url(output));
            Ok(Plan::Encode(a))
        }

        // Video or audio to audio.
        (Kind::Video | Kind::Audio, Kind::Audio) => {
            let audio = media
                .audio
                .as_ref()
                .ok_or_else(|| EngineError::failed("the file has no audio track to convert"))?;
            a.extend([OsString::from("-i"), file_url(input)]);
            a.extend(args(&["-map", "0:a:0", "-vn", "-sn", "-dn"]));
            let reencode = options.audio_kbps.is_some()
                || options.audio_channels.is_some()
                || options.quality.is_some();
            if !reencode && copyable_audio(to_s, &audio.codec) {
                a.extend(args(&["-c:a", "copy"]));
            } else {
                a.extend(audio_encoder(to_s, options));
            }
            a.extend(args(&["-f", muxer(to_s)]));
            a.push(file_url(output));
            Ok(Plan::Encode(a))
        }

        // Video to animated GIF, with a palette made from the clip itself.
        (Kind::Video, Kind::AnimatedImage) => {
            a.extend([OsString::from("-i"), file_url(input)]);
            let scale = match options.resize {
                Some(r) => format!(
                    "scale='min({},iw)':'min({},ih)':force_original_aspect_ratio=decrease:flags=lanczos",
                    r.max_width, r.max_height
                ),
                None => format!("scale='min({GIF_MAX_WIDTH},iw)':-2:flags=lanczos"),
            };
            let filter = format!(
                "fps={GIF_FPS},{scale},split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4"
            );
            a.extend(args(&["-map", "0:v:0", "-an", "-sn", "-dn", "-vf", &filter, "-loop", "0"]));
            a.extend(args(&["-f", "gif"]));
            a.push(file_url(output));
            Ok(Plan::Encode(a))
        }

        // Video to a still image: one frame, at --at or 10% in.
        (Kind::Video, Kind::StillImage) => {
            let duration = media.duration.unwrap_or(0.0);
            let at = match options.at {
                Some(at) if media.duration.is_some_and(|d| at.as_secs_f64() >= d) => {
                    return Err(EngineError::failed(format!(
                        "--at {:.1}s is past the end of the video ({duration:.1}s)",
                        at.as_secs_f64()
                    )));
                }
                Some(at) => at,
                None => Duration::from_secs_f64(duration * 0.1),
            };
            a.extend(args(&["-ss", &seconds(at)]));
            a.extend([OsString::from("-i"), file_url(input)]);
            a.extend(args(&["-map", "0:v:0", "-frames:v", "1", "-update", "1", "-c:v", "png"]));
            a.extend(args(&["-f", "image2"]));
            a.push(file_url(output));
            Ok(Plan::Frame(a))
        }

        _ => Err(EngineError::failed(format!("FFmpeg does not convert {from} to {to}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffmpeg::probe::{AudioStream, VideoStream};
    use tumble_core::Resize;

    fn media(v: Option<&str>, a: Option<&str>) -> Media {
        Media {
            duration: Some(20.0),
            video: v.map(|c| VideoStream { codec: c.into(), width: 1920, height: 1080 }),
            audio: a.map(|c| AudioStream { codec: c.into(), channels: 2 }),
        }
    }

    fn plan(from: &'static str, to: &'static str, m: &Media, o: &ConvertOptions) -> Vec<String> {
        let p = build(FormatId(from), FormatId(to), m, o, &input(), &output()).unwrap();
        let (Plan::Encode(a) | Plan::Frame(a)) = p;
        a.into_iter().map(|s| s.into_string().unwrap()).collect()
    }

    fn root() -> std::path::PathBuf {
        if cfg!(windows) { "C:\\".into() } else { "/".into() }
    }

    fn input() -> std::path::PathBuf {
        root().join("in").join("-clip.x")
    }

    fn output() -> std::path::PathBuf {
        root().join("out").join("o.x")
    }

    fn has(args: &[String], pair: &[&str]) -> bool {
        args.windows(pair.len()).any(|w| w.iter().zip(pair).all(|(a, b)| a == b))
    }

    #[test]
    fn remuxes_when_streams_fit() {
        let a = plan("mkv", "mp4", &media(Some("h264"), Some("aac")), &ConvertOptions::default());
        assert!(has(&a, &["-c", "copy"]), "{a:?}");
        assert!(has(&a, &["-movflags", "+faststart"]));
        assert!(has(&a, &["-f", "mp4"]));
    }

    #[test]
    fn reencodes_when_streams_do_not_fit_or_options_ask() {
        let a = plan("mp4", "webm", &media(Some("h264"), Some("aac")), &ConvertOptions::default());
        assert!(has(&a, &["-c:v", "libvpx-vp9"]) && has(&a, &["-c:a", "libopus"]), "{a:?}");
        let small = ConvertOptions {
            resize: Some(Resize { max_width: 1280, max_height: 720 }),
            ..Default::default()
        };
        let a = plan("mkv", "mp4", &media(Some("h264"), Some("aac")), &small);
        assert!(has(&a, &["-c:v", "libx264"]), "resize forces re-encode");
        assert!(a.iter().any(|s| s.contains("min(1280,iw)")));
    }

    #[test]
    fn paths_are_file_urls() {
        let a = plan("mp4", "mp3", &media(Some("h264"), Some("aac")), &ConvertOptions::default());
        let url = |p: std::path::PathBuf| format!("file:{}", p.display());
        assert!(a.contains(&url(input())), "a leading dash stays a path: {a:?}");
        assert_eq!(a.last().unwrap(), &url(output()));
    }

    #[test]
    fn quality_and_presets_map_to_encoder_scales() {
        let q = ConvertOptions { quality: Some(100), ..Default::default() };
        let a = plan("mov", "mp4", &media(Some("prores"), None), &q);
        assert!(has(&a, &["-crf", "0"]) && a.contains(&"-an".to_string()));
        let preset = ConvertOptions { crf: Some(28), ..Default::default() };
        assert!(has(&plan("mov", "mp4", &media(Some("prores"), None), &preset), &["-crf", "28"]));
        let voice =
            ConvertOptions { audio_kbps: Some(96), audio_channels: Some(1), ..Default::default() };
        let a = plan("wav", "mp3", &media(None, Some("pcm_s16le")), &voice);
        assert!(has(&a, &["-b:a", "96k"]) && has(&a, &["-ac", "1"]));
    }

    #[test]
    fn audio_copy_and_missing_tracks() {
        let a = plan("m4a", "aac", &media(None, Some("aac")), &ConvertOptions::default());
        assert!(has(&a, &["-c:a", "copy"]) && has(&a, &["-f", "adts"]));
        let err = build(
            FormatId("mp4"),
            FormatId("mp3"),
            &media(Some("h264"), None),
            &ConvertOptions::default(),
            Path::new("a"),
            Path::new("b"),
        );
        assert!(err.unwrap_err().to_string().contains("no audio track"));
    }

    #[test]
    fn frames_default_to_ten_percent_and_check_at() {
        let a = plan("mp4", "png", &media(Some("h264"), None), &ConvertOptions::default());
        assert!(has(&a, &["-ss", "2.000"]) && has(&a, &["-frames:v", "1"]), "{a:?}");
        let late = ConvertOptions { at: Some(Duration::from_secs(30)), ..Default::default() };
        let err = build(
            FormatId("mp4"),
            FormatId("png"),
            &media(Some("h264"), None),
            &late,
            Path::new("a"),
            Path::new("b"),
        );
        assert!(err.unwrap_err().to_string().contains("past the end"));
    }

    #[test]
    fn gif_gets_a_palette_and_gif_input_never_copies() {
        let a = plan("mp4", "gif", &media(Some("h264"), Some("aac")), &ConvertOptions::default());
        assert!(a.iter().any(|s| s.contains("palettegen")) && has(&a, &["-loop", "0"]));
        let a = plan("gif", "mp4", &media(Some("gif"), None), &ConvertOptions::default());
        assert!(has(&a, &["-c:v", "libx264"]) && !has(&a, &["-c", "copy"]));
    }
}
