//! This module defines the core data types and error types used throughout the `mc` crate.
//!
//! It provides a centralized location for structures that represent the state and results
//! of the cleaning process, ensuring consistency across different modules. These types
//! are designed to be serializable with `serde` for potential use in structured
//! output formats like JSON.

use serde::Serialize;
use std::path::PathBuf;
use std::time::Duration;

/// Represents an item on the file system that has been identified for cleaning.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CleanItem {
    /// The absolute path to the item.
    pub path: PathBuf,
    /// The size of the item in bytes. For directories, this is the recursive size.
    pub size: u64,
    /// The type of the file system item (directory, file, or symlink).
    pub item_type: ItemType,
    /// Details about the pattern that matched this item.
    pub pattern: PatternMatch,
}

/// An enumeration of the types of file system items that can be cleaned.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ItemType {
    /// A directory.
    Directory,
    /// A regular file.
    File,
    /// A symbolic link.
    Symlink,
}

/// Represents the details of a pattern match.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PatternMatch {
    /// The glob pattern that was matched.
    pub pattern: String,
    /// The priority of the match (lower is higher priority).
    pub priority: u32,
    /// The source of the pattern (e.g., built-in, config, or CLI).
    pub source: PatternSource,
    /// The category of the pattern for UI grouping.
    pub category: PatternCategory,
}

/// An enumeration of the possible sources for a cleaning pattern.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum PatternSource {
    /// A pattern that is built into `mc`.
    BuiltIn,
    /// A pattern from a `.mc.toml` configuration file.
    Config,
    /// A pattern provided via a command-line argument.
    CLI,
}

/// Categories for organizing matched patterns in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum PatternCategory {
    /// Dependencies (node_modules, vendor, .venv)
    Dependencies,
    /// Build outputs (dist, build, target, .next, out)
    BuildOutputs,
    /// Cache files (.turbo, .pytest_cache, coverage)
    Cache,
    /// IDE and tool files (.idea, .vscode)
    IDE,
    /// Log files
    Logs,
    /// Other/uncategorized
    Other,
}

impl PatternCategory {
    /// Returns a human-readable label for the category.
    pub fn label(&self) -> &'static str {
        match self {
            PatternCategory::Dependencies => "Dependencies",
            PatternCategory::BuildOutputs => "Build",
            PatternCategory::Cache => "Cache",
            PatternCategory::IDE => "IDE",
            PatternCategory::Logs => "Logs",
            PatternCategory::Other => "Other",
        }
    }
}

/// A report summarizing the results of a cleaning operation.
#[derive(Debug, Default, Serialize)]
pub struct CleanReport {
    /// The total number of items deleted.
    pub items_deleted: usize,
    /// The total number of bytes freed.
    pub bytes_freed: u64,
    /// A list of errors that occurred during the cleaning process.
    pub errors: Vec<CleanError>,
    /// A list of errors that occurred during the scanning process.
    pub scan_errors: Vec<ScanError>,
    /// The total duration of the cleaning operation.
    pub duration: Duration,
    /// The duration of the scanning phase.
    pub scan_duration: Duration,
    /// A flag indicating whether the operation was a dry run.
    pub dry_run: bool,
    /// Number of directories deleted.
    pub dirs_deleted: usize,
    /// Number of files deleted.
    pub files_deleted: usize,
    /// Total entries scanned during the scan phase.
    pub entries_scanned: usize,
}

/// An error that can occur during the cleaning of a single item.
/// These errors are typically specific to a single item and do not stop the entire operation,
/// but they do not stop the entire operation.
#[derive(Debug, Clone, thiserror::Error, Serialize)]
pub enum CleanError {
    /// An error indicating that a file or directory could not be accessed.
    #[error("Permission denied: {path}")]
    PermissionDenied { path: PathBuf },

    /// A general I/O error that occurred during deletion.
    #[error("IO error at {path}: {message}")]
    IoError { path: PathBuf, message: String },

    /// An error related to parsing a glob pattern.
    #[error("Pattern error: {0}")]
    PatternError(String),
}

/// An error that can occur during the scanning of the file system.
#[derive(Debug, Clone, thiserror::Error, Serialize)]
pub enum ScanError {
    /// An I/O error that occurred while accessing a path.
    #[error("IO error at {path}: {message}")]
    IoError { path: PathBuf, message: String },
    /// A symbolic link cycle was detected.
    #[error("Symbolic link cycle detected at {path}")]
    SymlinkCycle { path: PathBuf },
}

/// The main error type for the `mc` crate.
///
/// This enum consolidates all possible errors that can occur during the entire
/// lifecycle of the application, from configuration loading to file cleaning.
/// It uses `thiserror` to derive the `Error` trait and provide convenient
/// error handling.
#[derive(Debug, thiserror::Error)]
pub enum McError {
    /// An I/O error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// An error that occurred while parsing a TOML configuration file.
    #[error("Configuration parse error: {0}")]
    ConfigParse(#[from] toml::de::Error),

    /// An error that occurred while serializing a configuration to TOML.
    #[error("Configuration serialize error: {0}")]
    ConfigSerialize(#[from] toml::ser::Error),

    /// An error related to an invalid glob pattern.
    #[error("Pattern error: {0}")]
    Pattern(#[from] glob::PatternError),

    /// An error that occurred during JSON serialization.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// An error indicating a permission issue.
    #[error("Permission denied: {path}")]
    PermissionDenied { path: PathBuf },

    /// An error indicating that a safety check failed.
    #[error("Safety check failed: {0}")]
    Safety(String),

    /// An error indicating that the user cancelled the operation.
    #[error("User cancelled operation")]
    Cancelled,
}

/// A specialized `Result` type for the `mc` crate, using `McError` as the error type.
pub type Result<T> = std::result::Result<T, McError>;
