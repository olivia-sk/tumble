//! `tumble targets <files>`: what a file can become, or with several files,
//! what every one of them can become. Judged by extension, so the files do
//! not need to exist. GNOME Files' menu uses `--menu --json`.

use crate::exit;
use serde_json::json;
use std::path::PathBuf;
use tumble_core::menu::common_menu_targets;
use tumble_core::{Format, FormatId};

pub fn run(files: &[PathBuf], menu: bool, json: bool) -> u8 {
    let mut formats = Vec::new();
    for file in files {
        let Some(format) = Format::of_path(file) else {
            eprintln!("unsupported file type: {}", file.display());
            return exit::NO_ROUTE;
        };
        formats.push(format);
    }
    let registry = tumble_engines::default_registry();
    let targets: Vec<FormatId> = if menu {
        common_menu_targets(&registry, &formats)
    } else {
        let mut all = registry.targets(formats[0].id);
        for f in &formats[1..] {
            let these = registry.targets(f.id);
            all.retain(|t| these.contains(t));
        }
        all
    };

    if json {
        let rows: Vec<_> = targets
            .iter()
            .map(|t| {
                let f = t.format();
                json!({ "id": f.id, "name": f.name, "extension": f.primary_extension() })
            })
            .collect();
        println!("{}", serde_json::Value::Array(rows));
    } else {
        for t in &targets {
            println!("{:<6} {}", t.as_str(), t.format().name);
        }
    }
    if targets.is_empty() {
        if formats.len() == 1 {
            eprintln!("no conversions available for {} files", formats[0].name);
        } else {
            eprintln!("no conversion is available for all of these files");
        }
        return exit::NO_ROUTE;
    }
    exit::OK
}
