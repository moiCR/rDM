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

    #[error("Incomplete segment download: expected {expected} bytes, received {actual} bytes")]
    IncompleteSegment { expected: u64, actual: u64 },

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Task not found: {0}")]
    TaskNotFound(Uuid),

    #[error("Task already exists: {0}")]
    TaskAlreadyExists(Uuid),

    #[error("Download was cancelled")]
    Cancelled,

    #[error("Insufficient disk space: required {required} bytes, available {available} bytes")]
    InsufficientDiskSpace { required: u64, available: u64 },

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
