use std::process::{Command, Stdio};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ReplayGainResult {
    pub track_gain_db: f64,
    pub track_peak: f64,
}

pub fn db_to_linear(db: f64) -> f64 {
    10_f64.powf(db / 20.0)
}

pub fn linear_to_db(lin: f64) -> f64 {
    20.0 * lin.log10()
}

pub fn apply_clipping_prevention(gain_db: f64, preamp_db: f64, peak: f64, prevent: bool) -> f64 {
    let mut effective = gain_db + preamp_db;
    if prevent {
        let lin = db_to_linear(effective);
        let projected_peak = peak * lin;
        if projected_peak > 1.0 {
            effective += linear_to_db(1.0 / projected_peak);
        }
    }
    effective
}

pub fn vlc_volume(user_percent: u8, effective_db: f64) -> i32 {
    let base = f64::from(user_percent.clamp(0, 100)) / 100.0;
    let boosted = (base * db_to_linear(effective_db)).clamp(0.0, 2.0);
    (boosted * 256.0).round() as i32
}

pub fn decode_pcm_ffmpeg(path: &str) -> anyhow::Result<std::process::Child> {
    let child = Command::new("ffmpeg")
        .args([
            "-i", path, "-f", "f32le", "-ac", "2", "-ar", "44100", "pipe:1",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(child)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_linear_roundtrip() {
        let db = -6.0;
        let lin = db_to_linear(db);
        assert!((linear_to_db(lin) - db).abs() < 1e-9);
    }

    #[test]
    fn clipping_prevention_reduces_gain() {
        let effective = apply_clipping_prevention(6.0, 0.0, 1.5, true);
        assert!(effective < 6.0);
    }

    #[test]
    fn vlc_volume_capped() {
        assert_eq!(vlc_volume(100, 20.0), 512);
    }
}
