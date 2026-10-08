//! Menu mode (`tumble.exe convert ...`, what the right-click menu runs):
//! batching, silence and the failure log. The dialog and toast are switched
//! off with TUMBLE_NO_UI; logs go to a temp LOCALAPPDATA. Never touches the
//! real registry.
#![cfg(windows)]

mod common;

use common::*;
use std::path::Path;
use std::process::{Child, Command};

fn menu_convert(localappdata: &Path, to: &str, file: &Path) -> Child {
    Command::new(env!("CARGO_BIN_EXE_tumble"))
        .env("TUMBLE_NO_UI", "1")
        .env("LOCALAPPDATA", localappdata)
        .args(["convert", "--to", to])
        .arg(file)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap()
}

#[test]
fn simultaneous_invocations_become_one_batch() {
    let dir = tempfile::tempdir().unwrap();
    let local = tempfile::tempdir().unwrap();
    let mut files = Vec::new();
    for i in 0..7 {
        files.push(image_file(&dir.path().join(format!("pic{i}.png"))));
    }
    let bad = dir.path().join("broken.png");
    std::fs::write(&bad, b"not a png").unwrap();
    files.push(bad);

    // Like Explorer: one process per selected file, all at once. Each uses a
    // target no other test uses, so batches never mix across tests.
    let children: Vec<Child> = files.iter().map(|f| menu_convert(local.path(), "tga", f)).collect();
    let outputs: Vec<_> = children.into_iter().map(|c| c.wait_with_output().unwrap()).collect();

    for o in &outputs {
        assert!(o.stdout.is_empty() && o.stderr.is_empty(), "menu mode is silent");
    }
    let codes: Vec<i32> = outputs.iter().map(|o| o.status.code().unwrap()).collect();
    assert_eq!(
        codes.iter().filter(|&&c| c == 1).count(),
        1,
        "one leader converted everything: {codes:?}"
    );
    assert_eq!(codes.iter().filter(|&&c| c == 0).count(), 7, "{codes:?}");
    for i in 0..7 {
        assert!(dir.path().join(format!("pic{i}.tga")).is_file());
    }

    // The failure is in today's log, with the file and the reason.
    let logs = local.path().join(r"Tumble\logs");
    let log = std::fs::read_dir(&logs).unwrap().next().unwrap().unwrap().path();
    let text = std::fs::read_to_string(log).unwrap();
    assert!(text.contains("broken.png -> tga") && text.contains("cannot"), "{text}");
}

#[test]
fn menu_status_is_read_only_and_works() {
    // Reads the real registry; never writes it.
    let out = tumble(["menu", "status"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("not installed") || text.contains("Installed for"), "{text}");
}
