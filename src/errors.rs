use std::fmt::{Display, Formatter};

use serde::Serialize;

#[derive(Debug, Serialize)]
pub enum AppError {
    Usage(String),
    Runtime(String),
    Io(String),
}

impl AppError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Usage(_) => 2,
            Self::Runtime(_) | Self::Io(_) => 1,
        }
    }
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage(msg) | Self::Runtime(msg) | Self::Io(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(value: reqwest::Error) -> Self {
        Self::Runtime(value.to_string())
    }
}
