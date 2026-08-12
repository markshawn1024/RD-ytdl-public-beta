mod binaries;
mod commands;
mod config;
mod download;
mod downloader;
mod error;
mod models;
mod queue;
mod storage;
mod video_info;

use config::AppConfig;
use download::TaskManager;
use downloader::ActiveDownload;
use parking_lot::RwLock;
use queue::QueueManager;
use std::sync::Arc;
use tauri::Manager;

pub struct AppState {
    pub task_manager: Arc<TaskManager>,
    pub config: Arc<RwLock<AppConfig>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config = Arc::new(RwLock::new(config::load_config(app.handle())));
            let task_manager = Arc::new(TaskManager::new(app.handle().clone(), config.clone()));
            app.manage(AppState {
                task_manager,
                config,
            });
            app.manage(Arc::new(ActiveDownload::new()));
            app.manage(Arc::new(QueueManager::new(app.handle().clone())));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::download_video,
            commands::cancel_active_download,
            commands::open_download_folder,
            commands::check_sidecars,
            commands::get_video_info,
            commands::queue_add,
            commands::queue_list,
            commands::queue_remove_waiting,
            commands::queue_pause,
            commands::queue_resume,
            commands::queue_cancel_current,
            commands::probe_url,
            commands::start_download,
            commands::cancel_download,
            commands::list_downloads,
            commands::clear_finished,
            commands::get_settings,
            commands::update_settings,
            commands::get_storage_paths,
            commands::select_directory,
            commands::open_path,
            commands::get_binary_versions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
