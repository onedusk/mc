use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;
use directories::ProjectDirs;
use crate::patterns::BUILTIN_PATTERNS;
use crate::types::Result;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub patterns: PatternConfig,
    pub options: OptionsConfig,
    pub safety: SafetyConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PatternConfig {
    pub directories: Vec<String>,
    pub files: Vec<String>,
    pub exclude: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OptionsConfig {
    #[serde(default = "default_parallel_threads")]
    pub parallel_threads: usize,

    #[serde(default = "default_true")]
    pub require_confirmation: bool,

    #[serde(default = "default_true")]
    pub show_statistics: bool,

    #[serde(default = "default_true")]
    pub preserve_symlinks: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SafetyConfig {
    #[serde(default = "default_true")]
    pub check_git_repo: bool,

    #[serde(default = "default_max_depth")]
    pub max_depth: usize,

    #[serde(default = "default_min_free_space")]
    pub min_free_space_gb: f64,
}

impl Config {
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

    fn default_config_path() -> PathBuf {
        ProjectDirs::from("com", "mc", "mc")
            .map(|dirs| dirs.config_dir().join("config.toml"))
            .unwrap_or_else(|| PathBuf::from(".mc.toml"))
    }

    pub fn merge_cli_args(&mut self, exclude: Vec<String>, include: Vec<String>, nuclear: bool, preserve_env: bool) {
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

        // Handle nuclear mode
        if nuclear {
            // Remove .git and .github from exclusions
            self.patterns.exclude.retain(|p| p != ".git" && p != ".github");

            // Add .env files to removal list if not preserving
            if !preserve_env {
                self.patterns.files.push(".env".to_string());
                self.patterns.files.push(".env.example".to_string());
            }
        }

        // Handle preserve_env
        if preserve_env && !nuclear {
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
                directories: BUILTIN_PATTERNS.directories
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                files: BUILTIN_PATTERNS.files
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                exclude: BUILTIN_PATTERNS.exclude
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