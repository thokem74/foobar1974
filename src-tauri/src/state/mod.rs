use std::{fs, path::PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::models::{RepeatMode, ReplayGainSettings};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateFile {
    pub library_folders: Vec<String>,
    pub volume: u8,
    pub shuffle: bool,
    pub repeat_mode: RepeatMode,
    pub replaygain: ReplayGainSettings,
}

impl Default for AppStateFile {
    fn default() -> Self {
        Self {
            library_folders: vec![],
            volume: 100,
            shuffle: false,
            repeat_mode: RepeatMode::Off,
            replaygain: ReplayGainSettings::default(),
        }
    }
}

pub fn load(path: &PathBuf) -> anyhow::Result<AppStateFile> {
    if !path.exists() {
        return Ok(AppStateFile::default());
    }
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(serde_json::from_str(&text).unwrap_or_default())
}

pub fn save(path: &PathBuf, state: &AppStateFile) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(state)?;
    fs::write(path, json)?;
    Ok(())
}
