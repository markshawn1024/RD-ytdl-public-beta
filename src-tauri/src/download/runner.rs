use crate::binaries::{self, BinaryPaths};
use crate::config::AppConfig;
use crate::error::{AppError, AppResult};
use crate::models::{VideoFormat, VideoInfo};
use serde_json::Value;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

pub async fn probe_url(bins: &BinaryPaths, config: &AppConfig, url: &str) -> AppResult<VideoInfo> {
    validate_url(url)?;

    let mut cmd = Command::new(&bins.yt_dlp);
    cmd.arg("-J")
        .arg("--no-playlist")
        .arg("--no-warnings")
        .arg(url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    binaries::configure_hidden_process(&mut cmd);

    if let Some(proxy) = config.proxy.as_deref().filter(|s| !s.is_empty()) {
        cmd.arg("--proxy").arg(proxy);
    }

    let output = cmd
        .output()
        .await
        .map_err(|e| AppError::ProbeFailed(e.to_string()))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::ProbeFailed(err.trim().to_string()));
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    parse_video_info(&raw)
}

pub fn parse_video_info(raw: &str) -> AppResult<VideoInfo> {
    let value: Value = serde_json::from_str(raw)?;
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

            formats.push(VideoFormat {
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
        let ah = a
            .resolution
            .as_ref()
            .and_then(|r| {
                r.split('x')
                    .next_back()
                    .and_then(|h| h.trim_end_matches('p').parse::<u64>().ok())
            })
            .unwrap_or(0);
        let bh = b
            .resolution
            .as_ref()
            .and_then(|r| {
                r.split('x')
                    .next_back()
                    .and_then(|h| h.trim_end_matches('p').parse::<u64>().ok())
            })
            .unwrap_or(0);
        bh.cmp(&ah)
    });

    Ok(VideoInfo {
        id: value
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
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
        webpage_url: value
            .get("webpage_url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        formats,
    })
}

pub struct DownloadRunResult {
    pub success: bool,
    pub cancelled: bool,
    pub output_path: Option<String>,
    pub error: Option<String>,
}

pub async fn run_download<F>(
    bins: &BinaryPaths,
    config: &AppConfig,
    url: &str,
    format_id: &str,
    output_dir: &Path,
    cancelled: Arc<AtomicBool>,
    mut on_line: F,
) -> AppResult<DownloadRunResult>
where
    F: FnMut(String),
{
    validate_url(url)?;
    std::fs::create_dir_all(output_dir)?;

    let mut ffmpeg_location = bins
        .ffmpeg
        .canonicalize()
        .unwrap_or_else(|_| bins.ffmpeg.clone());
    #[cfg(windows)]
    {
        let s = ffmpeg_location.to_string_lossy();
        if let Some(stripped) = s.strip_prefix(r"\\?\") {
            ffmpeg_location = std::path::PathBuf::from(stripped);
        }
    }

    let output_template = output_dir
        .join("%(title).200B [%(id)s].%(ext)s")
        .to_string_lossy()
        .to_string();

    let mut cmd = Command::new(&bins.yt_dlp);
    cmd.arg("--ffmpeg-location")
        .arg(&ffmpeg_location)
        .arg("--newline")
        .arg("--progress")
        .arg("--no-playlist")
        .arg("-f")
        .arg(format_id)
        .arg("--merge-output-format")
        .arg("mp4")
        .arg("-o")
        .arg(&output_template)
        .arg("--print")
        .arg("after_move:filepath")
        .arg(url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    binaries::configure_hidden_process(&mut cmd);

    if let Some(proxy) = config.proxy.as_deref().filter(|s| !s.is_empty()) {
        cmd.arg("--proxy").arg(proxy);
    }

    if config.download_subtitles {
        cmd.arg("--write-subs").arg("--sub-langs").arg("zh.*,en.*");
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| AppError::DownloadFailed(e.to_string()))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::DownloadFailed("无法读取 stdout".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::DownloadFailed("无法读取 stderr".into()))?;

    let mut output_path: Option<String> = None;
    let mut err_lines: Vec<String> = Vec::new();

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();

    let mut stdout_done = false;
    let mut stderr_done = false;

    while !stdout_done || !stderr_done {
        if cancelled.load(Ordering::SeqCst) {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Ok(DownloadRunResult {
                success: false,
                cancelled: true,
                output_path: None,
                error: Some("用户取消".into()),
            });
        }

        tokio::select! {
            line = stdout_reader.next_line(), if !stdout_done => {
                match line {
                    Ok(Some(text)) => {
                        let trimmed = text.trim().to_string();
                        if !trimmed.is_empty() {
                            if looks_like_filepath(&trimmed) {
                                output_path = Some(trimmed.clone());
                            }
                            on_line(trimmed);
                        }
                    }
                    Ok(None) => stdout_done = true,
                    Err(e) => {
                        err_lines.push(e.to_string());
                        stdout_done = true;
                    }
                }
            }
            line = stderr_reader.next_line(), if !stderr_done => {
                match line {
                    Ok(Some(text)) => {
                        let trimmed = text.trim().to_string();
                        if !trimmed.is_empty() {
                            err_lines.push(trimmed.clone());
                            on_line(trimmed);
                        }
                    }
                    Ok(None) => stderr_done = true,
                    Err(e) => {
                        err_lines.push(e.to_string());
                        stderr_done = true;
                    }
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(200)) => {}
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|e| AppError::DownloadFailed(e.to_string()))?;

    if cancelled.load(Ordering::SeqCst) {
        return Ok(DownloadRunResult {
            success: false,
            cancelled: true,
            output_path: None,
            error: Some("用户取消".into()),
        });
    }

    if status.success() {
        Ok(DownloadRunResult {
            success: true,
            cancelled: false,
            output_path,
            error: None,
        })
    } else {
        Ok(DownloadRunResult {
            success: false,
            cancelled: false,
            output_path: None,
            error: Some(
                err_lines
                    .last()
                    .cloned()
                    .unwrap_or_else(|| format!("yt-dlp 退出码: {:?}", status.code())),
            ),
        })
    }
}

fn looks_like_filepath(line: &str) -> bool {
    if line.starts_with('[') || line.starts_with('{') {
        return false;
    }
    Path::new(line).extension().is_some() && (line.contains('/') || line.contains('\\'))
}

pub fn validate_url(url: &str) -> AppResult<()> {
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
