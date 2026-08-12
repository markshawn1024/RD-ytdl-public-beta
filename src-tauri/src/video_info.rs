//! Video metadata probe via yt-dlp `--dump-json`.
//! Independent of downloader / queue / storage.

use crate::binaries;
use crate::config;
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::Stdio;
use tauri::AppHandle;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoFormatInfo {
    pub format_id: String,
    pub ext: Option<String>,
    pub resolution: Option<String>,
    pub fps: Option<f64>,
    pub vcodec: Option<String>,
    pub acodec: Option<String>,
    pub filesize: Option<u64>,
    pub format_note: Option<String>,
    pub tbr: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoInfoResult {
    pub title: String,
    pub thumbnail: Option<String>,
    pub duration: Option<f64>,
    pub uploader: Option<String>,
    pub formats: Vec<VideoFormatInfo>,
    pub webpage_url: Option<String>,
    pub id: Option<String>,
}

fn validate_url(url: &str) -> AppResult<()> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidUrl);
    }
    let lower = trimmed.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err(AppError::InvalidUrl);
    }
    Ok(())
}

/// Fetch video metadata with `yt-dlp --dump-json <url>`.
pub async fn get_video_info(app: &AppHandle, url: String) -> AppResult<VideoInfoResult> {
    validate_url(&url)?;

    let bins = binaries::resolve(app)?;
    let cfg = config::load_config(app);

    let mut cmd = Command::new(&bins.yt_dlp);
    cmd.arg("--dump-json")
        .arg("--no-playlist")
        .arg("--no-warnings")
        .arg(url.trim())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    binaries::configure_hidden_process(&mut cmd);

    if let Some(proxy) = cfg.proxy.as_deref().filter(|s| !s.is_empty()) {
        cmd.arg("--proxy").arg(proxy);
    }

    let output = cmd
        .output()
        .await
        .map_err(|e| AppError::ProbeFailed(format!("无法启动 yt-dlp: {e}")))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let msg = err.trim();
        if msg.is_empty() {
            return Err(AppError::ProbeFailed("解析失败（未知错误）".into()));
        }
        return Err(AppError::ProbeFailed(msg.to_string()));
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    parse_dump_json(&raw)
}

fn parse_dump_json(raw: &str) -> AppResult<VideoInfoResult> {
    let value: Value = serde_json::from_str(raw)
        .map_err(|e| AppError::ProbeFailed(format!("JSON 解析失败: {e}")))?;

    let title = value
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("未命名视频")
        .to_string();

    let mut formats = Vec::new();
    if let Some(arr) = value.get("formats").and_then(|v| v.as_array()) {
        for item in arr {
            let format_id = match item.get("format_id").and_then(|v| v.as_str()) {
                Some(id) => id.to_string(),
                None => continue,
            };
            let height = item.get("height").and_then(|v| v.as_u64());
            let width = item.get("width").and_then(|v| v.as_u64());
            let resolution = match (width, height) {
                (Some(w), Some(h)) => Some(format!("{w}x{h}")),
                (None, Some(h)) => Some(format!("{h}p")),
                _ => item
                    .get("resolution")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            };

            formats.push(VideoFormatInfo {
                format_id,
                ext: item
                    .get("ext")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                resolution,
                fps: item.get("fps").and_then(|v| v.as_f64()),
                vcodec: item
                    .get("vcodec")
                    .and_then(|v| v.as_str())
                    .filter(|s| *s != "none")
                    .map(|s| s.to_string()),
                acodec: item
                    .get("acodec")
                    .and_then(|v| v.as_str())
                    .filter(|s| *s != "none")
                    .map(|s| s.to_string()),
                filesize: item
                    .get("filesize")
                    .or_else(|| item.get("filesize_approx"))
                    .and_then(|v| v.as_u64()),
                format_note: item
                    .get("format_note")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                tbr: item.get("tbr").and_then(|v| v.as_f64()),
            });
        }
    }

    formats.retain(|f| f.vcodec.is_some() || f.acodec.is_some());
    formats.sort_by(|a, b| {
        let ah = height_hint(a.resolution.as_deref());
        let bh = height_hint(b.resolution.as_deref());
        bh.cmp(&ah)
    });

    Ok(VideoInfoResult {
        title,
        thumbnail: value
            .get("thumbnail")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        duration: value.get("duration").and_then(|v| v.as_f64()),
        uploader: value
            .get("uploader")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        formats,
        webpage_url: value
            .get("webpage_url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        id: value
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    })
}

fn height_hint(resolution: Option<&str>) -> u64 {
    resolution
        .and_then(|r| {
            r.split('x')
                .next_back()
                .and_then(|h| h.trim_end_matches('p').parse::<u64>().ok())
        })
        .unwrap_or(0)
}
