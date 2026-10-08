//! PRD section 14: no HTTP or TLS client in the desktop window either.
//!
//! Checked for the Windows target only: Tauri itself depends on reqwest for
//! its Android and iOS builds, which Tumble never makes. For
//! x86_64-pc-windows-msvc no such crate is compiled in.

use std::process::Command;

const FORBIDDEN: &[&str] = &[
    "attohttpc",
    "awc",
    "curl",
    "curl-sys",
    "ehttp",
    "h2",
    "h3",
    "http-req",
    "hyper",
    "hyper-util",
    "isahc",
    "minreq",
    "native-tls",
    "openssl",
    "quinn",
    "reqwest",
    "rustls",
    "surf",
    "tungstenite",
    "ureq",
    "websocket",
];

#[test]
fn windows_build_has_no_network_clients() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(cargo)
        .args(["tree", "--prefix", "none", "--edges", "normal,build"])
        .args(["--target", "x86_64-pc-windows-msvc"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("run cargo tree");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let tree = String::from_utf8(out.stdout).unwrap();
    let found: Vec<&str> = tree
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .filter(|n| FORBIDDEN.contains(n))
        .collect();
    assert!(found.is_empty(), "network crates in the desktop build: {found:?}");
    assert!(tree.contains("tumble-engines"));
}
