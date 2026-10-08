//! `tumble formats`: the PRD section 7 table.

use crate::exit;
use serde::Serialize;
use tumble_core::format::{Family, Format, FormatId, Kind, image_outputs};
use tumble_core::{FORMATS, menu};

#[derive(Serialize)]
struct Row {
    #[serde(flatten)]
    format: &'static Format,
    menu_group: &'static str,
    converts_to: Vec<FormatId>,
}

pub fn run(json: bool) -> u8 {
    if json {
        let rows: Vec<Row> = FORMATS
            .iter()
            .map(|f| Row {
                format: f,
                menu_group: menu::menu_group(f),
                converts_to: f.declared_targets(),
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows).expect("rows serialize"));
        return exit::OK;
    }

    let sections: [(&str, &[Kind]); 4] = [
        ("Images", &[Kind::StillImage, Kind::AnimatedImage]),
        ("Video", &[Kind::Video]),
        ("Audio", &[Kind::Audio]),
        ("Documents", &[Kind::Document]),
    ];
    for (i, (title, kinds)) in sections.iter().enumerate() {
        if i > 0 {
            println!();
        }
        println!("{title}");
        let rows: Vec<[String; 3]> = FORMATS
            .iter()
            .filter(|f| kinds.contains(&f.kind))
            .map(|f| [f.name.to_string(), extensions(f), summarize(f)])
            .collect();
        print_table(["Format", "Extensions", "Converts to"], &rows);
    }
    println!();
    println!("Image outputs: {}", names(&image_outputs()));
    exit::OK
}

fn extensions(f: &Format) -> String {
    f.extensions.iter().map(|e| format!(".{e}")).collect::<Vec<_>>().join(" ")
}

fn names(ids: &[FormatId]) -> String {
    let mut v: Vec<&str> = ids.iter().map(|id| id.format().name).collect();
    v.sort_unstable();
    v.join(", ")
}

/// Turns a target list into the wording the PRD uses, e.g.
/// "all other image outputs, plus AVI, MKV, MOV, MP4, WebM".
fn summarize(f: &Format) -> String {
    let mut rest = f.declared_targets();
    let mut phrases = Vec::new();
    for (family, noun) in
        [(Family::Video, "video"), (Family::Image, "image"), (Family::Audio, "audio")]
    {
        let group: Vec<FormatId> = FORMATS
            .iter()
            .filter(|g| g.family == family && g.output && g.id != f.id)
            .map(|g| g.id)
            .collect();
        // A non-video input that reaches video (GIF) lists the formats by name.
        let collapse = family != Family::Video || f.family == Family::Video;
        if collapse && group.len() > 1 && group.iter().all(|g| rest.contains(g)) {
            rest.retain(|g| !group.contains(g));
            let other = if f.family == family { "other " } else { "" };
            phrases.push(format!("all {other}{noun} outputs"));
        }
    }
    let summary = match (phrases.is_empty(), rest.is_empty()) {
        (_, true) => phrases.join(", "),
        (true, false) => names(&rest),
        (false, false) => format!("{}, plus {}", phrases.join(", "), names(&rest)),
    };
    if f.output { summary } else { format!("{summary} ({} is input only)", f.name) }
}

fn print_table(header: [&str; 3], rows: &[[String; 3]]) {
    let mut widths = header.map(str::len);
    for row in rows {
        for (w, cell) in widths.iter_mut().zip(row) {
            *w = (*w).max(cell.chars().count());
        }
    }
    let line = |cells: [&str; 3]| {
        println!(
            "  {:<w0$}  {:<w1$}  {}",
            cells[0],
            cells[1],
            cells[2],
            w0 = widths[0],
            w1 = widths[1]
        );
    };
    line(header);
    line([
        "-".repeat(widths[0]).as_str(),
        "-".repeat(widths[1]).as_str(),
        "-".repeat(widths[2]).as_str(),
    ]);
    for row in rows {
        line([row[0].as_str(), row[1].as_str(), row[2].as_str()]);
    }
}
