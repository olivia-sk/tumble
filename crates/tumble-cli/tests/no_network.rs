//! PRD section 14: no Tumble binary may contain an HTTP or TLS client.
//! Fails if any such crate appears anywhere in the workspace dependency tree,
//! for any target platform.

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
fn dependency_tree_has_no_network_clients() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(cargo)
        .args([
            "tree",
            "--workspace",
            "--prefix",
            "none",
            "--edges",
            "normal,build",
            "--target",
            "all",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("run cargo tree");
    assert!(out.status.success(), "cargo tree failed: {}", String::from_utf8_lossy(&out.stderr));

    let tree = String::from_utf8(out.stdout).expect("utf-8 output");
    let found: Vec<&str> = tree
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| FORBIDDEN.contains(name))
        .collect();
    assert!(found.is_empty(), "network crates in dependency tree: {found:?}");
    assert!(tree.contains("tumble-core"), "unexpected cargo tree output:\n{tree}");
}
