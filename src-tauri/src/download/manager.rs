use crate::binaries::{self, BinaryPaths};
use crate::config::{self, AppConfig};
use crate::download::progress::{self, ParsedProgress};
use crate::download::runner;
use crate::error::{AppError, AppResult};
use crate::models::{DownloadTask, LogEvent, ProgressEvent, StartDownloadRequest, TaskStatus};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

pub struct TaskManager {
    app: AppHandle,
    tasks: RwLock<HashMap<Uuid, DownloadTask>>,
    cancel_flags: RwLock<HashMap<Uuid, Arc<AtomicBool>>>,
    active: RwLock<u32>,
    bins: RwLock<Option<BinaryPaths>>,
    config: Arc<RwLock<AppConfig>>,
}

impl TaskManager {
    pub fn new(app: AppHandle, config: Arc<RwLock<AppConfig>>) -> Self {
        let mgr = Self {
            app,
            tasks: RwLock::new(HashMap::new()),
            cancel_flags: RwLock::new(HashMap::new()),
            active: RwLock::new(0),
            bins: RwLock::new(None),
            config,
        };
        if let Ok(history) = load_history(&mgr.app) {
            let mut tasks = mgr.tasks.write();
            for task in history {
                tasks.insert(task.id, task);
            }
        }
        mgr
    }

    pub fn ensure_bins(&self) -> AppResult<BinaryPaths> {
        if let Some(bins) = self.bins.read().clone() {
            return Ok(bins);
        }
        let bins = binaries::resolve(&self.app)?;
        *self.bins.write() = Some(bins.clone());
        Ok(bins)
    }

    pub fn list_tasks(&self) -> Vec<DownloadTask> {
        let mut list: Vec<_> = self.tasks.read().values().cloned().collect();
        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        list
    }

    pub fn start_download(&self, req: StartDownloadRequest) -> AppResult<DownloadTask> {
        runner::validate_url(&req.url)?;
        let _ = self.ensure_bins()?;

        let cfg = self.config.read().clone();
        let output_dir = req
            .output_dir
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| cfg.download_dir.clone());
        config::ensure_download_dir(PathBuf::from(&output_dir).as_path())?;

