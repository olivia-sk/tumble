//! `tumble presets`: built-in and user presets.

use crate::exit;
use tumble_core::presets;

pub fn run() -> u8 {
    let (list, warning) = presets::all();
    if let Some(w) = warning {
        eprintln!("warning: {w}");
    }
    let width = list.iter().map(|p| p.name.len()).max().unwrap_or(0);
    for p in &list {
        let origin = if p.built_in { "" } else { "  (yours)" };
        println!("{:<width$}  -> {:<5} {}{origin}", p.name, p.to.as_str(), p.description);
    }
    if let Some(dir) = tumble_core::config::config_dir() {
        println!("\nAdd your own in {}", dir.join("presets.toml").display());
    }
    exit::OK
}
