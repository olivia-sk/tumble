//! FFmpeg engine (PRD phase 3 acceptance): every video and audio pair
//! converts and passes ffprobe checks. Skips when FFmpeg is missing.
//! Fixtures are generated with FFmpeg's own test sources.

mod common;

use common::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tumble_core::{
    CancelToken, ConvertOptions, Engine, EngineError, FormatId, Progress, Resize, Step,
};
use tumble_engines::ffmpeg::{FfmpegEngine, probe};

const VIDEOS: [&str; 5] = ["mp4", "mov", "webm", "mkv", "avi"];
const AUDIO: [&str; 7] = ["mp3", "wav", "flac", "aac", "m4a", "ogg", "opus"];

fn tools() -> Option<&'static (PathBuf, PathBuf)> {
    let t = tumble_engines::ffmpeg::tools();
    if t.is_none() {
        eprintln!("skipped: FFmpeg not found (winget install Gyan.FFmpeg)");
    }
    t
}

/// Runs ffmpeg directly to make a fixture.
fn make(args: &[&str], out: &Path) {
    let (ffmpeg, _) = tools().unwrap();
    let status = Command::new(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(args)
        .arg(out)
        .status()
        .unwrap();
    assert!(status.success(), "fixture {}", out.display());
}

/// 2 s, 320x240 test pattern with a 440 Hz tone, H.264 + AAC.
fn clip(dir: &Path) -> PathBuf {
    let out = dir.join("clip.mp4");
    make(
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=320x240:rate=25:duration=2",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=2",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
        ],
        &out,
    );
    out
}

/// 1 s, 440 Hz stereo tone as WAV.
fn tone(dir: &Path) -> PathBuf {
    let out = dir.join("tone.wav");
    make(&["-f", "lavfi", "-i", "sine=frequency=440:duration=1", "-ac", "2"], &out);
    out
}

fn run(
    from: &'static str,
    to: &'static str,
    input: &Path,
    output: &Path,
    options: &ConvertOptions,
) -> Result<(), String> {
    FfmpegEngine
        .convert(
            Step::new(from, to),
            input,
            output,
            options,
            &tumble_core::NoProgress,
            &CancelToken::new(),
        )
        .map(|_| ())
        .map_err(|e| format!("{from} -> {to}: {e}"))
}

fn info(path: &Path) -> probe::Media {
    probe::probe(&tools().unwrap().1, path).unwrap()
}

fn video_ok(container: &str, codec: &str) -> bool {
    match container {
        "mp4" | "mov" => matches!(codec, "h264" | "hevc" | "av1" | "mpeg4"),
        "mkv" => matches!(codec, "h264" | "hevc" | "vp9" | "av1" | "mpeg4"),
        "webm" => matches!(codec, "vp8" | "vp9" | "av1"),
        "avi" => codec == "mpeg4",
        _ => false,
    }
}

fn audio_codec(format: &str) -> &'static [&'static str] {
    match format {
        "mp4" | "mov" => &["aac", "mp3"],
        "mkv" => &["aac", "mp3", "opus", "vorbis"],
        "webm" => &["opus", "vorbis"],
        "avi" => &["mp3"],
        "mp3" => &["mp3"],
        "wav" => &["pcm_s16le"],
        "flac" => &["flac"],
        "aac" | "m4a" => &["aac"],
        "ogg" => &["vorbis"],
        "opus" => &["opus"],
        _ => &[],
    }
}

fn close(label: &str, got: Option<f64>, want: f64, tol: f64) {
    let got = got.unwrap_or_else(|| panic!("{label}: no duration"));
    assert!((got - want).abs() <= tol, "{label}: duration {got:.3}s, want {want}s");
}

