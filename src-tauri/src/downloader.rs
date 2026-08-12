//! Sidecar management + single-video download loop (MVP).
//!
//! Emits live progress via Tauri events (`download://log`).
//! Unified statuses: idle | checking | downloading | merging | completed | failed | cancelled

use crate::binaries::{self, BinaryPaths};
use crate::config;
use crate::download::progress;
use crate::error::{AppError, AppResult};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

pub const LOG_EVENT: &str = "download://log";

/// Shared handle for the in-flight yt-dlp process (cancel support).
pub struct ActiveDownload {
    cancel: AtomicBool,
    child: Mutex<Option<Child>>,
}

impl ActiveDownload {
    pub fn new() -> Self {
        Self {
            cancel: AtomicBool::new(false),
            child: Mutex::new(None),
        }
    }

    async fn reset_for_start(&self) {
        self.cancel.store(false, Ordering::SeqCst);
        let _ = self.child.lock().await.take();
    }

    fn request_cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    async fn set_child(&self, child: Child) {
        *self.child.lock().await = Some(child);
    }

    async fn take_child(&self) -> Option<Child> {
        self.child.lock().await.take()
    }
}

impl Default for ActiveDownload {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadLog {
    /// idle | checking | downloading | merging | completed | failed | cancelled
    pub status: String,
    pub message: String,
    pub percent: Option<f64>,
    pub speed: Option<String>,
    pub eta: Option<String>,
}

fn emit_log(app: &AppHandle, log: DownloadLog) {
    let _ = app.emit(LOG_EVENT, &log);
    if let Some(queue) = app.try_state::<Arc<crate::queue::QueueManager>>() {
        queue.apply_log(&log);
    }
}

/// Resolve and verify yt-dlp / ffmpeg sidecars; return their paths.
pub fn ensure_sidecars(app: &AppHandle) -> AppResult<BinaryPaths> {
    let bins = binaries::resolve(app)?;
    if !bins.yt_dlp.is_file() {
        return Err(AppError::BinaryMissing(format!(
            "yt-dlp 不存在: {}",
            bins.yt_dlp.display()
        )));
    }
    if !bins.ffmpeg.is_file() {
        return Err(AppError::BinaryMissing(format!(
            "ffmpeg 不存在: {}",
            bins.ffmpeg.display()
        )));
    }
    Ok(bins)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SidecarVersions {
    pub yt_dlp: String,
    pub ffmpeg: String,
    pub yt_dlp_path: String,
    pub ffmpeg_path: String,
}

/// Check sidecars exist and report versions (`yt-dlp --version`, `ffmpeg -version`).
pub async fn check_sidecars(app: &AppHandle) -> AppResult<SidecarVersions> {
    emit_log(
        app,
        DownloadLog {
            status: "checking".into(),
            message: "正在检查 sidecar...".into(),
            percent: None,
            speed: None,
            eta: None,
        },
    );

    let bins = ensure_sidecars(app).map_err(|e| {
        emit_log(
            app,
            DownloadLog {
                status: "failed".into(),
                message: e.to_string(),
                percent: None,
                speed: None,
                eta: None,
            },
        );
        e
    })?;

    let yt = binaries::read_version(&bins.yt_dlp)
        .await
        .ok_or_else(|| {
            let err = AppError::BinaryMissing("yt-dlp 无法执行 --version".into());
            emit_log(
                app,
                DownloadLog {
                    status: "failed".into(),
                    message: err.to_string(),
                    percent: None,
                    speed: None,
                    eta: None,
                },
            );
            err
        })?;

    let ff = binaries::read_version(&bins.ffmpeg)
        .await
        .ok_or_else(|| {
            let err = AppError::BinaryMissing("ffmpeg 无法执行 -version".into());
            emit_log(
                app,
                DownloadLog {
                    status: "failed".into(),
                    message: err.to_string(),
                    percent: None,
                    speed: None,
                    eta: None,
                },
            );
            err
        })?;

    Ok(SidecarVersions {
        yt_dlp: yt,
        ffmpeg: ff,
        yt_dlp_path: bins.yt_dlp.to_string_lossy().to_string(),
        ffmpeg_path: bins.ffmpeg.to_string_lossy().to_string(),
    })
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

pub fn default_download_dir(app: &AppHandle) -> PathBuf {
    let cfg = config::load_config(app);
    PathBuf::from(cfg.download_dir)
}

/// Absolute path to the bundled ffmpeg binary for `--ffmpeg-location`.
/// Never rely on PATH; strip Windows `\\?\` prefix which can break yt-dlp.
fn bundled_ffmpeg_location(ffmpeg: &Path) -> AppResult<PathBuf> {
    if !ffmpeg.is_file() {
        return Err(AppError::BinaryMissing(format!(
            "bundled ffmpeg 不存在: {}",
            ffmpeg.display()
        )));
    }

    let abs = std::fs::canonicalize(ffmpeg).unwrap_or_else(|_| ffmpeg.to_path_buf());
    Ok(normalize_path_for_yt_dlp(abs))
}

fn normalize_path_for_yt_dlp(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let s = path.to_string_lossy();
        if let Some(stripped) = s.strip_prefix(r"\\?\") {
            return PathBuf::from(stripped);
        }
    }
    path
}

fn is_ffmpeg_missing_error(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("ffmpeg not found")
        || (lower.contains("ffmpeg-location") && lower.contains("does not exist"))
        || (lower.contains("merging of multiple formats") && lower.contains("ffmpeg"))
        || lower.contains("please install or provide the path using --ffmpeg-location")
}

fn classify_line(line: &str) -> DownloadLog {
    let parsed = progress::parse_line(line);
    let lower = line.to_ascii_lowercase();

    if is_ffmpeg_missing_error(&lower) {
        return DownloadLog {
            status: "failed".into(),
            message: format!("找不到 bundled ffmpeg，无法合并音视频: {line}"),
            percent: None,
            speed: None,
            eta: None,
        };
    }

    if lower.contains("[merger]")
        || lower.contains("merging formats")
        || lower.contains("combining")
    {
        let done = lower.contains("merged")
            || lower.contains("deleting original")
            || lower.contains("has already been downloaded");
        return DownloadLog {
            status: "merging".into(),
            message: if done {
                format!("合并成功: {line}")
            } else if lower.contains("merging formats into") || lower.contains("[merger]") {
                format!("开始 ffmpeg 合并: {line}")
            } else {
                line.to_string()
            },
            percent: parsed.percent.or(Some(99.0)),
            speed: None,
            eta: None,
        };
    }

    // yt-dlp destination lines for separate streams: title [id].fXXXX.ext
    if lower.contains("destination:") || lower.contains("[download] destination") {
        let is_fragment = line.contains(".f")
            && (lower.contains(".mp4")
                || lower.contains(".webm")
                || lower.contains(".m4a")
                || lower.contains(".mkv")
                || lower.contains(".opus"));
        if is_fragment {
            let audio_like = lower.contains(".m4a")
                || lower.contains(".opus")
                || lower.contains(".mp3")
                || lower.contains("audio only")
                || looks_like_audio_fragment(line);
            let message = if audio_like {
                format!("下载音频流: {line}")
            } else {
                format!("下载视频流: {line}")
            };
            return DownloadLog {
                status: "downloading".into(),
                message,
                percent: parsed.percent,
                speed: parsed.speed,
                eta: parsed.eta,
            };
        }
    }

    if lower.contains("downloading") && lower.contains("audio") {
        return DownloadLog {
            status: "downloading".into(),
            message: format!("下载音频流: {line}"),
            percent: parsed.percent,
            speed: parsed.speed,
            eta: parsed.eta,
        };
    }
    if lower.contains("downloading") && lower.contains("video") {
        return DownloadLog {
            status: "downloading".into(),
            message: format!("下载视频流: {line}"),
            percent: parsed.percent,
            speed: parsed.speed,
            eta: parsed.eta,
        };
    }

    if let Some(percent) = parsed.percent {
        return DownloadLog {
            status: "downloading".into(),
            message: format!("{percent:.1}%"),
            percent: Some(percent),
            speed: parsed.speed,
            eta: parsed.eta,
        };
    }

    DownloadLog {
        status: "downloading".into(),
        message: line.to_string(),
        percent: parsed.percent,
        speed: parsed.speed,
        eta: parsed.eta,
    }
}

fn looks_like_audio_fragment(line: &str) -> bool {
    // Heuristic: small-format audio ids often appear as .f14x / .f25x webm/m4a
    let lower = line.to_ascii_lowercase();
    lower.contains("audio")
        || lower.contains(".m4a")
        || lower.contains(".opus")
        || (lower.contains(".webm") && lower.contains(".f14"))
        || (lower.contains(".webm") && lower.contains(".f25"))
}

/// Cancel the active yt-dlp process (if any).
pub async fn cancel_download(app: AppHandle) -> AppResult<()> {
    let active = app.state::<Arc<ActiveDownload>>();
    active.request_cancel();

    if let Some(mut c) = active.take_child().await {
        let _ = c.kill().await;
        let _ = c.wait().await;
    }

    emit_log(
        &app,
        DownloadLog {
            status: "cancelled".into(),
            message: "下载已取消".into(),
            percent: None,
            speed: None,
            eta: None,
        },
    );
    Ok(())
}

/// Open the default download directory in the system file manager.
pub fn open_download_folder(app: AppHandle) -> AppResult<String> {
    let dir = default_download_dir(&app);
    std::fs::create_dir_all(&dir)?;
    let path = dir.to_string_lossy().to_string();

    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(path.clone(), None::<&str>)
        .map_err(|e| AppError::Message(format!("无法打开目录: {e}")))?;

    Ok(path)
}

/// Download a single video with best video+audio, merge to mp4.
/// Streams progress via `download://log`. Returns output path on success.
pub async fn download_video(app: AppHandle, url: String) -> AppResult<String> {
    let active = app.state::<Arc<ActiveDownload>>().inner().clone();

    if let Err(e) = validate_url(&url) {
        emit_log(
            &app,
            DownloadLog {
                status: "failed".into(),
                message: e.to_string(),
                percent: None,
                speed: None,
                eta: None,
            },
        );
        return Err(e);
    }

    active.reset_for_start().await;

    emit_log(
        &app,
        DownloadLog {
            status: "checking".into(),
            message: "检查 sidecar 与下载目录...".into(),
            percent: None,
            speed: None,
            eta: None,
        },
    );

    let bins = match ensure_sidecars(&app) {
        Ok(b) => b,
        Err(e) => {
            emit_log(
                &app,
                DownloadLog {
                    status: "failed".into(),
                    message: e.to_string(),
                    percent: None,
                    speed: None,
                    eta: None,
                },
            );
            return Err(e);
        }
    };

    let out_dir = default_download_dir(&app);
    if let Err(e) = std::fs::create_dir_all(&out_dir) {
        let err = AppError::Io(e);
        emit_log(
            &app,
            DownloadLog {
                status: "failed".into(),
                message: format!("无法创建下载目录: {err}"),
                percent: None,
                speed: None,
                eta: None,
            },
        );
        return Err(err);
    }

    let ffmpeg_location = match bundled_ffmpeg_location(&bins.ffmpeg) {
        Ok(p) => p,
        Err(e) => {
            emit_log(
                &app,
                DownloadLog {
                    status: "failed".into(),
                    message: e.to_string(),
                    percent: None,
                    speed: None,
                    eta: None,
                },
            );
            return Err(e);
        }
    };

    // Prove the exact binary yt-dlp will use (never PATH).
    if binaries::read_version(&ffmpeg_location).await.is_none() {
        let err = AppError::BinaryMissing(format!(
            "bundled ffmpeg 无法执行: {}",
            ffmpeg_location.display()
        ));
        emit_log(
            &app,
            DownloadLog {
                status: "failed".into(),
                message: err.to_string(),
                percent: None,
                speed: None,
                eta: None,
            },
        );
        return Err(err);
    }

    emit_log(
        &app,
        DownloadLog {
            status: "downloading".into(),
            message: format!(
                "正在下载...\n目录: {}\nffmpeg: {}",
                out_dir.display(),
                ffmpeg_location.display()
            ),
            percent: Some(0.0),
            speed: None,
            eta: None,
        },
    );

    let output_template = out_dir
        .join("%(title).200B [%(id)s].%(ext)s")
        .to_string_lossy()
        .to_string();

    let mut cmd = Command::new(&bins.yt_dlp);
    // Pass the bundled ffmpeg *binary* path (not PATH, not parent-only guess).
    cmd.arg("--ffmpeg-location")
        .arg(&ffmpeg_location)
        .arg("--newline")
        .arg("--progress")
        .arg("--no-playlist")
        .arg("-f")
        .arg("bv*+ba/b")
        .arg("--merge-output-format")
        .arg("mp4")
        .arg("-o")
        .arg(&output_template)
        .arg("--print")
        .arg("after_move:filepath")
        .arg(url.trim())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    binaries::configure_hidden_process(&mut cmd);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let err = AppError::DownloadFailed(format!("无法启动 yt-dlp: {e}"));
            emit_log(
                &app,
                DownloadLog {
                    status: "failed".into(),
                    message: err.to_string(),
                    percent: None,
                    speed: None,
                    eta: None,
                },
            );
            return Err(err);
        }
    };

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::DownloadFailed("无法读取 stdout".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::DownloadFailed("无法读取 stderr".into()))?;

