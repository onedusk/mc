//! This module defines the command-line interface for the `mc` application.
//!
//! It uses the `clap` crate to parse command-line arguments and subcommands,
//! providing a structured way to configure the cleaning process at runtime.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// A high-performance build directory cleaner.
#[derive(Parser)]
#[command(name = "mc")]
#[command(about = "Mr. Cleann - A high-performance build directory cleaner")]
#[command(version)]
#[command(author)]
pub struct Cli {
    /// The root path from which to start cleaning. Defaults to the current directory.
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// If set, previews what would be deleted without performing any actual file operations.
    #[arg(short = 'd', long = "dry-run")]
    pub dry_run: bool,

    /// Enables verbose output, which may provide more details about the cleaning process.
    /// Note: Currently not implemented.
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// Suppresses all non-essential output, showing only critical errors.
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,

    /// Skips any interactive confirmation prompts, useful for scripting.
    /// This overrides the `require_confirmation` setting in the configuration file.
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,

    /// Specifies one or more patterns to exclude from cleaning. Can be repeated.
    /// These are merged with the exclude patterns from the configuration file.
    #[arg(short = 'e', long = "exclude")]
    pub exclude: Vec<String>,

    /// Specifies one or more additional patterns to include for cleaning. Can be repeated.
    /// These are merged with the include patterns from the configuration file.
    #[arg(short = 'i', long = "include")]
    pub include: Vec<String>,

    /// Specifies a path to a custom configuration file (`.mc.toml`).
    /// If not provided, `mc` searches for `.mc.toml` in the current directory and its ancestors.
    #[arg(short = 'c', long = "config")]
    pub config: Option<PathBuf>,

    /// If set, displays detailed statistics after the cleaning operation is complete.
    /// This overrides the `show_statistics` setting in the configuration file.
    #[arg(short = 's', long = "stats")]
    pub stats: bool,

    /// Specifies the number of parallel threads to use for cleaning.
    /// This overrides the `parallel_threads` setting in the configuration file.
    #[arg(short = 'p', long = "parallel")]
    pub parallel: Option<usize>,

    /// Disables the safety check that prevents cleaning inside a git repository.
    /// This overrides the `check_git_repo` setting in the configuration file.
    #[arg(long = "no-git-check")]
    pub no_git_check: bool,

    /// If set, `.env` files will be preserved and not deleted.
    /// This takes precedence over "nuclear" mode for `.env` files.
    #[arg(long = "preserve-env")]
    pub preserve_env: bool,

    /// The subcommand to execute, if any. Subcommands have their own set of options.
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Defines the available subcommands for `mc`.
#[derive(Subcommand, Clone)]
pub enum Commands {
    /// Lists all items that would be cleaned in the target path, without deleting them.
    List {
        /// If set, formats the output as a JSON array.
        #[arg(long = "json")]
        json: bool,
    },

    /// Creates a new `.mc.toml` configuration file in the current or global directory.
    Init {
        /// If set, creates the configuration file in the global user config directory.
        #[arg(long = "global")]
        global: bool,
    },

    /// Displays the current configuration that `mc` would use for the given path.
    Config,
}