#[test]
fn every_video_pair() {
    if tools().is_none() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let source = clip(dir.path());
    let mut sources = Vec::new();
    for to in VIDEOS {
        let out = dir.path().join(format!("src.{to}"));
        if to == "mp4" {
            std::fs::copy(&source, &out).unwrap();
        } else {
            run("mp4", to, &source, &out, &ConvertOptions::default()).unwrap();
        }
        sources.push((to, out));
    }
    let mut failures = Vec::new();
    for (from, input) in &sources {
        for to in VIDEOS.iter().chain(AUDIO.iter()) {
            if from == to {
                continue;
            }
            let out = dir.path().join(format!("{from}-to.{to}"));
            if let Err(e) = run(from, to, input, &out, &ConvertOptions::default()) {
                failures.push(e);
                continue;
            }
            let m = info(&out);
            let label = format!("{from} -> {to}");
            if VIDEOS.contains(to) {
                let v = m.video.as_ref().unwrap_or_else(|| panic!("{label}: no video"));
                assert!(video_ok(to, &v.codec), "{label}: video codec {}", v.codec);
                assert_eq!((v.width, v.height), (320, 240), "{label}");
            } else {
                assert!(m.video.is_none(), "{label}: audio file has video");
            }
            let a = m.audio.as_ref().unwrap_or_else(|| panic!("{label}: no audio"));
            assert!(
                audio_codec(to).contains(&a.codec.as_str()),
                "{label}: audio codec {}",
                a.codec
            );
            close(&label, m.duration, 2.0, 0.2);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_audio_pair() {
    if tools().is_none() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let wav = tone(dir.path());
    let mut failures = Vec::new();
    for from in AUDIO {
        let input = dir.path().join(format!("src.{from}"));
        if from == "wav" {
            std::fs::copy(&wav, &input).unwrap();
        } else {
            run("wav", from, &wav, &input, &ConvertOptions::default()).unwrap();
        }
        for to in AUDIO {
            if from == to {
                continue;
            }
            let out = dir.path().join(format!("{from}-to.{to}"));
            if let Err(e) = run(from, to, &input, &out, &ConvertOptions::default()) {
                failures.push(e);
                continue;
            }
            let m = info(&out);
            let label = format!("{from} -> {to}");
            let a = m.audio.as_ref().unwrap_or_else(|| panic!("{label}: no audio"));
            assert!(audio_codec(to).contains(&a.codec.as_str()), "{label}: codec {}", a.codec);
            assert_eq!(a.channels, 2, "{label}");
            close(&label, m.duration, 1.0, 0.15);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn video_to_every_image_output() {
    if tools().is_none() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let source = clip(dir.path());
    let registry = tumble_engines::default_registry();
    let images: Vec<FormatId> = registry
        .targets(FormatId("mp4"))
        .into_iter()
        .filter(|t| {
            t.format().kind != tumble_core::Kind::Video
                && t.format().kind != tumble_core::Kind::Audio
        })
        .collect();
    assert!(images.len() >= 12, "{images:?}");
    for to in images {
        let to = to.as_str();
        let out = dir.path().join(format!("frame.{to}"));
        run("mp4", to, &source, &out, &ConvertOptions::default()).unwrap();
        assert_magic(to, &out);
        if to == "gif" {
            let m = info(&out);
            assert!(m.duration.is_some_and(|d| d > 1.5), "animated GIF keeps the motion");
        } else {
            let want = if to == "ico" { (256, 192) } else { (320, 240) }; // ICO holds at most 256 px
            assert_eq!(read_back(to, &out).dimensions(), want, "mp4 -> {to}");
        }
    }
}

#[test]
fn frame_time_follows_at() {
    if tools().is_none() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let video = dir.path().join("redblue.mp4");
    // 1 s red, then 1 s blue.
    make(
        &[
            "-f",
            "lavfi",
            "-i",
            "color=c=red:s=64x64:d=1",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=64x64:d=1",
            "-filter_complex",
            "[0][1]concat=n=2:v=1",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv444p",
        ],
        &video,
    );
    let pixel = |options: &ConvertOptions, name: &str| {
        let out = dir.path().join(name);
        run("mp4", "png", &video, &out, options).unwrap();
        image::open(&out).unwrap().to_rgb8().get_pixel(32, 32).0
    };
    let default = pixel(&ConvertOptions::default(), "default.png");
    assert!(default[0] > 200 && default[2] < 60, "10% in is red: {default:?}");
    let late = ConvertOptions { at: Some(Duration::from_millis(1500)), ..Default::default() };
    let blue = pixel(&late, "late.png");
    assert!(blue[2] > 200 && blue[0] < 60, "--at 1.5 is blue: {blue:?}");
    let past = ConvertOptions { at: Some(Duration::from_secs(5)), ..Default::default() };
    assert!(
        run("mp4", "png", &video, &dir.path().join("x.png"), &past)
            .unwrap_err()
            .contains("past the end")
    );
}

#[test]
fn gif_to_video_and_odd_sizes() {
    if tools().is_none() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let odd = dir.path().join("odd.mp4");
    make(
        &["-f", "lavfi", "-i", "testsrc2=size=321x241:rate=10:duration=1", "-c:v", "libx264rgb"],
        &odd,
    );
    let gif = dir.path().join("anim.gif");
    run("mp4", "gif", &odd, &gif, &ConvertOptions::default()).unwrap();
    for to in VIDEOS {
        let out = dir.path().join(format!("from-gif.{to}"));
        run("gif", to, &gif, &out, &ConvertOptions::default()).unwrap();
        let v = info(&out).video.unwrap();
        assert!(video_ok(to, &v.codec), "gif -> {to}: {}", v.codec);
        assert_eq!(
            (v.width % 2, v.height % 2),
            (0, 0),
            "gif -> {to}: {}x{} must be even",
            v.width,
            v.height
        );
    }
}

#[test]
fn presets_shape_the_output() {
    if tools().is_none() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let hd = dir.path().join("hd.mov");
    make(
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=1920x1080:rate=25:duration=1",
            "-f",
            "lavfi",
            "-i",
            "sine=duration=1",
            "-c:v",
            "mpeg4",
            "-c:a",
            "pcm_s16le",
            "-shortest",
        ],
        &hd,
    );
    let mut small = ConvertOptions::default();
    tumble_core::presets::find("small-video").unwrap().apply(&mut small);
    let out = dir.path().join("small.mp4");
    run("mov", "mp4", &hd, &out, &small).unwrap();
    let v = info(&out).video.unwrap();
    assert_eq!((v.codec.as_str(), v.width, v.height), ("h264", 1280, 720));

    let mut voice = ConvertOptions::default();
    tumble_core::presets::find("voice").unwrap().apply(&mut voice);
    let out = dir.path().join("voice.mp3");
    run("mov", "mp3", &hd, &out, &voice).unwrap();
    let a = info(&out).audio.unwrap();
    assert_eq!((a.codec.as_str(), a.channels), ("mp3", 1));

    let resized = ConvertOptions {
        resize: Some(Resize { max_width: 4000, max_height: 200 }),
        ..Default::default()
    };
    let out = dir.path().join("short.webm");
    run("mov", "webm", &hd, &out, &resized).unwrap();
    let v = info(&out).video.unwrap();
    assert_eq!((v.width, v.height), (356, 200), "fits the box, keeps aspect, even sides");
}

#[test]
fn missing_audio_is_a_clear_error() {
    if tools().is_none() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let silent = dir.path().join("silent.mp4");
    make(&["-f", "lavfi", "-i", "testsrc2=size=64x64:duration=1", "-c:v", "libx264"], &silent);
    let err = run("mp4", "mp3", &silent, &dir.path().join("x.mp3"), &ConvertOptions::default())
        .unwrap_err();
    assert!(err.contains("no audio track"), "{err}");
    let bad = dir.path().join("bad.mp4");
    std::fs::write(&bad, b"not a video").unwrap();
    let err =
        run("mp4", "mkv", &bad, &dir.path().join("x.mkv"), &ConvertOptions::default()).unwrap_err();
    assert!(err.contains("cannot read media file"), "{err}");
}

struct Recorder(Mutex<Vec<f32>>);

impl Progress for Recorder {
    fn update(&self, f: f32) {
        self.0.lock().unwrap().push(f);
    }
}

#[test]
fn progress_is_reported_and_cancel_stops_ffmpeg() {
    if tools().is_none() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let long = dir.path().join("long.mp4");
    make(
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=1280x720:rate=30:duration=20",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
        ],
        &long,
    );

    // Progress: a short re-encode reports fractions that rise towards 1.
    let rec = Recorder(Mutex::new(Vec::new()));
    let short = ConvertOptions {
        resize: Some(Resize { max_width: 320, max_height: 180 }),
        ..Default::default()
    };
    FfmpegEngine
        .convert(
            Step::new("mp4", "mkv"),
            &long,
            &dir.path().join("p.mkv"),
            &short,
            &rec,
            &CancelToken::new(),
        )
        .unwrap();
    let seen = rec.0.into_inner().unwrap();
    assert!(seen.iter().any(|f| *f > 0.0 && *f < 1.0), "intermediate progress: {seen:?}");
    assert!(seen.windows(2).all(|w| w[0] <= w[1]), "monotonic: {seen:?}");

    // Cancel: a slow VP9 encode stops promptly.
    let cancel = CancelToken::new();
    let trigger = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(500));
        trigger.cancel();
    });
    let start = Instant::now();
    let result = FfmpegEngine.convert(
        Step::new("mp4", "webm"),
        &long,
        &dir.path().join("c.webm"),
        &ConvertOptions::default(),
        &tumble_core::NoProgress,
        &cancel,
    );
    assert!(matches!(result, Err(EngineError::Cancelled)), "{result:?}");
    assert!(start.elapsed() < Duration::from_secs(3), "took {:?}", start.elapsed());
}
