use thiserror::Error;

#[derive(Error, Debug)]
pub enum CliError {
    #[error("Missing URL to download")]
    MissingUrl,

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Core engine error: {0}")]
    Engine(#[from] rdm_core::error::EngineError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
