//! `tumble engines`: every engine, whether it is available, and what it
//! reads and writes.

use crate::exit;
use serde_json::json;
use tumble_core::FormatId;

pub fn run(json: bool) -> u8 {
    let report = tumble_engines::engine_report();
    if json {
        let list: Vec<_> = report
            .iter()
            .map(|e| {
                json!({
                    "name": e.name, "available": e.available, "detail": e.detail,
                    "reads": e.reads, "writes": e.writes
                })
            })
            .collect();
        println!("{}", json!({ "engines": list }));
        return exit::OK;
    }
    let names =
        |ids: &[FormatId]| ids.iter().map(|id| id.format().name).collect::<Vec<_>>().join(", ");
    for e in &report {
        let mark = if e.available { "ok" } else { "missing" };
        println!("{} [{mark}] {}", e.name, e.detail);
        if e.available {
            println!("  reads:  {}", names(&e.reads));
            println!("  writes: {}", names(&e.writes));
        }
    }
    exit::OK
}
