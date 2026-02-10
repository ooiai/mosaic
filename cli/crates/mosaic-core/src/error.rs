use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Unknown,
    Config,
    Auth,
    Network,
    Tool,
    Io,
    Validation,
}

impl ErrorCode {
    pub fn exit_code(self) -> i32 {
        match self {
            Self::Unknown => 1,
            Self::Config => 2,
            Self::Auth => 3,
            Self::Network => 4,
            Self::Tool => 5,
            Self::Io => 6,
            Self::Validation => 7,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MosaicError {
    #[error("configuration error: {0}")]
    Config(String),
    #[error("authentication error: {0}")]
    Auth(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("tool error: {0}")]
    Tool(String),
    #[error("I/O error: {0}")]
    Io(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("unknown error: {0}")]
    Unknown(String),
}

impl MosaicError {
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Config(_) => ErrorCode::Config,
            Self::Auth(_) => ErrorCode::Auth,
            Self::Network(_) => ErrorCode::Network,
            Self::Tool(_) => ErrorCode::Tool,
            Self::Io(_) => ErrorCode::Io,
            Self::Validation(_) => ErrorCode::Validation,
            Self::Unknown(_) => ErrorCode::Unknown,
        }
    }

    pub fn exit_code(&self) -> i32 {
        self.code().exit_code()
    }

    pub fn with_context(self, context: impl Display) -> Self {
        match self {
            Self::Config(msg) => Self::Config(format!("{context}: {msg}")),
            Self::Auth(msg) => Self::Auth(format!("{context}: {msg}")),
            Self::Network(msg) => Self::Network(format!("{context}: {msg}")),
            Self::Tool(msg) => Self::Tool(format!("{context}: {msg}")),
            Self::Io(msg) => Self::Io(format!("{context}: {msg}")),
            Self::Validation(msg) => Self::Validation(format!("{context}: {msg}")),
            Self::Unknown(msg) => Self::Unknown(format!("{context}: {msg}")),
        }
    }
}

impl Display for ErrorCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::Unknown => "unknown",
            Self::Config => "config",
            Self::Auth => "auth",
            Self::Network => "network",
            Self::Tool => "tool",
            Self::Io => "io",
            Self::Validation => "validation",
        };
        write!(f, "{text}")
    }
}

impl From<std::io::Error> for MosaicError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

impl From<toml::de::Error> for MosaicError {
    fn from(value: toml::de::Error) -> Self {
        Self::Config(value.to_string())
    }
}

impl From<toml::ser::Error> for MosaicError {
    fn from(value: toml::ser::Error) -> Self {
        Self::Config(value.to_string())
    }
}

impl From<serde_json::Error> for MosaicError {
    fn from(value: serde_json::Error) -> Self {
        Self::Validation(value.to_string())
    }
}

pub type Result<T> = std::result::Result<T, MosaicError>;
