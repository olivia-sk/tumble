//! Per-job conversion options. Engines ignore what does not apply to them.

use std::fmt;
use std::str::FromStr;
use std::time::Duration;

#[derive(Clone, Debug, Default)]
pub struct ConvertOptions {
    /// 0-100, for lossy outputs. `None` uses the engine's default.
    pub quality: Option<u8>,
    pub resize: Option<Resize>,
    /// Frame time for video to still image.
    pub at: Option<Duration>,
    /// Video constant rate factor, in the encoder's own scale (x264: 0-51).
    /// Overrides `quality` for video. Set by presets.
    pub crf: Option<u8>,
    /// Audio bitrate in kbit/s. Overrides `quality` for audio.
    pub audio_kbps: Option<u32>,
    /// Downmix audio to this many channels.
    pub audio_channels: Option<u8>,
}

/// Fit within a box, keeping the aspect ratio. Raster images are only ever
/// shrunk; vector input (SVG) is rendered at the box size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resize {
    pub max_width: u32,
    pub max_height: u32,
}

impl Resize {
    /// The size an image of `width` x `height` gets when fitted into the box.
    /// `allow_upscale` lets small images grow to fill it.
    pub fn fit(self, width: u32, height: u32, allow_upscale: bool) -> (u32, u32) {
        if width == 0 || height == 0 {
            return (width, height);
        }
        let scale =
            f64::min(self.max_width as f64 / width as f64, self.max_height as f64 / height as f64);
        if scale >= 1.0 && !allow_upscale {
            return (width, height);
        }
        let w = (width as f64 * scale).round().max(1.0) as u32;
        let h = (height as f64 * scale).round().max(1.0) as u32;
        (w, h)
    }
}

impl FromStr for Resize {
    type Err = String;

    /// Accepts `WxH` (`1920x1080`) or a single number for the longest side.
    fn from_str(s: &str) -> Result<Resize, String> {
        let bad = || format!("invalid size {s:?}: use WxH (1920x1080) or a single number (2048)");
        let num = |t: &str| t.trim().parse::<u32>().ok().filter(|&n| n > 0);
        match s.split_once(['x', 'X']) {
            Some((w, h)) => Ok(Resize {
                max_width: num(w).ok_or_else(bad)?,
                max_height: num(h).ok_or_else(bad)?,
            }),
            None => {
                let n = num(s).ok_or_else(bad)?;
                Ok(Resize { max_width: n, max_height: n })
            }
        }
    }
}

impl fmt::Display for Resize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{}", self.max_width, self.max_height)
    }
}

/// Parses a time for `--at`: seconds (`12`, `12.5`) or clock form
/// (`1:30`, `01:02:03.25`).
pub fn parse_time(s: &str) -> Result<Duration, String> {
    let bad = || format!("invalid time {s:?}: use seconds (12.5) or [hh:]mm:ss (1:30)");
    let parts: Vec<&str> = s.trim().split(':').collect();
    if parts.len() > 3 || parts.iter().any(|p| p.is_empty()) {
        return Err(bad());
    }
    let (whole, last) = parts.split_at(parts.len() - 1);
    let seconds: f64 = last[0].parse().map_err(|_| bad())?;
    if !seconds.is_finite() || seconds < 0.0 || (!whole.is_empty() && seconds >= 60.0) {
        return Err(bad());
    }
    let mut total = seconds;
    for (i, p) in whole.iter().rev().enumerate() {
        let n: u32 = p.parse().map_err(|_| bad())?;
        if i == 0 && whole.len() == 2 && n >= 60 {
            return Err(bad());
        }
        total += f64::from(n) * 60f64.powi(i as i32 + 1);
    }
    Ok(Duration::from_secs_f64(total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_times() {
        assert_eq!(parse_time("12"), Ok(Duration::from_secs(12)));
        assert_eq!(parse_time("12.5"), Ok(Duration::from_millis(12_500)));
        assert_eq!(parse_time("1:30"), Ok(Duration::from_secs(90)));
        assert_eq!(parse_time("01:02:03.25"), Ok(Duration::from_millis(3_723_250)));
        for bad in ["", "abc", "1:60", "1:61:00", "-1", "1::2", "1:2:3:4"] {
            assert!(parse_time(bad).is_err(), "{bad:?} should not parse");
        }
    }

    #[test]
    fn parses_both_forms() {
        assert_eq!("800x600".parse(), Ok(Resize { max_width: 800, max_height: 600 }));
        assert_eq!("2048".parse(), Ok(Resize { max_width: 2048, max_height: 2048 }));
        assert!("0".parse::<Resize>().is_err());
        assert!("x600".parse::<Resize>().is_err());
        assert!("big".parse::<Resize>().is_err());
    }

    #[test]
    fn fit_keeps_aspect_and_never_upscales_raster() {
        let r = Resize { max_width: 100, max_height: 100 };
        assert_eq!(r.fit(400, 200, false), (100, 50));
        assert_eq!(r.fit(50, 20, false), (50, 20));
        assert_eq!(r.fit(50, 20, true), (100, 40));
    }
}
