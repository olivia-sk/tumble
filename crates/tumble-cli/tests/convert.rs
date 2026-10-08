//! End-to-end conversion through `tumble.exe`.

mod common;

use common::*;
use serde_json::Value;

#[test]
fn converts_next_to_the_input() {
    let dir = tempfile::tempdir().unwrap();
    let png = image_file(&dir.path().join("a.png"));
    let out = tumble([png.as_os_str(), "--to".as_ref(), "webp".as_ref()]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(is_webp(&dir.path().join("a.webp")));
    assert!(stdout(&out).contains("a.webp"));
    assert!(stderr(&out).contains("1 file converted"));
    assert_eq!(names(dir.path()), ["a.png", "a.webp"], "no temp files left behind");
}

#[test]
fn numbers_existing_outputs_unless_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let png = image_file(&dir.path().join("a.png"));
    for _ in 0..2 {
        assert!(tumble([png.as_os_str(), "--to".as_ref(), ".webp".as_ref()]).status.success());
    }
    assert_eq!(names(dir.path()), ["a (1).webp", "a.png", "a.webp"]);
    let out = tumble([png.as_os_str(), "--to".as_ref(), "webp".as_ref(), "--overwrite".as_ref()]);
    assert!(out.status.success());
    assert_eq!(names(dir.path()), ["a (1).webp", "a.png", "a.webp"]);
}

#[test]
fn never_overwrites_an_input_of_the_same_batch() {
    let dir = tempfile::tempdir().unwrap();
    let png = image_file(&dir.path().join("a.png"));
    let webp = dir.path().join("a.webp");
    std::fs::write(&webp, b"original").unwrap();
    let out = tumble([
        png.as_os_str(),
        webp.as_os_str(),
        "--to".as_ref(),
        "webp".as_ref(),
        "--overwrite".as_ref(),
    ]);
    assert_eq!(std::fs::read(&webp).unwrap(), b"original");
    assert!(is_webp(&dir.path().join("a (1).webp")));
    assert_eq!(out.status.code(), Some(1), "a.webp is not a real WebP, so re-encoding it fails");
}

#[test]
fn folders_recursion_and_output_layout() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    image_file(&src.join("x.png"));
    image_file(&src.join("sub").join("y.jpg"));
    std::fs::write(src.join("notes.txt"), b"hi").unwrap();
    let out_dir = dir.path().join("out");

    let flat = tumble([
        src.as_os_str(),
        "--to".as_ref(),
        "bmp".as_ref(),
        "-o".as_ref(),
        out_dir.as_os_str(),
    ]);
    assert!(flat.status.success(), "{}", stderr(&flat));
    assert_eq!(names(&out_dir), ["x.bmp"]);
    assert!(stderr(&flat).contains("1 skipped"), "text files are not images: {}", stderr(&flat));

    let deep = tumble([
        src.as_os_str(),
        "--to".as_ref(),
        "bmp".as_ref(),
        "-r".as_ref(),
        "-o".as_ref(),
        out_dir.as_os_str(),
    ]);
    assert!(deep.status.success(), "{}", stderr(&deep));
    assert_eq!(names(&out_dir), ["sub", "x (1).bmp", "x.bmp"]);
    assert_eq!(names(&out_dir.join("sub")), ["y.bmp"]);
    assert_eq!(names(&src), ["notes.txt", "sub", "x.png"], "inputs untouched");
}

#[test]
fn json_events() {
    let dir = tempfile::tempdir().unwrap();
    let png = image_file(&dir.path().join("a.png"));
    let out = tumble([png.as_os_str(), "--to".as_ref(), "jpg".as_ref(), "--json".as_ref()]);
    assert!(out.status.success());
    let events: Vec<Value> = stdout(&out)
        .lines()
        .map(|l| serde_json::from_str(l).expect("one JSON object per line"))
        .collect();
    let kinds: Vec<&str> = events.iter().map(|e| e["event"].as_str().unwrap()).collect();
    assert_eq!(kinds.first(), Some(&"start"));
    assert!(kinds.contains(&"progress"));
    assert_eq!(&kinds[kinds.len() - 2..], ["done", "summary"]);
    let done = &events[events.len() - 2];
    assert!(done["outputs"][0].as_str().unwrap().ends_with("a.jpg"));
    assert_eq!(done["route"], "png -> jpeg (image)");
    assert_eq!(events.last().unwrap()["converted"], 1);
}

