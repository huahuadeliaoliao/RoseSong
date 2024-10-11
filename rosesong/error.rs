use flexi_logger::FlexiLoggerError;
use glib::BoolError;
use reqwest::header::InvalidHeaderValue;
use std::io;
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::AcquireError;
use tokio::task::JoinError;
use zbus::Error as ZbusError;

#[derive(Error, Debug, Clone)]
pub enum App {
    #[error("Network error: {0}")]
    Network(String),

    #[error("I/O error: {0}")]
    Io(String),

    #[error("Data parsing error: {0}")]
    DataParsing(String),

    #[error("Header value error: {0}")]
    HeaderValue(String),

    #[error("Semaphore acquire error: {0}")]
    SemaphoreAcquire(String),

    #[error("Join task error: {0}")]
    JoinTask(String),

    #[error("GStreamer initialization error: {0}")]
    Init(String),

    #[error("TOML parsing error: {0}")]
    TomlParsing(String),

    #[error("Fetch error: {0}")]
    Fetch(String),

    #[error("Logger initialization error: {0}")]
    Logger(String),

    #[error("Channel send error: {0}")]
    Send(String),

    #[error("GStreamer element error: {0}")]
    Element(String),

    #[error("GStreamer pipeline error: {0}")]
    Pipeline(String),

    #[error("GStreamer link error: {0}")]
    Link(String),

    #[error("GStreamer state error: {0}")]
    State(String),

    #[error("ZBus error: {0}")]
    ZBus(String),
}

impl From<reqwest::Error> for App {
    fn from(error: reqwest::Error) -> Self {
        App::Network(error.to_string())
    }
}

impl From<io::Error> for App {
    fn from(error: io::Error) -> Self {
        App::Io(error.to_string())
    }
}

impl From<InvalidHeaderValue> for App {
    fn from(error: InvalidHeaderValue) -> Self {
        App::HeaderValue(error.to_string())
    }
}

impl From<AcquireError> for App {
    fn from(error: AcquireError) -> Self {
        App::SemaphoreAcquire(error.to_string())
    }
}

impl From<JoinError> for App {
    fn from(error: JoinError) -> Self {
        App::JoinTask(error.to_string())
    }
}

impl From<toml::de::Error> for App {
    fn from(error: toml::de::Error) -> Self {
        App::TomlParsing(error.to_string())
    }
}

impl From<FlexiLoggerError> for App {
    fn from(error: FlexiLoggerError) -> Self {
        App::Logger(error.to_string())
    }
}

impl<T> From<SendError<T>> for App {
    fn from(error: SendError<T>) -> Self {
        App::Send(error.to_string())
    }
}

impl From<BoolError> for App {
    fn from(_: BoolError) -> Self {
        App::Init("Failed to perform an operation on GStreamer pipeline".to_string())
    }
}

impl From<ZbusError> for App {
    fn from(error: ZbusError) -> Self {
        App::ZBus(error.to_string())
    }
}
