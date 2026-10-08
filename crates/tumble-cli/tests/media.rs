//! Video, audio, presets and config.toml through `tumble.exe`. FFmpeg tests
//! skip when FFmpeg is missing.

mod common;

use common::*;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn ffmpeg() -> Option<PathBuf> {
    let found = tumble_engines::ffmpeg::tools().map(|(f, _)| f.clone());
    if found.is_none() {
        eprintln!("skipped: FFmpeg not found");
    }
    found
}

fn tone(ffmpeg: &Path, out: &Path) {
    let status = Command::new(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "sine=duration=1"])
        .arg(out)
        .status()
        .unwrap();
    assert!(status.success());
}

/// Runs tumble.exe with `APPDATA` pointed at `appdata`, so config.toml and
/// presets.toml come from there.
fn tumble_with_appdata(appdata: &Path, args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tumble")).env("APPDATA", appdata).args(args).output().unwrap()
}

#[test]
fn awkward_names_and_long_paths_through_ffmpeg() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let dir = tempfile::tempdir().unwrap();
    let mut deep = dir.path().to_path_buf();
    while deep.as_os_str().len() < 280 {
        deep.push("a-rather-long-folder-name-for-testing");
    }
    std::fs::create_dir_all(&deep).unwrap();
    for name in ["-dash.wav", "песня 歌 🎵.wav", "with space.wav"] {
        let wav = deep.join(name);
        // Make the fixture under a plain name; what is under test is tumble
        // handing the awkward name to FFmpeg.
        let plain = dir.path().join("plain.wav");
        tone(&ffmpeg, &plain);
        std::fs::rename(&plain, &wav).unwrap();
        let out = tumble(["--to".as_ref(), "mp3".as_ref(), "--".as_ref(), wav.as_os_str()]);
        assert!(out.status.success(), "{name}: {}", stderr(&out));
        assert!(wav.with_extension("mp3").is_file(), "{name}");
    }
}

#[test]
fn presets_command_and_flag() {
    let out = tumble(["presets"]);
    assert!(out.status.success());
    let text = stdout(&out);
    for name in ["web", "small-video", "voice"] {
        assert!(text.contains(name), "{text}");
    }
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("big.png");
    image::RgbImage::from_pixel(3000, 1000, image::Rgb([10, 20, 30])).save(&png).unwrap();
    let out = tumble([png.as_os_str(), "--preset".as_ref(), "web".as_ref()]);
    assert!(out.status.success(), "{}", stderr(&out));
    let webp = image::open(dir.path().join("big.webp")).unwrap();
    assert_eq!((webp.width(), webp.height()), (2048, 683), "web preset: WebP, at most 2048 px");
    assert_eq!(
        tumble([png.as_os_str(), "--preset".as_ref(), "nope".as_ref()]).status.code(),
        Some(2)
    );
}

#[test]
fn user_presets_and_config_toml() {
    let appdata = tempfile::tempdir().unwrap();
    let data = appdata.path().join("Tumble");
    std::fs::create_dir_all(&data).unwrap();
    let out_dir = appdata.path().join("converted");
    std::fs::write(
        data.join("config.toml"),
        format!("output = '{}'\nquality = 10\n", out_dir.display()),
    )
    .unwrap();
    std::fs::write(
        data.join("presets.toml"),
        "[thumb]\ndescription = 'Tiny JPEG'\nto = 'jpg'\nresize = '64'\n",
    )
    .unwrap();

    let list = tumble_with_appdata(appdata.path(), &["presets".as_ref()]);
    assert!(
        stdout(&list).contains("thumb") && stdout(&list).contains("(yours)"),
        "{}",
        stdout(&list)
    );

    let src = tempfile::tempdir().unwrap();
    let png = src.path().join("photo.png");
    image::RgbImage::from_fn(640, 480, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 99]))
        .save(&png)
        .unwrap();
    let out = tumble_with_appdata(
        appdata.path(),
        &[png.as_os_str(), "--preset".as_ref(), "thumb".as_ref()],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    let jpg = out_dir.join("photo.jpg");
    let img = image::open(&jpg).expect("config output folder used");
    assert_eq!((img.width(), img.height()), (64, 48));

    // config quality 10 is the fallback; a typed -q wins.
    let low = std::fs::metadata(&jpg).unwrap().len();
    let out = tumble_with_appdata(
        appdata.path(),
        &[
            png.as_os_str(),
            "--to".as_ref(),
            "jpg".as_ref(),
            "-q".as_ref(),
            "95".as_ref(),
            "--overwrite".as_ref(),
        ],
    );
    assert!(out.status.success());
    let high = std::fs::metadata(&jpg).unwrap().len();
    assert!(high > low * 2, "q95 ({high} bytes) vs config q10 thumb ({low} bytes)");

    // A broken config.toml is reported and ignored.
    std::fs::write(data.join("config.toml"), "qualty = 5").unwrap();
    let out =
        tumble_with_appdata(appdata.path(), &[png.as_os_str(), "--to".as_ref(), "bmp".as_ref()]);
    assert!(out.status.success());
    assert!(stderr(&out).contains("warning: ignoring"), "{}", stderr(&out));
    assert!(src.path().join("photo.bmp").is_file(), "falls back to same-folder output");
}