    // Store child so Cancel can kill it from another command.
    active.set_child(child).await;

    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();
    let mut stdout_done = false;
    let mut stderr_done = false;
    let mut output_path: Option<String> = None;
    let mut err_lines: Vec<String> = Vec::new();

    while !stdout_done || !stderr_done {
        if active.is_cancelled() {
            if let Some(mut c) = active.take_child().await {
                let _ = c.kill().await;
                let _ = c.wait().await;
            }
            return Err(AppError::Cancelled);
        }

        tokio::select! {
            line = stdout_reader.next_line(), if !stdout_done => {
                match line {
                    Ok(Some(text)) => {
                        let trimmed = text.trim().to_string();
                        if trimmed.is_empty() {
                            continue;
                        }
                        if looks_like_filepath(&trimmed) {
                            output_path = Some(trimmed.clone());
                        }
                        if !active.is_cancelled() {
                            emit_log(&app, classify_line(&trimmed));
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
                        if trimmed.is_empty() {
                            continue;
                        }
                        err_lines.push(trimmed.clone());
                        if !active.is_cancelled() {
                            emit_log(&app, classify_line(&trimmed));
                        }
                    }
                    Ok(None) => stderr_done = true,
                    Err(e) => {
                        err_lines.push(e.to_string());
                        stderr_done = true;
                    }
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(150)) => {}
        }
    }

    if active.is_cancelled() {
        let _ = active.take_child().await;
        return Err(AppError::Cancelled);
    }

    let mut child = match active.take_child().await {
        Some(c) => c,
        None => return Err(AppError::Cancelled),
    };

    let status = child
        .wait()
        .await
        .map_err(|e| AppError::DownloadFailed(e.to_string()))?;

    if active.is_cancelled() {
        return Err(AppError::Cancelled);
    }

    if status.success() {
        let path = output_path
            .clone()
            .unwrap_or_else(|| out_dir.to_string_lossy().to_string());

        let final_path = PathBuf::from(&path);
        let removed = cleanup_merge_leftovers(&final_path);
        if !removed.is_empty() {
            emit_log(
                &app,
                DownloadLog {
                    status: "merging".into(),
                    message: format!(
                        "合并成功，已清理临时流文件: {}",
                        removed.join(", ")
                    ),
                    percent: Some(99.5),
                    speed: None,
                    eta: None,
                },
            );
        }

        emit_log(
            &app,
            DownloadLog {
                status: "completed".into(),
                message: format!("合并成功\n已保存: {path}"),
                percent: Some(100.0),
                speed: None,
                eta: None,
            },
        );
        Ok(path)
    } else {
        let detail = err_lines
            .iter()
            .rev()
            .find(|l| !l.is_empty())
            .cloned()
            .unwrap_or_else(|| format!("yt-dlp 退出码: {:?}", status.code()));

        let lower = detail.to_ascii_lowercase();
        let message = if is_ffmpeg_missing_error(&detail) {
            format!(
                "找不到 bundled ffmpeg（路径: {}）: {detail}",
                ffmpeg_location.display()
            )
        } else if lower.contains("unable to download")
            || lower.contains("network")
            || lower.contains("urlopen")
            || lower.contains("timed out")
        {
            format!("网络错误: {detail}")
        } else {
            format!("下载失败: {detail}")
        };

        emit_log(
            &app,
            DownloadLog {
                status: "failed".into(),
                message: message.clone(),
                percent: None,
                speed: None,
                eta: None,
            },
        );
        Err(AppError::DownloadFailed(message))
    }
}

fn looks_like_filepath(line: &str) -> bool {
    if line.starts_with('[') || line.starts_with('{') {
        return false;
    }
    Path::new(line).extension().is_some() && (line.contains('/') || line.contains('\\'))
}

/// Remove yt-dlp intermediate stream files left beside the merged output.
/// Example: `Title [id].f137.mp4` / `Title [id].f251.webm` next to `Title [id].mp4`.
fn cleanup_merge_leftovers(final_path: &Path) -> Vec<String> {
    let mut removed = Vec::new();
    let Some(parent) = final_path.parent() else {
        return removed;
    };
    let Some(stem) = final_path.file_stem().and_then(|s| s.to_str()) else {
        return removed;
    };
    if stem.is_empty() {
        return removed;
    }

    let prefix = format!("{stem}.f");
    let Ok(entries) = std::fs::read_dir(parent) else {
        return removed;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path == final_path {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with(&prefix) {
            if std::fs::remove_file(&path).is_ok() {
                removed.push(name.to_string());
            }
        }
    }
    removed
}
