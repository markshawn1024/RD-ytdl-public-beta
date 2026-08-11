use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),
    #[error("二进制缺失: {0}")]
    BinaryMissing(String),
    #[error("无效的 URL")]
    InvalidUrl,
    #[error("解析失败: {0}")]
    ProbeFailed(String),
    #[error("下载失败: {0}")]
    DownloadFailed(String),
    #[error("任务不存在")]
    TaskNotFound,
    #[error("已取消")]
    Cancelled,
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON 错误: {0}")]
    Json(#[from] serde_json::Error),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(2))?;
        let code = match self {
            AppError::BinaryMissing(_) => "BinaryMissing",
            AppError::InvalidUrl => "InvalidUrl",
            AppError::ProbeFailed(_) => "ProbeFailed",
            AppError::DownloadFailed(_) => "DownloadFailed",
            AppError::TaskNotFound => "TaskNotFound",
            AppError::Cancelled => "Cancelled",
            AppError::Io(_) => "Io",
            AppError::Json(_) => "Json",
            AppError::Message(_) => "Message",
        };
        map.serialize_entry("code", code)?;
        map.serialize_entry("message", &self.to_string())?;
        map.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;
