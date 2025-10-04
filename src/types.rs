use std::path::PathBuf;
use std::time::Duration;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CleanItem {
    pub path: PathBuf,
    pub size: u64,
    pub item_type: ItemType,
    pub pattern: PatternMatch,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ItemType {
    Directory,
    File,
    Symlink,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PatternMatch {
    pub pattern: String,
    pub priority: u32,
    pub source: PatternSource,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum PatternSource {
    BuiltIn,
    Config,
    CLI,
}

#[derive(Debug, Default, Serialize)]
pub struct CleanReport {
    pub items_deleted: usize,
    pub bytes_freed: u64,
    pub errors: Vec<CleanError>,
    pub duration: Duration,
    pub dry_run: bool,
}

#[derive(Debug, Clone, thiserror::Error, Serialize)]
pub enum CleanError {
    #[error("Permission denied: {path}")]
    PermissionDenied { path: PathBuf },

    #[error("IO error at {path}: {message}")]
    IoError {
        path: PathBuf,
        message: String,
    },

    #[error("Pattern error: {0}")]
    PatternError(String),
}

#[derive(Debug, thiserror::Error)]
pub enum McError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration parse error: {0}")]
    ConfigParse(#[from] toml::de::Error),

    #[error("Configuration serialize error: {0}")]
    ConfigSerialize(#[from] toml::ser::Error),

    #[error("Pattern error: {0}")]
    Pattern(#[from] glob::PatternError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Permission denied: {path}")]
    PermissionDenied { path: PathBuf },

    #[error("Safety check failed: {0}")]
    Safety(String),

    #[error("User cancelled operation")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, McError>;