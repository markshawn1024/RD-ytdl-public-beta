use crate::binaries::BinaryPaths;
use crate::config::AppConfig;
use crate::error::AppResult;
use crate::models::{BinaryVersions, DownloadTask, StartDownloadRequest, VideoInfo};
use crate::AppState;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use uuid::Uuid;

/// MVP: one-shot download with live `download://log` events.
#[tauri::command]
pub async fn download_video(app: AppHandle, url: String) -> Result<String, crate::error::AppError> {
    crate::downloader::download_video(app, url).await
}

/// MVP: kill the active yt-dlp child process.
#[tauri::command]
pub async fn cancel_active_download(app: AppHandle) -> Result<(), crate::error::AppError> {
    crate::downloader::cancel_download(app, "cancel_active_download").await
}

/// MVP: open default download folder in Finder / Explorer.
#[tauri::command]
pub fn open_download_folder(app: AppHandle) -> Result<String, crate::error::AppError> {
    crate::downloader::open_download_folder(app)
}

/// MVP: verify sidecars + versions.
#[tauri::command]
pub async fn check_sidecars(
    app: AppHandle,
) -> Result<crate::downloader::SidecarVersions, crate::error::AppError> {
    crate::downloader::check_sidecars(&app).await
}

/// Probe video metadata before enqueue (`yt-dlp --dump-json`).
#[tauri::command]
pub async fn get_video_info(
    app: AppHandle,
    url: String,
) -> Result<crate::video_info::VideoInfoResult, crate::error::AppError> {
    crate::video_info::get_video_info(&app, url).await
}

// --- Download Queue ---

#[tauri::command]
pub fn queue_add(
    app: AppHandle,
    url: String,
) -> Result<crate::queue::DownloadTask, crate::error::AppError> {
    let queue = app.state::<std::sync::Arc<crate::queue::QueueManager>>();
    queue.add(url)
}

#[tauri::command]
pub fn queue_list(app: AppHandle) -> Result<crate::queue::QueueSnapshot, crate::error::AppError> {
    let queue = app.state::<std::sync::Arc<crate::queue::QueueManager>>();
    Ok(queue.snapshot())
}

#[tauri::command]
pub fn queue_remove_waiting(
    app: AppHandle,
    id: String,
) -> Result<crate::queue::DownloadTask, crate::error::AppError> {
    let uuid = Uuid::parse_str(&id).map_err(|_| crate::error::AppError::TaskNotFound)?;
    let queue = app.state::<std::sync::Arc<crate::queue::QueueManager>>();
    queue.remove_waiting(uuid)
}

#[tauri::command]
pub fn queue_pause(app: AppHandle) -> Result<crate::queue::QueueSnapshot, crate::error::AppError> {
    let queue = app.state::<std::sync::Arc<crate::queue::QueueManager>>();
    queue.pause();
    Ok(queue.snapshot())
}

#[tauri::command]
pub fn queue_resume(app: AppHandle) -> Result<crate::queue::QueueSnapshot, crate::error::AppError> {
    let queue = app.state::<std::sync::Arc<crate::queue::QueueManager>>();
    queue.resume();
    Ok(queue.snapshot())
}

#[tauri::command]
pub async fn queue_cancel_current(app: AppHandle) -> Result<(), crate::error::AppError> {
    let queue = app
        .state::<std::sync::Arc<crate::queue::QueueManager>>()
        .inner()
        .clone();
    queue.cancel_current().await
}

#[tauri::command]
pub async fn probe_url(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
) -> Result<VideoInfo, crate::error::AppError> {
    let bins = state.task_manager.ensure_bins()?;
    let config = state.config.read().clone();
    let cache_dir = crate::binaries::cache_dir(&app)?;
    crate::download::runner::probe_url(&bins, &config, &url, &cache_dir).await
}

#[tauri::command]
pub fn start_download(
    state: State<'_, AppState>,
    request: StartDownloadRequest,
) -> Result<DownloadTask, crate::error::AppError> {
    state.task_manager.start_download(request)
}

#[tauri::command]
pub fn cancel_download(
    state: State<'_, AppState>,
    id: String,
) -> Result<DownloadTask, crate::error::AppError> {
    let uuid = Uuid::parse_str(&id).map_err(|_| crate::error::AppError::TaskNotFound)?;
    state.task_manager.cancel(uuid)
}

#[tauri::command]
pub fn list_downloads(
    state: State<'_, AppState>,
) -> Result<Vec<DownloadTask>, crate::error::AppError> {
    Ok(state.task_manager.list_tasks())
}

#[tauri::command]
pub fn clear_finished(state: State<'_, AppState>) -> Result<(), crate::error::AppError> {
    state.task_manager.clear_finished();
    Ok(())
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<AppConfig, crate::error::AppError> {
    Ok(state.config.read().clone())
}

#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: AppConfig,
) -> Result<AppConfig, crate::error::AppError> {
    let mut next = settings;
    next.concurrency = next.concurrency.clamp(1, 4);
    if next.default_format.trim().is_empty() {
        next.default_format = "bv*+ba/b".into();
    }
    if next.default_quality.trim().is_empty() {
        next.default_quality = "best".into();
    }
    crate::storage::save_settings(&app, &next)?;
    *state.config.write() = next.clone();
    let _ = app.emit("settings://changed", &next);
    state.task_manager.schedule();
    Ok(next)
}

#[tauri::command]
pub fn get_storage_paths(
    app: AppHandle,
) -> Result<crate::storage::StoragePaths, crate::error::AppError> {
    crate::storage::storage_paths(&app)
}

#[tauri::command]
pub async fn select_directory(app: AppHandle) -> Result<Option<String>, crate::error::AppError> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folder(move |folder| {
        let _ = tx.send(folder.map(|p| p.to_string()));
    });
    rx.await
        .map_err(|e| crate::error::AppError::Message(e.to_string()))
}

#[tauri::command]
pub fn open_path(app: AppHandle, path: String) -> Result<(), crate::error::AppError> {
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| crate::error::AppError::Message(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn get_binary_versions(
    state: State<'_, AppState>,
) -> Result<BinaryVersions, crate::error::AppError> {
    let bins = state.task_manager.ensure_bins()?;
    Ok(versions_from_bins(&bins).await)
}

async fn versions_from_bins(bins: &BinaryPaths) -> BinaryVersions {
    BinaryVersions {
        yt_dlp: crate::binaries::read_version(&bins.yt_dlp).await,
        ffmpeg: crate::binaries::read_version(&bins.ffmpeg).await,
        deno: crate::binaries::read_version(&bins.deno).await,
    }
}

#[allow(dead_code)]
fn _ok_result() -> AppResult<()> {
    Ok(())
}
