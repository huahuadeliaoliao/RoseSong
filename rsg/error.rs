use reqwest::Error as ReqwestError;
use std::io::Error as IoError;
use thiserror::Error;
use zbus::Error as ZbusError;

#[derive(Error, Debug)]
pub enum App {
    #[error("HTTP request failed")]
    HttpRequest(#[from] ReqwestError),
    #[error("I/O operation failed")]
    Io(#[from] IoError),
    #[error("Data parsing error: {0}")]
    DataParsing(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Environment variable error")]
    EnvVar(#[from] std::env::VarError),
    #[error("UTF-8 conversion error")]
    Utf8Conversion(#[from] std::string::FromUtf8Error),
    #[error("Oneshot channel receive error")]
    OneshotRecv(#[from] tokio::sync::oneshot::error::RecvError),
    #[error("Zbus error")]
    Zbus(#[from] ZbusError),
}
