//! Named presets (PRD section 10): a built-in table, plus the user's own in
//! `presets.toml` next to config.toml, which may add presets or replace
//! built-in ones by name.
//!
//! ```toml
//! [web]
//! description = "WebP for websites"
//! to = "webp"
//! quality = 80
//! resize = "2048"
//! ```

use crate::config::config_dir;
use crate::engine::{ConvertOptions, Resize};
use crate::format::{Format, FormatId};
use serde::Deserialize;
use std::collections::BTreeMap;

const BUILT_IN: &str = r#"
[web]
description = "WebP at quality 80, at most 2048 px"
to = "webp"
quality = 80
resize = "2048"

[small-video]
description = "MP4 (H.264) at 720p, CRF 28"
to = "mp4"
resize = "1280x720"
crf = 28

[voice]
description = "MP3 at 96 kbit/s, mono"
to = "mp3"
audio_kbps = 96
channels = 1
"#;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    description: Option<String>,
    to: String,
    quality: Option<u8>,
    resize: Option<String>,
    crf: Option<u8>,
    audio_kbps: Option<u32>,
    channels: Option<u8>,
}

#[derive(Debug, Clone)]
pub struct Preset {
    pub name: String,
    pub description: String,
    pub to: FormatId,
    pub options: ConvertOptions,
    pub built_in: bool,
}

impl Preset {
    /// Fills the gaps in `options` (what the user typed wins).
    pub fn apply(&self, options: &mut ConvertOptions) {
        let p = &self.options;
        options.quality = options.quality.or(p.quality);
        options.resize = options.resize.or(p.resize);
        options.crf = options.crf.or(p.crf);
        options.audio_kbps = options.audio_kbps.or(p.audio_kbps);
        options.audio_channels = options.audio_channels.or(p.audio_channels);
    }
}

fn parse(text: &str, built_in: bool) -> Result<Vec<Preset>, String> {
    let table: BTreeMap<String, Raw> = toml::from_str(text).map_err(|e| e.to_string())?;
    table
        .into_iter()
        .map(|(name, raw)| {
            let to = Format::parse(&raw.to)
                .filter(|f| f.output)
                .ok_or_else(|| format!("preset {name}: unknown output format {:?}", raw.to))?
                .id;
            if raw.quality.is_some_and(|q| q > 100) {
                return Err(format!("preset {name}: quality must be 0-100"));
            }
            let resize = raw
                .resize
                .as_deref()
                .map(str::parse::<Resize>)
                .transpose()
                .map_err(|e| format!("preset {name}: {e}"))?;
            Ok(Preset {
                description: raw.description.unwrap_or_default(),
                to,
                options: ConvertOptions {
                    quality: raw.quality,
                    resize,
                    crf: raw.crf,
                    audio_kbps: raw.audio_kbps,
                    audio_channels: raw.channels,
                    ..Default::default()
                },
                built_in,
                name,
            })
        })
        .collect()
}

/// Built-in presets merged with the user's file. Returns the list and, if
/// the user's file could not be used, why.
pub fn all() -> (Vec<Preset>, Option<String>) {
    let mut presets = parse(BUILT_IN, true).expect("built-in presets parse");
    let mut warning = None;
    if let Some(path) = config_dir().map(|d| d.join("presets.toml"))
        && let Ok(text) = std::fs::read_to_string(&path)
    {
        match parse(&text, false) {
            Ok(user) => {
                for p in user {
                    presets.retain(|b| b.name != p.name);
                    presets.push(p);
                }
            }
            Err(e) => warning = Some(format!("ignoring {}: {e}", path.display())),
        }
    }
    presets.sort_by(|a, b| a.name.cmp(&b.name));
    (presets, warning)
}

pub fn find(name: &str) -> Option<Preset> {
    all().0.into_iter().find(|p| p.name.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_ins_match_the_prd() {
        let presets = parse(BUILT_IN, true).unwrap();
        let names: Vec<&str> = presets.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["small-video", "voice", "web"]);
        let web = &presets[2];
        assert_eq!(web.to, FormatId("webp"));
        assert_eq!(web.options.quality, Some(80));
        assert_eq!(web.options.resize, Some(Resize { max_width: 2048, max_height: 2048 }));
        assert_eq!(presets[0].options.crf, Some(28));
        assert_eq!(presets[1].options.audio_channels, Some(1));
    }

    #[test]
    fn typed_options_beat_the_preset() {
        let web = parse(BUILT_IN, true).unwrap().pop().unwrap();
        let mut options = ConvertOptions { quality: Some(50), ..Default::default() };
        web.apply(&mut options);
        assert_eq!(options.quality, Some(50));
        assert!(options.resize.is_some());
    }

    #[test]
    fn bad_user_presets_are_errors() {
        assert!(parse("[x]\nto = 'svg'", false).is_err(), "input-only target");
        assert!(parse("[x]\nto = 'png'\nresize = 'huge'", false).is_err());
        assert!(parse("[x]\nto = 'png'\nspeed = 3", false).is_err());
    }
}
