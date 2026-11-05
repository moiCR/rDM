use thiserror::Error;

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
}