        let format_id = req
            .format_id
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "bv*+ba/b".into());

        let mut task = DownloadTask::new(req.url, format_id, output_dir);
        task.title = req.title;
        task.thumbnail = req.thumbnail;

        let flag = Arc::new(AtomicBool::new(false));
        self.cancel_flags.write().insert(task.id, flag);
        self.upsert(task.clone());
        self.schedule();
        Ok(task)
    }

    pub fn cancel(&self, id: Uuid) -> AppResult<DownloadTask> {
        if let Some(flag) = self.cancel_flags.read().get(&id) {
            flag.store(true, Ordering::SeqCst);
        }

        let mut tasks = self.tasks.write();
        let task = tasks.get_mut(&id).ok_or(AppError::TaskNotFound)?;
        match task.status {
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled => {}
            TaskStatus::Running => {
                task.status = TaskStatus::Cancelled;
                task.error = Some("用户取消".into());
                task.touch();
            }
            _ => {
                task.status = TaskStatus::Cancelled;
                task.error = Some("用户取消".into());
                task.touch();
            }
        }
        let cloned = task.clone();
        drop(tasks);
        self.emit_task(&cloned);
        self.persist_history();
        self.schedule();
        Ok(cloned)
    }

    pub fn clear_finished(&self) {
        self.tasks.write().retain(|_, t| {
            !matches!(
                t.status,
                TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
            )
        });
        self.persist_history();
    }

    fn upsert(&self, task: DownloadTask) {
        self.emit_task(&task);
        self.tasks.write().insert(task.id, task);
        self.persist_history();
    }

    fn update_task<F>(&self, id: Uuid, mutator: F) -> Option<DownloadTask>
    where
        F: FnOnce(&mut DownloadTask),
    {
        let mut tasks = self.tasks.write();
        let task = tasks.get_mut(&id)?;
        mutator(task);
        task.touch();
        let cloned = task.clone();
        drop(tasks);
        self.emit_task(&cloned);
        Some(cloned)
    }

    fn emit_task(&self, task: &DownloadTask) {
        let _ = self.app.emit("download://task-upsert", task);
    }

    fn emit_progress(&self, event: ProgressEvent) {
        let _ = self.app.emit("download://progress", &event);
    }

    fn emit_log(&self, id: Uuid, line: String) {
        let _ = self.app.emit(
            "download://log",
            LogEvent {
                id,
                line: line.clone(),
            },
        );
        self.update_task(id, |t| t.push_log(line));
    }

    pub fn schedule(&self) {
        let concurrency = self.config.read().concurrency.max(1);
        let active = *self.active.read();
        if active >= concurrency {
            return;
        }

        let next_id = {
            let tasks = self.tasks.read();
            tasks
                .values()
                .filter(|t| t.status == TaskStatus::Queued)
                .min_by_key(|t| t.created_at)
                .map(|t| t.id)
        };

        let Some(id) = next_id else {
            return;
        };

        {
            let mut active = self.active.write();
            if *active >= concurrency {
                return;
            }
            *active += 1;
        }

        let cancelled = self
            .cancel_flags
            .read()
            .get(&id)
            .cloned()
            .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));

        if cancelled.load(Ordering::SeqCst) {
            *self.active.write() -= 1;
            let _ = self.cancel(id);
            return;
        }

        self.update_task(id, |t| {
            t.status = TaskStatus::Running;
            t.progress = 0.0;
            t.error = None;
        });

        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let state = app.state::<crate::AppState>();
            state.task_manager.run_task(id, cancelled).await;
            {
                let mut active = state.task_manager.active.write();
                *active = active.saturating_sub(1);
            }
            state.task_manager.schedule();
        });
    }

    async fn run_task(&self, id: Uuid, cancelled: Arc<AtomicBool>) {
        let bins = match self.ensure_bins() {
            Ok(b) => b,
            Err(e) => {
                self.update_task(id, |t| {
                    t.status = TaskStatus::Failed;
                    t.error = Some(e.to_string());
                });
                self.persist_history();
                return;
            }
        };

        let (url, format_id, output_dir) = {
            let tasks = self.tasks.read();
            match tasks.get(&id) {
                Some(t) => (t.url.clone(), t.format_id.clone(), t.output_dir.clone()),
                None => return,
            }
        };

        if cancelled.load(Ordering::SeqCst) {
            self.update_task(id, |t| {
                t.status = TaskStatus::Cancelled;
                t.error = Some("用户取消".into());
            });
            self.persist_history();
            return;
        }

        let cfg = self.config.read().clone();
        let mgr_app = self.app.clone();

        let result = runner::run_download(
            &bins,
            &cfg,
            &url,
            &format_id,
            PathBuf::from(&output_dir).as_path(),
            cancelled.clone(),
            |line| {
                if cancelled.load(Ordering::SeqCst) {
                    return;
                }
                let state = mgr_app.state::<crate::AppState>();
                let parsed = progress::parse_line(&line);
                state.task_manager.apply_progress(id, &parsed);
                state.task_manager.emit_log(id, line);
            },
        )
        .await;

        match result {
            Ok(res) if res.cancelled => {
                self.update_task(id, |t| {
                    t.status = TaskStatus::Cancelled;
                    t.error = Some("用户取消".into());
                });
            }
            Ok(res) if res.success => {
                self.update_task(id, |t| {
                    t.status = TaskStatus::Completed;
                    t.progress = 100.0;
                    t.output_path = res.output_path;
                    t.speed = None;
                    t.eta = None;
                });
            }
            Ok(res) => {
                self.update_task(id, |t| {
                    t.status = TaskStatus::Failed;
                    t.error = res.error.or_else(|| Some("下载失败".into()));
                });
            }
            Err(e) => {
                self.update_task(id, |t| {
                    t.status = TaskStatus::Failed;
                    t.error = Some(e.to_string());
                });
            }
        }
        self.persist_history();
    }

    fn apply_progress(&self, id: Uuid, parsed: &ParsedProgress) {
        if let Some(event) = progress::to_event(id, parsed) {
            self.emit_progress(event.clone());
            self.update_task(id, |t| {
                if let Some(p) = parsed.percent {
                    t.progress = p;
                }
                if parsed.speed.is_some() {
                    t.speed = parsed.speed.clone();
                }
                if parsed.eta.is_some() {
                    t.eta = parsed.eta.clone();
                }
                if let Some(dest) = &parsed.destination {
                    t.output_path = Some(dest.clone());
                }
            });
            let _ = event;
        } else if let Some(dest) = &parsed.destination {
            self.update_task(id, |t| {
                t.output_path = Some(dest.clone());
            });
        }
    }

    fn persist_history(&self) {
        let path = match config::history_path(&self.app) {
            Ok(p) => p,
            Err(_) => return,
        };
        let list = self.list_tasks();
        // Keep last 200 entries
        let trimmed: Vec<_> = list.into_iter().take(200).collect();
        if let Ok(raw) = serde_json::to_string_pretty(&trimmed) {
            let _ = fs::write(path, raw);
        }
    }
}

fn load_history(app: &AppHandle) -> AppResult<Vec<DownloadTask>> {
    let path = config::history_path(app)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(path)?;
    let list: Vec<DownloadTask> = serde_json::from_str(&raw)?;
    Ok(list)
}
