//! `tumblew.exe` hands its arguments to `tumble.exe` and returns its exit code.

mod common;

use common::*;
use std::process::Command;

fn tumblew(args: &[&std::ffi::OsStr]) -> Option<i32> {
    // Menu mode logs failures; keep them out of the real %LOCALAPPDATA%.
    let logs = tempfile::tempdir().unwrap();
    Command::new(env!("CARGO_BIN_EXE_tumblew"))
        .env("TUMBLE_NO_UI", "1")
        .env("LOCALAPPDATA", logs.path())
        .args(args)
        .status()
        .expect("run tumblew")
        .code()
}

#[test]
fn converts_through_tumble_exe() {
    let dir = tempfile::tempdir().unwrap();
    let png = image_file(&dir.path().join("menu pick.png"));
    let code = tumblew(&["convert".as_ref(), "--to".as_ref(), "bmp".as_ref(), png.as_os_str()]);
    assert_eq!(code, Some(0));
    assert!(dir.path().join("menu pick.bmp").exists());
}

#[test]
fn passes_exit_codes_through() {
    assert_eq!(tumblew(&["convert".as_ref(), "x.png".as_ref()]), Some(2), "missing --to");
    let code = tumblew(&["convert".as_ref(), "--to".as_ref(), "mp3".as_ref(), "x.png".as_ref()]);
    assert_eq!(code, Some(1), "missing file");
}

/// PRD section 13: our two binaries stay under 15 MB together. Only checked
/// with `cargo test --release`, since debug builds are much larger.
#[test]
fn release_binaries_fit_the_size_budget() {
    if cfg!(debug_assertions) {
        eprintln!("skipped: run with --release to check the size budget");
        return;
    }
    let size = |p: &str| std::fs::metadata(p).unwrap().len();
    let total = size(env!("CARGO_BIN_EXE_tumble")) + size(env!("CARGO_BIN_EXE_tumblew"));
    assert!(total < 15 * 1024 * 1024, "tumble.exe + tumblew.exe = {total} bytes");
    assert!(
        size(env!("CARGO_BIN_EXE_tumblew")) < 1024 * 1024,
        "tumblew.exe should stay a launcher"
    );
}
