//! This module handles the configuration for `mc`.
//!
//! It defines the structure of the `.mc.toml` configuration file and provides
//! functionality for loading, parsing, and merging configurations from files
//! and command-line arguments. The configuration is deserialized using `serde`
//! and `toml`.

use crate::patterns::BUILTIN_PATTERNS;
use crate::types::Result;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// The main configuration structure for `mc`.
///
/// This struct aggregates all configuration settings, including patterns for matching,
/// general options, and safety guardrails. It is designed to be deserialized
/// from a TOML file.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    /// Configuration for file and directory matching patterns.
    pub patterns: PatternConfig,
    /// Configuration for operational options, like parallelism and confirmations.
    pub options: OptionsConfig,
    /// Configuration for safety checks, like git repository detection.
    pub safety: SafetyConfig,
}

/// Defines the patterns used for matching items to be cleaned.
/// These are interpreted as glob patterns.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PatternConfig {
    /// A list of glob patterns for matching directories to be cleaned.
    pub directories: Vec<String>,
    /// A list of glob patterns for matching files to be cleaned.
    pub files: Vec<String>,
    /// A list of glob patterns for excluding items from being cleaned.
    pub exclude: Vec<String>,
}

/// Defines operational options for the cleaner.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OptionsConfig {
    /// The number of parallel threads to use for cleaning. Defaults to the number of CPU cores.
    #[serde(default = "default_parallel_threads")]
    pub parallel_threads: usize,

    /// Whether to require user confirmation before cleaning. Defaults to `true`.
    #[serde(default = "default_true")]
    pub require_confirmation: bool,

    /// Whether to show detailed statistics after cleaning. Defaults to `true`.
    #[serde(default = "default_true")]
    pub show_statistics: bool,

    /// Whether to preserve symbolic links. Defaults to `true`.
    #[serde(default = "default_true")]
    pub preserve_symlinks: bool,
}

/// Defines safety-related configurations for the cleaner.
/// These checks are performed before the scanning phase.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SafetyConfig {
    /// Whether to check if the target path is inside a git repository. Defaults to `true`.
    #[serde(default = "default_true")]
    pub check_git_repo: bool,

    /// The maximum depth to scan for items. Defaults to 10.
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,

    /// The minimum required free disk space in GB before cleaning. Defaults to 1.0.
    #[serde(default = "default_min_free_space")]
    pub min_free_space_gb: f64,
}

impl Config {
    /// Loads the configuration from a file.
    ///
    /// It searches for a `.mc.toml` file in the current directory and its ancestors. If not found, it
    /// checks for a global config file. If a path is provided, it attempts to load
    /// from that path. If no file is found, it falls back to the default configuration.
    ///
    /// # Arguments
    ///
    /// * `path` - An optional path to a specific configuration file.
    pub fn load(path: Option<&PathBuf>) -> Result<Self> {
        let config_path = path
            .cloned()
            .or_else(Self::find_config_file)
            .unwrap_or_else(Self::default_config_path);

        if config_path.exists() {
            let contents = fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&contents)?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    /// Finds the configuration file by searching upward from the current directory.
    fn find_config_file() -> Option<PathBuf> {
        // Look for .mc.toml in current directory and parents
        let current = std::env::current_dir().ok()?;

        for ancestor in current.ancestors() {
            let config = ancestor.join(".mc.toml");
            if config.exists() {
                return Some(config);
            }
        }

        None
    }

    /// Determines the default path for the global configuration file.
    fn default_config_path() -> PathBuf {
        ProjectDirs::from("com", "mc", "mc")
            .map(|dirs| dirs.config_dir().join("config.toml"))
            .unwrap_or_else(|| PathBuf::from(".mc.toml"))
    }

    /// Merges command-line arguments into the configuration.
    ///
    /// This allows overriding the configuration file settings with flags passed
    /// directly to the CLI.
    ///
    /// # Arguments
    ///
    /// * `exclude` - A list of patterns to add to the exclude list.
    /// * `include` - A list of patterns to add to the include lists (files or directories).
    /// * `preserve_env` - A flag to control the preservation of `.env` files.
    pub fn merge_cli_args(
        &mut self,
        exclude: Vec<String>,
        include: Vec<String>,
        preserve_env: bool,
    ) {
        // Add CLI excludes
        for pattern in exclude {
            if !self.patterns.exclude.contains(&pattern) {
                self.patterns.exclude.push(pattern);
            }
        }

        // Add CLI includes
        for pattern in include {
            // Determine if it's a file or directory pattern
            if pattern.contains('.') || pattern.contains('*') {
                if !self.patterns.files.contains(&pattern) {
                    self.patterns.files.push(pattern);
                }
            } else {
                if !self.patterns.directories.contains(&pattern) {
                    self.patterns.directories.push(pattern);
                }
            }
        }

        // Handle preserve_env
        if preserve_env {
            if !self.patterns.exclude.contains(&".env".to_string()) {
                self.patterns.exclude.push(".env".to_string());
            }
            if !self.patterns.exclude.contains(&".env.example".to_string()) {
                self.patterns.exclude.push(".env.example".to_string());
            }
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            patterns: PatternConfig {
                directories: BUILTIN_PATTERNS
                    .directories()
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                files: BUILTIN_PATTERNS
                    .files()
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                exclude: BUILTIN_PATTERNS
                    .exclude
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            },
            options: OptionsConfig::default(),
            safety: SafetyConfig::default(),
        }
    }
}

impl Default for OptionsConfig {
    fn default() -> Self {
        Self {
            parallel_threads: default_parallel_threads(),
            require_confirmation: true,
            show_statistics: true,
            preserve_symlinks: true,
        }
    }
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            check_git_repo: true,
            max_depth: default_max_depth(),
            min_free_space_gb: default_min_free_space(),
        }
    }
}

fn default_parallel_threads() -> usize {
    num_cpus::get()
}

fn default_true() -> bool {
    true
}

fn default_max_depth() -> usize {
    10
}

fn default_min_free_space() -> f64 {
    1.0
}

// Helper function to get CPU count
fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

mod num_cpus {
    pub fn get() -> usize {
        super::num_cpus()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_cli_args_preserve_env() {
        let mut config = Config::default();
        let initial_exclude_len = config.patterns.exclude.len();

        config.merge_cli_args(vec![], vec![], true);

        assert_eq!(config.patterns.exclude.len(), initial_exclude_len + 2);
        assert!(config.patterns.exclude.contains(&".env".to_string()));
        assert!(config
            .patterns
            .exclude
            .contains(&".env.example".to_string()));
    }

    #[test]
    fn test_merge_cli_args_include_and_exclude() {
        let mut config = Config::default();
        let initial_exclude_len = config.patterns.exclude.len();
        let initial_files_len = config.patterns.files.len();
        let initial_dirs_len = config.patterns.directories.len();

        let excludes = vec!["custom_exclude".to_string()];
        let includes = vec!["custom_include.file".to_string(), "custom_dir".to_string()];

        config.merge_cli_args(excludes.clone(), includes.clone(), false);

        assert_eq!(
            config.patterns.exclude.len(),
            initial_exclude_len + excludes.len()
        );
        assert!(config.patterns.exclude.contains(&excludes[0]));

        assert_eq!(config.patterns.files.len(), initial_files_len + 1);
        assert!(config.patterns.files.contains(&includes[0]));

        assert_eq!(config.patterns.directories.len(), initial_dirs_len + 1);
        assert!(config.patterns.directories.contains(&includes[1]));
    }
}
