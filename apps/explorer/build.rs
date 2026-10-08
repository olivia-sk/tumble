//! Exports the COM entry points as PRIVATE (see exports.def).

fn main() {
    println!("cargo:rerun-if-changed=exports.def");
    let def = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("exports.def");
    println!("cargo:rustc-cdylib-link-arg=/DEF:{}", def.display());
}
