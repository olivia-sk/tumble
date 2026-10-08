//! Embeds the app icon (assets/tumble.ico) in both binaries on Windows.

fn main() {
    println!("cargo:rerun-if-changed=../../assets/tumble.rc");
    println!("cargo:rerun-if-changed=../../assets/tumble.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("../../assets/tumble.rc", embed_resource::NONE)
            .manifest_optional()
            .expect("compile the icon resource");
    }
}
