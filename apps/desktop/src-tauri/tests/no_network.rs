//! PRD section 14: no HTTP or TLS client in the desktop window either.
//!
//! Checked for the desktop targets Tumble ships (Windows, Linux and macOS)
//! only: Tauri itself depends on reqwest for its Android and iOS builds,
//! which Tumble never makes.

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

const TARGETS: &[&str] = &[
    "x86_64-pc-windows-msvc",
    "x86_64-unknown-linux-gnu",
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
];

#[test]
fn desktop_builds_have_no_network_clients() {
    for target in TARGETS {
        check(target);
    }
}

fn check(target: &str) {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(cargo)
        .args(["tree", "--prefix", "none", "--edges", "normal,build"])
        .args(["--target", target])
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
    assert!(found.is_empty(), "network crates in the {target} desktop build: {found:?}");
    assert!(tree.contains("tumble-engines"));
}