#[test]
fn without_ffmpeg_video_and_audio_disappear() {
    // A lone copy of tumble.exe with PATH and the usual install folders
    // pointed somewhere empty.
    let dir = tempfile::tempdir().unwrap();
    let lone = dir.path().join("tumble.exe");
    std::fs::copy(env!("CARGO_BIN_EXE_tumble"), &lone).unwrap();
    let empty = dir.path().join("empty");
    std::fs::create_dir(&empty).unwrap();
    let run = |args: &[&str]| {
        Command::new(&lone)
            .args(args)
            .env("PATH", r"C:\Windows\System32")
            .env("LOCALAPPDATA", &empty)
            .env("USERPROFILE", &empty)
            .env("APPDATA", &empty)
            .env_remove("TUMBLE_FFMPEG")
            .env_remove("TUMBLE_FFPROBE")
            .output()
            .unwrap()
    };
    if Path::new(r"C:\Program Files\ffmpeg\bin\ffmpeg.exe").exists()
        || Path::new(r"C:\ffmpeg\bin\ffmpeg.exe").exists()
        || Path::new(r"C:\ProgramData\chocolatey\bin\ffmpeg.exe").exists()
    {
        eprintln!("skipped: FFmpeg is installed in a machine-wide folder");
        return;
    }
    assert_eq!(run(&["targets", "clip.mp4"]).status.code(), Some(3));
    assert_eq!(run(&["targets", "song.flac"]).status.code(), Some(3));
    let engines = String::from_utf8(run(&["engines"]).stdout).unwrap();
    assert!(
        engines.contains("ffmpeg [missing]") && engines.contains("winget install Gyan.FFmpeg"),
        "{engines}"
    );
    let targets = String::from_utf8(run(&["targets", "anim.gif"]).stdout).unwrap();
    assert!(!targets.contains("mp4"), "GIF to video needs FFmpeg: {targets}");
}

#[test]
fn ffmpeg_env_override_wins() {
    let Some(real) = ffmpeg() else { return };
    let dir = tempfile::tempdir().unwrap();
    let wav = dir.path().join("t.wav");
    tone(&real, &wav);
    // Pointing TUMBLE_FFMPEG at a missing file falls through to the next
    // place; pointing it at the real one works.
    let out = Command::new(env!("CARGO_BIN_EXE_tumble"))
        .env("TUMBLE_FFMPEG", &real)
        .args([wav.as_os_str(), "--to".as_ref(), "flac".as_ref()])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let engines = Command::new(env!("CARGO_BIN_EXE_tumble"))
        .env("TUMBLE_FFMPEG", &real)
        .arg("engines")
        .output()
        .unwrap();
    assert!(stdout(&engines).contains(&real.display().to_string()), "{}", stdout(&engines));
}

#[test]
fn same_format_conversions_resize_and_keep_the_input() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("wide.png");
    image::RgbImage::from_pixel(800, 400, image::Rgb([1, 2, 3])).save(&png).unwrap();
    let out = tumble([
        png.as_os_str(),
        "--to".as_ref(),
        "png".as_ref(),
        "--resize".as_ref(),
        "200".as_ref(),
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(image::open(&png).unwrap().width(), 800, "input untouched");
    assert_eq!(image::open(dir.path().join("wide (1).png")).unwrap().width(), 200);

    let Some(ffmpeg) = ffmpeg() else { return };
    let mp4 = dir.path().join("clip.mp4");
    let status = Command::new(&ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i"])
        .args(["testsrc2=size=1920x1080:duration=1", "-c:v", "libx264", "-pix_fmt", "yuv420p"])
        .arg(&mp4)
        .status()
        .unwrap();
    assert!(status.success());
    let out = tumble([mp4.as_os_str(), "--preset".as_ref(), "small-video".as_ref()]);
    assert!(out.status.success(), "{}", stderr(&out));
    let probe = tumble_engines::ffmpeg::probe::probe(
        &tumble_engines::ffmpeg::tools().unwrap().1,
        &dir.path().join("clip (1).mp4"),
    )
    .unwrap();
    let v = probe.video.unwrap();
    assert_eq!((v.width, v.height), (1280, 720));
}
