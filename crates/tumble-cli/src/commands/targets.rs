//! `tumble targets <file>`: what a file can become. Judged by extension, so
//! the file does not need to exist.

use crate::exit;
use serde_json::json;
use std::path::Path;
use tumble_core::Format;
use tumble_core::menu::menu_targets;

pub fn run(file: &Path, menu: bool, json: bool) -> u8 {
    let Some(format) = Format::of_path(file) else {
        eprintln!("unsupported file type: {}", file.display());
        return exit::NO_ROUTE;
    };
    let registry = tumble_engines::default_registry();
    let targets = if menu { menu_targets(&registry, format) } else { registry.targets(format.id) };

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
        eprintln!("no conversions available for {} files", format.name);
        return exit::NO_ROUTE;
    }
    exit::OK
}
