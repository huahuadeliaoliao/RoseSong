use flexi_logger::FlexiLoggerError;
use reqwest::header::InvalidHeaderValue;
use std::io;
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::AcquireError;
use tokio::task::JoinError;

#[derive(Error, Debug)]
pub enum ApplicationError {
    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),

    #[error("Data parsing error: {0}")]
    DataParsingError(String),

    #[error("Header value error: {0}")]
    HeaderValueError(#[from] InvalidHeaderValue),

    #[error("Semaphore acquire error: {0}")]
    SemaphoreAcquireError(#[from] AcquireError),

    #[error("Join task error: {0}")]
    JoinTaskError(#[from] JoinError),

    #[error("GStreamer initialization error: {0}")]
    InitError(String),

    #[error("TOML parsing error: {0}")]
    TomlParsingError(#[from] toml::de::Error),

    #[error("Fetch error: {0}")]
    FetchError(String),

    #[error("Logger initialization error: {0}")]
    LoggerError(#[from] FlexiLoggerError),

    #[error("Channel send error: {0}")]
    SendError(String),
}

impl<T> From<SendError<T>> for ApplicationError {
    fn from(error: SendError<T>) -> Self {
        ApplicationError::SendError(error.to_string())
    }
}
