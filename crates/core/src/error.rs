use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum EngineError {
    #[error("Network request failed: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Disk I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Server did not provide a valid Content-Length header")]
    MissingContentLength,

    #[error("Worker task panicked or was cancelled: {0}")]
    WorkerJoin(#[from] tokio::task::JoinError),

    #[error("HTTP request returned error status: {0}")]
    HttpStatus(reqwest::StatusCode),

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Task not found: {0}")]
    TaskNotFound(Uuid),

    #[error("Task already exists: {0}")]
    TaskAlreadyExists(Uuid),

    #[error("Download was cancelled")]
    Cancelled,
}
