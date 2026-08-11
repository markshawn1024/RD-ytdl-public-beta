use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub download_dir: String,
    /// yt-dlp format selector, e.g. `bv*+ba/b`
    pub default_format: String,
    /// Display / preference label, e.g. `best` | `1080p` | `720p`
    pub default_quality: String,
    pub concurrency: u32,
    pub proxy: Option<String>,
    pub download_subtitles: bool,
    pub language: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            download_dir: default_download_dir(),
            default_format: "bv*+ba/b".into(),
            default_quality: "best".into(),
            concurrency: 2,
            proxy: None,
            download_subtitles: false,
            language: "zh-CN".into(),
        }
    }
}

fn default_download_dir() -> String {
    dirs::download_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("RD Media Toolkit")
        .to_string_lossy()
        .to_string()
}

fn settings_path(app: &AppHandle) -> AppResult<PathBuf> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Message(e.to_string()))?;
    fs::create_dir_all(&dir)?;
    Ok(dir.join("settings.json"))
}

pub fn history_path(app: &AppHandle) -> AppResult<PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Message(e.to_string()))?;
    fs::create_dir_all(&dir)?;
    Ok(dir.join("history.json"))
}

pub fn load_config(app: &AppHandle) -> AppConfig {
    let path = match settings_path(app) {
        Ok(p) => p,
        Err(_) => return AppConfig::default(),
    };
    match fs::read_to_string(&path) {
        Ok(raw) => {
            // Merge with defaults so new fields appear after upgrades.
            let mut cfg = AppConfig::default();
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Ok(parsed) = serde_json::from_value::<AppConfig>(value.clone()) {
                    cfg = parsed;
                } else {
                    // Partial legacy file: pick known keys.
                    if let Some(dir) = value.get("downloadDir").and_then(|v| v.as_str()) {
                        cfg.download_dir = dir.to_string();
                    }
                    if let Some(n) = value.get("concurrency").and_then(|v| v.as_u64()) {
                        cfg.concurrency = n as u32;
                    }
                    if let Some(p) = value.get("proxy").and_then(|v| v.as_str()) {
                        cfg.proxy = Some(p.to_string());
                    }
                }
            }
            cfg
        }
        Err(_) => {
            let cfg = AppConfig::default();
            let _ = save_config(app, &cfg);
            cfg
        }
    }
}

pub fn save_config(app: &AppHandle, config: &AppConfig) -> AppResult<()> {
    let path = settings_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(config)?;
    fs::write(path, raw)?;
    ensure_download_dir(Path::new(&config.download_dir))?;
    Ok(())
}

pub fn ensure_download_dir(path: &Path) -> AppResult<()> {
    fs::create_dir_all(path)?;
    Ok(())
}