#[test]
fn exit_codes() {
    let dir = tempfile::tempdir().unwrap();
    let png = image_file(&dir.path().join("a.png"));
    let p = png.as_os_str();
    let code = |args: &[&std::ffi::OsStr]| tumble(args).status.code();

    assert_eq!(code(&[p, "--to".as_ref(), "nope".as_ref()]), Some(2), "unknown format");
    assert_eq!(code(&[p, "--to".as_ref(), "svg".as_ref()]), Some(2), "input-only format");
    assert_eq!(code(&[p, "--to".as_ref(), "png".as_ref(), "-q".as_ref(), "101".as_ref()]), Some(2));
    assert_eq!(
        code(&[p, "--to".as_ref(), "png".as_ref(), "--resize".as_ref(), "big".as_ref()]),
        Some(2)
    );
    assert_eq!(
        code(&[p, "--to".as_ref(), "png".as_ref(), "--at".as_ref(), "1:99".as_ref()]),
        Some(2)
    );
    assert_eq!(code(&[p, "--to".as_ref(), "mp3".as_ref()]), Some(3), "no route");
    assert_eq!(
        code(&[dir.path().join("nope.png").as_os_str(), "--to".as_ref(), "bmp".as_ref()]),
        Some(1)
    );

    let bad = dir.path().join("bad.png");
    std::fs::write(&bad, b"not a png").unwrap();
    let out = tumble([p, bad.as_os_str(), "--to".as_ref(), "bmp".as_ref()]);
    assert_eq!(out.status.code(), Some(1), "one of two failed");
    assert!(dir.path().join("a.bmp").exists());
    assert!(!dir.path().join("bad.bmp").exists(), "no half-written output");
    assert!(stderr(&out).contains("1 file converted, 1 failed"), "{}", stderr(&out));
    assert!(!names(dir.path()).iter().any(|n| n.starts_with(".tumble-")), "staging removed");
}

#[test]
fn awkward_file_names() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["my photo.png", "фото 写真 🎞.png", "-dash.png", "dots.in.name.png"] {
        let png = image_file(&dir.path().join(name));
        // `--` keeps a leading dash from being read as an option.
        let out = tumble(["--to".as_ref(), "qoi".as_ref(), "--".as_ref(), png.as_os_str()]);
        assert!(out.status.success(), "{name}: {}", stderr(&out));
        assert!(png.with_extension("qoi").exists(), "{name}");
    }
}

#[test]
fn paths_longer_than_max_path() {
    let dir = tempfile::tempdir().unwrap();
    let mut deep = dir.path().to_path_buf();
    while deep.as_os_str().len() < 300 {
        deep.push("a-rather-long-folder-name-for-testing");
    }
    let png = image_file(&deep.join("long.png"));
    assert!(png.as_os_str().len() > 260);
    let out = tumble([png.as_os_str(), "--to".as_ref(), "webp".as_ref()]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(is_webp(&deep.join("long.webp")));
}

#[test]
fn parallel_jobs_get_unique_names() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..12 {
        image_file(&dir.path().join(format!("img{i}.png")));
        image_file(&dir.path().join(format!("img{i}.bmp")));
    }
    let out = tumble([
        dir.path().as_os_str(),
        "--to".as_ref(),
        "tga".as_ref(),
        "-j".as_ref(),
        "6".as_ref(),
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    let tgas: Vec<String> = names(dir.path()).into_iter().filter(|n| n.ends_with(".tga")).collect();
    assert_eq!(tgas.len(), 24);
    for i in 0..12 {
        assert!(tgas.contains(&format!("img{i}.tga")));
        assert!(tgas.contains(&format!("img{i} (1).tga")));
    }
}
