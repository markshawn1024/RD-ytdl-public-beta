//! JSON file persistence for queue history and user settings.
//! No database — plain files under app config / data dirs.

use crate::config::{self, AppConfig};
use crate::error::{AppError, AppResult};
use crate::queue::DownloadTask;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QueueStore {
    pub tasks: Vec<DownloadTask>,
    /// Whether the queue was paused when last saved.
    #[serde(default)]
    pub paused: bool,
}

fn queue_path(app: &AppHandle) -> AppResult<PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Message(e.to_string()))?;
    fs::create_dir_all(&dir)?;
    Ok(dir.join("queue.json"))
}

pub fn load_queue(app: &AppHandle) -> QueueStore {
    let path = match queue_path(app) {
        Ok(p) => p,
        Err(_) => return QueueStore::default(),
    };
    match fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => QueueStore::default(),
    }
}

pub fn save_queue(app: &AppHandle, store: &QueueStore) -> AppResult<()> {
    let path = queue_path(app)?;
    let raw = serde_json::to_string_pretty(store)?;
    fs::write(path, raw)?;
    Ok(())
}

pub fn save_settings(app: &AppHandle, settings: &AppConfig) -> AppResult<()> {
    config::save_config(app, settings)
}

/// Absolute paths used by this module (for debugging / UI).
pub fn storage_paths(app: &AppHandle) -> AppResult<StoragePaths> {
    let settings = app
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Message(e.to_string()))?
        .join("settings.json");
    let queue = queue_path(app)?;
    Ok(StoragePaths {
        settings: settings.to_string_lossy().to_string(),
        queue: queue.to_string_lossy().to_string(),
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoragePaths {
    pub settings: String,
    pub queue: String,
}
