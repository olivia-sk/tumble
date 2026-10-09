//! Menu mode (`tumble convert ...`, what the right-click menu runs):
//! batching, silence and the failure log, plus `tumble pick` (the macOS
//! Quick Action) and installing the menu on macOS and Linux. The dialog and
//! notification are switched off with TUMBLE_NO_UI; settings, logs and
//! menus go to a temp folder. Never touches the real registry or home.

mod common;

use common::*;
use std::path::Path;
use std::process::{Child, Command};

fn menu_convert(localappdata: &Path, to: &str, file: &Path) -> Child {
    isolate(&mut Command::new(env!("CARGO_BIN_EXE_tumble")), localappdata)
        .env("TUMBLE_NO_UI", "1")
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
    let logs = log_dir(local.path());
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

#[test]
fn pick_converts_to_the_chosen_format() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let a = image_file(&dir.path().join("a.png"));
    let b = image_file(&dir.path().join("b.png"));
    let pick = |answer: &str| {
        isolate(&mut Command::new(env!("CARGO_BIN_EXE_tumble")), home.path())
            .env("TUMBLE_NO_UI", "1")
            .env("TUMBLE_PICK", answer)
            .args(["pick", "--"])
            .args([&a, &b])
            .output()
            .unwrap()
    };
    let out = pick("tiff");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(dir.path().join("a.tiff").is_file() && dir.path().join("b.tiff").is_file());

    // A format the menu doesn't offer for these files counts as Cancel.
    let out = pick("mp3");
    assert_eq!(out.status.code(), Some(0));
    assert!(!dir.path().join("a.mp3").exists());

    let notes = dir.path().join("notes.xyz");
    std::fs::write(&notes, "?").unwrap();
    let out = isolate(&mut Command::new(env!("CARGO_BIN_EXE_tumble")), home.path())
        .env("TUMBLE_NO_UI", "1")
        .env("TUMBLE_PICK", "png")
        .args(["pick", "--"])
        .arg(&notes)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3), "unreadable files are refused");
}

#[cfg(target_os = "linux")]
#[test]
fn linux_menu_install_status_uninstall() {
    let home = tempfile::tempdir().unwrap();
    let run = |args: &[&str]| {
        isolate(&mut Command::new(env!("CARGO_BIN_EXE_tumble")), home.path())
            .env("TUMBLE_FILE_MANAGERS", "dolphin,nemo,thunar,nautilus-python")
            .args(args)
            .output()
            .unwrap()
    };
    let out = run(&["menu", "install"]);
    assert!(out.status.success(), "{}{}", stdout(&out), stderr(&out));
    let data = home.path().join(".local/share");
    let jpeg = std::fs::read_to_string(data.join("kio/servicemenus/tumble-jpeg.desktop")).unwrap();
    let tumble = std::fs::canonicalize(env!("CARGO_BIN_EXE_tumble")).unwrap();
    assert!(
        jpeg.contains(&format!("Exec=\"{}\" convert --to png -- %F", tumble.display())),
        "{jpeg}"
    );
    assert!(data.join("nemo/actions/tumble-png-jpeg.nemo_action").is_file());
    assert!(data.join("nautilus-python/extensions/tumble.py").is_file());
    let uca = std::fs::read_to_string(home.path().join(".config/Thunar/uca.xml")).unwrap();
    assert!(uca.contains("<unique-id>tumble-jpeg-png</unique-id>"), "{uca}");

    let status = stdout(&run(&["menu", "status"]));
    assert!(status.contains("Dolphin") && status.contains("Thunar"), "{status}");

    // What the menu entry runs: tumble convert --to png -- <file>.
    let pic = image_file(&home.path().join("menu pick.jpg"));
    let out = isolate(&mut Command::new(env!("CARGO_BIN_EXE_tumble")), home.path())
        .env("TUMBLE_NO_UI", "1")
        .args(["convert", "--to", "png", "--"])
        .arg(&pic)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(home.path().join("menu pick.png").is_file());

    assert!(run(&["menu", "uninstall"]).status.success());
    assert!(!data.join("kio/servicemenus/tumble-jpeg.desktop").exists());
    assert!(
        !std::fs::read_to_string(home.path().join(".config/Thunar/uca.xml"))
            .unwrap()
            .contains("tumble-")
    );
    assert!(stdout(&run(&["menu", "status"])).contains("not installed"));
}

#[cfg(target_os = "macos")]
#[test]
fn macos_menu_install_status_uninstall() {
    let home = tempfile::tempdir().unwrap();
    let run = |args: &[&str]| {
        isolate(&mut Command::new(env!("CARGO_BIN_EXE_tumble")), home.path())
            .args(args)
            .output()
            .unwrap()
    };
    let out = run(&["menu", "install"]);
    assert!(out.status.success(), "{}{}", stdout(&out), stderr(&out));
    let workflow = home.path().join("Library/Services/Convert with Tumble.workflow/Contents");
    let doc = std::fs::read_to_string(workflow.join("document.wflow")).unwrap();
    let tumble = std::fs::canonicalize(env!("CARGO_BIN_EXE_tumble")).unwrap();
    assert!(doc.contains(&format!("exec '{}' pick --", tumble.display())), "{doc}");
    // Both files must be valid property lists.
    for file in ["Info.plist", "document.wflow"] {
        let lint =
            Command::new("/usr/bin/plutil").arg("-lint").arg(workflow.join(file)).output().unwrap();
        assert!(lint.status.success(), "{file}: {}", stdout(&lint));
    }
    assert!(stdout(&run(&["menu", "status"])).contains("Quick Actions"));
    assert!(run(&["menu", "uninstall"]).status.success());
    assert!(!workflow.exists());
}
