//! LibreOffice past its time limit is killed with its whole process tree.
//! A test binary of its own, so no other LibreOffice runs meanwhile and the
//! process count is meaningful.

mod common;

use common::*;
use std::process::Command;
use std::time::{Duration, Instant};

/// Running LibreOffice processes (`soffice.bin`; on macOS the app's own
/// `soffice`).
fn soffice_bins() -> usize {
    if cfg!(windows) {
        let out = Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq soffice.bin", "/FO", "CSV", "/NH"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).lines().filter(|l| l.contains("soffice.bin")).count()
    } else {
        let out = Command::new("pgrep").args(["-f", "soffice"]).output().unwrap();
        String::from_utf8_lossy(&out.stdout).lines().count()
    }
}

#[test]
fn timeout_kills_libreoffice() {
    if tumble_engines::office::soffice().is_none() {
        eprintln!("skipped: LibreOffice not found");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let txt = dir.path().join("slow.txt");
    std::fs::write(&txt, "x").unwrap();
    // Build the profile template first, so the timed run is a normal one.
    let warm = tumble([txt.as_os_str(), "--to".as_ref(), "odt".as_ref()]);
    assert!(warm.status.success(), "{}", stderr(&warm));

    // Big enough that no machine converts it within the limit.
    std::fs::write(
        &txt,
        "A line of text to lay out across many pages.
"
        .repeat(200_000),
    )
    .unwrap();

    let before = soffice_bins();
    let start = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_tumble"))
        .env("TUMBLE_SOFFICE_TIMEOUT", "0.2")
        .args([txt.as_os_str(), "--to".as_ref(), "pdf".as_ref()])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("took longer than"), "{}", stderr(&out));
    assert!(start.elapsed() < Duration::from_secs(5), "took {:?}", start.elapsed());
    assert!(!dir.path().join("slow.pdf").exists());
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(soffice_bins(), before, "no soffice.bin left running");
}
