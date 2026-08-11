//! In-memory download queue on top of the MVP downloader.
//! Sequential execution; pause holds waiting tasks; cancel reuses ActiveDownload.
//! Persistence via `storage` (JSON) — restore waiting / mark downloading as interrupted.

use crate::downloader::{self, DownloadLog};
use crate::error::{AppError, AppResult};
use crate::storage::{self, QueueStore};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

pub const QUEUE_EVENT: &str = "queue://updated";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum QueueTaskStatus {
    Waiting,
    Downloading,
    Completed,
    Failed,
    Cancelled,
    /// Was downloading when the app quit — not auto-retried.
    Interrupted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTask {
    pub id: Uuid,
    pub url: String,
    pub title: Option<String>,
    pub status: QueueTaskStatus,
    pub progress: f64,
    pub speed: Option<String>,
    pub eta: Option<String>,
    pub created_at: DateTime<Utc>,
    pub output_path: Option<String>,
    pub error: Option<String>,
}

impl DownloadTask {
    fn new(url: String) -> Self {
        let title = guess_title(&url);
        Self {
            id: Uuid::new_v4(),
            url,
            title: Some(title),
            status: QueueTaskStatus::Waiting,
            progress: 0.0,
            speed: None,
            eta: None,
            created_at: Utc::now(),
            output_path: None,
            error: None,
        }
    }
}

fn guess_title(url: &str) -> String {
    url.trim()
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(url)
        .chars()
        .take(80)
        .collect()
}

pub struct QueueManager {
    app: AppHandle,
    tasks: RwLock<Vec<DownloadTask>>,
    paused: AtomicBool,
    running: AtomicBool,
    current_id: RwLock<Option<Uuid>>,
}

impl QueueManager {
    pub fn new(app: AppHandle) -> Self {
        let mgr = Self {
            app,
            tasks: RwLock::new(Vec::new()),
            paused: AtomicBool::new(false),
            running: AtomicBool::new(false),
            current_id: RwLock::new(None),
        };
        mgr.restore_from_disk();
        mgr
    }

    /// Load queue.json: waiting kept; downloading → interrupted; no auto-download.
    fn restore_from_disk(&self) {
        let store = storage::load_queue(&self.app);
        let mut restored = Vec::new();

        for mut task in store.tasks {
            match task.status {
                QueueTaskStatus::Waiting => {
                    restored.push(task);
                }
                QueueTaskStatus::Downloading => {
                    task.status = QueueTaskStatus::Interrupted;
                    task.error = Some("应用退出时中断，未自动重试".into());
                    task.speed = None;
                    task.eta = None;
                    restored.push(task);
                }
                QueueTaskStatus::Completed
                | QueueTaskStatus::Failed
                | QueueTaskStatus::Cancelled
                | QueueTaskStatus::Interrupted => {
                    restored.push(task);
                }
            }
        }

        if restored.len() > 200 {
            let drop_count = restored.len() - 200;
            restored.drain(0..drop_count);
        }

        let has_waiting = restored
            .iter()
            .any(|t| t.status == QueueTaskStatus::Waiting);

        *self.tasks.write() = restored;

        // Never auto-start after restore.
        if has_waiting || store.paused {
            self.paused.store(true, Ordering::SeqCst);
        }

        self.persist();
        self.emit_all_no_persist();
    }

    fn persist(&self) {
        let store = QueueStore {
            tasks: self.list(),
            paused: self.is_paused(),
        };
        let _ = storage::save_queue(&self.app, &store);
    }

    pub fn list(&self) -> Vec<DownloadTask> {
        self.tasks.read().clone()
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    pub fn add(&self, url: String) -> AppResult<DownloadTask> {
        let trimmed = url.trim().to_string();
        if trimmed.is_empty()
            || !(trimmed.to_ascii_lowercase().starts_with("http://")
                || trimmed.to_ascii_lowercase().starts_with("https://"))
        {
            return Err(AppError::InvalidUrl);
        }

        let task = DownloadTask::new(trimmed);
        self.tasks.write().push(task.clone());
        self.emit_all();
        self.schedule();
        Ok(task)
    }

    pub fn remove_waiting(&self, id: Uuid) -> AppResult<DownloadTask> {
        let mut tasks = self.tasks.write();
        let idx = tasks
            .iter()
            .position(|t| t.id == id)
            .ok_or(AppError::TaskNotFound)?;
        if tasks[idx].status != QueueTaskStatus::Waiting {
            return Err(AppError::Message("只能删除等待中的任务".into()));
        }
        let removed = tasks.remove(idx);
        drop(tasks);
        self.emit_all();
        Ok(removed)
    }

    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
        self.emit_all();
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
        self.emit_all();
        self.schedule();
    }

    pub async fn cancel_current(&self) -> AppResult<()> {
        let current = *self.current_id.read();
        if current.is_none() {
            return Err(AppError::Message("当前没有下载中的任务".into()));
        }

        downloader::cancel_download(self.app.clone()).await?;

        if let Some(id) = current {
            self.update_task(id, |t| {
                t.status = QueueTaskStatus::Cancelled;
                t.error = Some("用户取消".into());
                t.speed = None;
                t.eta = None;
            });
        }
        self.emit_all();
        Ok(())
    }

    pub fn apply_log(&self, log: &DownloadLog) {
        let Some(id) = *self.current_id.read() else {
            return;
        };

        match log.status.as_str() {
            "downloading" | "checking" | "merging" => {
                self.update_task(id, |t| {
                    t.status = QueueTaskStatus::Downloading;
                    if let Some(p) = log.percent {
                        t.progress = p;
                    }
                    if log.speed.is_some() {
                        t.speed = log.speed.clone();
                    }
                    if log.eta.is_some() {
                        t.eta = log.eta.clone();
                    }
                });
            }
            "completed" => {
                self.update_task(id, |t| {
                    t.status = QueueTaskStatus::Completed;
                    t.progress = 100.0;
                    t.speed = None;
                    t.eta = None;
                    if log.message.contains("已保存:") {
                        if let Some(path) = log.message.lines().find_map(|l| {
                            l.strip_prefix("已保存:")
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                        }) {
                            t.output_path = Some(path);
                        }
                    }
                });
            }
            "failed" => {
                self.update_task(id, |t| {
                    t.status = QueueTaskStatus::Failed;
                    t.error = Some(log.message.clone());
                    t.speed = None;
                    t.eta = None;
                });
            }
            "cancelled" => {
                self.update_task(id, |t| {
                    if t.status == QueueTaskStatus::Downloading {
                        t.status = QueueTaskStatus::Cancelled;
                        t.error = Some("用户取消".into());
                        t.speed = None;
                        t.eta = None;
                    }
                });
            }
            _ => {}
        }
        self.emit_all();
    }

    fn update_task<F>(&self, id: Uuid, f: F)
    where
        F: FnOnce(&mut DownloadTask),
    {
        let mut tasks = self.tasks.write();
        if let Some(task) = tasks.iter_mut().find(|t| t.id == id) {
            f(task);
        }
    }

    fn emit_all(&self) {
        self.persist();
        self.emit_all_no_persist();
    }

    fn emit_all_no_persist(&self) {
        let payload = QueueSnapshot {
            paused: self.is_paused(),
            tasks: self.list(),
            current_id: *self.current_id.read(),
        };
        let _ = self.app.emit(QUEUE_EVENT, &payload);
    }

    pub fn snapshot(&self) -> QueueSnapshot {
        QueueSnapshot {
            paused: self.is_paused(),
            tasks: self.list(),
            current_id: *self.current_id.read(),
        }
    }

    pub fn schedule(&self) {
        if self.paused.load(Ordering::SeqCst) {
            return;
        }
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        let next_id = {
            let tasks = self.tasks.read();
            tasks
                .iter()
                .find(|t| t.status == QueueTaskStatus::Waiting)
                .map(|t| t.id)
        };

        let Some(id) = next_id else {
            self.running.store(false, Ordering::SeqCst);
            return;
        };

        let url = {
            let mut tasks = self.tasks.write();
            let task = tasks.iter_mut().find(|t| t.id == id);
            match task {
                Some(t) => {
                    t.status = QueueTaskStatus::Downloading;
                    t.progress = 0.0;
                    t.error = None;
                    t.url.clone()
                }
                None => {
                    self.running.store(false, Ordering::SeqCst);
                    return;
                }
            }
        };

        *self.current_id.write() = Some(id);
        self.emit_all();

        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let result = downloader::download_video(app.clone(), url).await;
            let queue = app.state::<Arc<QueueManager>>();

            match result {
                Ok(path) => {
                    queue.update_task(id, |t| {
                        t.status = QueueTaskStatus::Completed;
                        t.progress = 100.0;
                        t.output_path = Some(path);
                        t.speed = None;
                        t.eta = None;
                        t.error = None;
                    });
                }
                Err(AppError::Cancelled) => {
                    queue.update_task(id, |t| {
                        if t.status != QueueTaskStatus::Cancelled {
                            t.status = QueueTaskStatus::Cancelled;
                            t.error = Some("用户取消".into());
                        }
                        t.speed = None;
                        t.eta = None;
                    });
                }
                Err(e) => {
                    queue.update_task(id, |t| {
                        t.status = QueueTaskStatus::Failed;
                        t.error = Some(e.to_string());
                        t.speed = None;
                        t.eta = None;
                    });
                }
            }

            *queue.current_id.write() = None;
            queue.running.store(false, Ordering::SeqCst);
            queue.emit_all();
            queue.schedule();
        });
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueSnapshot {
    pub paused: bool,
    pub tasks: Vec<DownloadTask>,
    pub current_id: Option<Uuid>,
}
