use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mc")]
#[command(about = "Mr. Cleann - A high-performance build directory cleaner")]
#[command(version)]
#[command(author)]
pub struct Cli {
    /// Path to clean
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Preview what would be deleted without actually deleting
    #[arg(short = 'd', long = "dry-run")]
    pub dry_run: bool,

    /// Verbose output
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// Suppress non-essential output
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,

    /// Skip confirmation prompts
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,

    /// Exclude patterns (can be repeated)
    #[arg(short = 'e', long = "exclude")]
    pub exclude: Vec<String>,

    /// Additional patterns to clean
    #[arg(short = 'i', long = "include")]
    pub include: Vec<String>,

    /// Use custom configuration file
    #[arg(short = 'c', long = "config")]
    pub config: Option<PathBuf>,

    /// Show statistics after cleaning
    #[arg(short = 's', long = "stats")]
    pub stats: bool,

    /// Number of parallel threads
    #[arg(short = 'p', long = "parallel")]
    pub parallel: Option<usize>,

    /// Don't check for .git directories
    #[arg(long = "no-git-check")]
    pub no_git_check: bool,

    /// Preserve .env files
    #[arg(long = "preserve-env")]
    pub preserve_env: bool,

    /// Include dangerous operations (git, env files)
    #[arg(long = "nuclear")]
    pub nuclear: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Clone)]
pub enum Commands {
    /// List what would be cleaned without deleting
    List {
        /// Format output as JSON
        #[arg(long = "json")]
        json: bool,
    },

    /// Initialize configuration file
    Init {
        /// Global configuration
        #[arg(long = "global")]
        global: bool,
    },

    /// Show current configuration
    Config,
}