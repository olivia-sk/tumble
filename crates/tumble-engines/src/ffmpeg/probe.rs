//! What is inside a media file, from `ffprobe -show_format -show_streams`.

use crate::tools;
use serde_json::Value;
use std::path::Path;
use tumble_core::EngineError;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Media {
    /// Seconds, when known.
    pub duration: Option<f64>,
    pub video: Option<VideoStream>,
    pub audio: Option<AudioStream>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VideoStream {
    pub codec: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioStream {
    pub codec: String,
    pub channels: u32,
}

pub fn probe(ffprobe: &Path, input: &Path) -> Result<Media, EngineError> {
    let out = tools::command(ffprobe)
        .args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams"])
        .arg(super::plan::file_url(input))
        .stdin(std::process::Stdio::null())
        .output()?;
    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stderr);
        let msg = msg.lines().last().unwrap_or("unknown error").trim();
        return Err(EngineError::failed(format!("cannot read media file: {msg}")));
    }
    parse(&out.stdout)
}

/// Parses ffprobe's JSON. The first real video stream (not cover art) and
/// the first audio stream are kept.
pub fn parse(json: &[u8]) -> Result<Media, EngineError> {
    let v: Value = serde_json::from_slice(json)
        .map_err(|e| EngineError::failed(format!("unexpected ffprobe output: {e}")))?;
    let num = |v: &Value| -> Option<f64> {
        v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok()))
    };
    let mut media = Media {
        duration: num(&v["format"]["duration"]).filter(|d| d.is_finite() && *d > 0.0),
        ..Default::default()
    };
    for s in v["streams"].as_array().into_iter().flatten() {
        let codec = s["codec_name"].as_str().unwrap_or_default().to_string();
        match s["codec_type"].as_str() {
            Some("video") if media.video.is_none() && s["disposition"]["attached_pic"] != 1 => {
                media.video = Some(VideoStream {
                    codec,
                    width: s["width"].as_u64().unwrap_or(0) as u32,
                    height: s["height"].as_u64().unwrap_or(0) as u32,
                });
            }
            Some("audio") if media.audio.is_none() => {
                media.audio = Some(AudioStream {
                    codec,
                    channels: s["channels"].as_u64().unwrap_or(0) as u32,
                });
            }
            _ => {}
        }
    }
    Ok(media)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_cover_art_and_reads_duration() {
        let json = br#"{
            "streams": [
                {"codec_type": "audio", "codec_name": "mp3", "channels": 2},
                {"codec_type": "video", "codec_name": "mjpeg", "width": 500, "height": 500,
                 "disposition": {"attached_pic": 1}}
            ],
            "format": {"duration": "12.500000"}
        }"#;
        let m = parse(json).unwrap();
        assert_eq!(m.duration, Some(12.5));
        assert_eq!(m.video, None);
        assert_eq!(m.audio, Some(AudioStream { codec: "mp3".into(), channels: 2 }));
    }

    #[test]
    fn reads_video() {
        let json = br#"{"streams": [{"codec_type": "video", "codec_name": "h264",
            "width": 1920, "height": 1080, "disposition": {"attached_pic": 0}}], "format": {}}"#;
        let m = parse(json).unwrap();
        assert_eq!(m.duration, None);
        assert_eq!(m.video.unwrap().codec, "h264");
    }
}
