use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TaskStatus {
    Queued,
    Probing,
    Ready,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTask {
    pub id: Uuid,
    pub url: String,
    pub title: Option<String>,
    pub thumbnail: Option<String>,
    pub format_id: String,
    pub output_dir: String,
    pub output_path: Option<String>,
    pub status: TaskStatus,
    pub progress: f64,
    pub speed: Option<String>,
    pub eta: Option<String>,
    pub log_tail: Vec<String>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl DownloadTask {
    pub fn new(url: String, format_id: String, output_dir: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            url,
            title: None,
            thumbnail: None,
            format_id,
            output_dir,
            output_path: None,
            status: TaskStatus::Queued,
            progress: 0.0,
            speed: None,
            eta: None,
            log_tail: Vec::new(),
            error: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }

    pub fn push_log(&mut self, line: impl Into<String>) {
        self.log_tail.push(line.into());
        if self.log_tail.len() > 80 {
            let overflow = self.log_tail.len() - 80;
            self.log_tail.drain(0..overflow);
        }
        self.touch();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoFormat {
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
pub struct VideoInfo {
    pub id: Option<String>,
    pub title: String,
    pub thumbnail: Option<String>,
    pub duration: Option<f64>,
    pub uploader: Option<String>,
    pub webpage_url: Option<String>,
    pub formats: Vec<VideoFormat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartDownloadRequest {
    pub url: String,
    pub format_id: Option<String>,
    pub output_dir: Option<String>,
    pub title: Option<String>,
    pub thumbnail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub id: Uuid,
    pub percent: f64,
    pub speed: Option<String>,
    pub eta: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEvent {
    pub id: Uuid,
    pub line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinaryVersions {
    pub yt_dlp: Option<String>,
    pub ffmpeg: Option<String>,
}
