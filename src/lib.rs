//! # mc
//!
//! `mc` (Mr. Clean) is a high-performance, parallel build directory cleaner.
//!
//! This library provides the core functionality for the `mc` CLI tool, but can also be
//! used as a standalone crate for cleaning build artifacts from your projects.
//!
//! # Features
//!
//! - **Parallel Cleaning**: Utilizes all available CPU cores to clean files concurrently.
//! - **Configurable Patterns**: Define what to clean using TOML configuration files.
//! - **Built-in Patterns**: Comes with a sensible set of default patterns for common project types.
//! - **Safety First**: Includes checks to prevent accidental deletion of important files (e.g., in git repositories).
//! - **Dry Run Mode**: Preview what will be deleted without making any changes.
//! - **Progress Reporting**: Visual feedback on the cleaning process.
//!
//! # Usage
//!
//! The main entry point for using this library is the [`Cleaner`] struct.
//!
//! # Implementation Details
//!
//! The cleaner operates in two main phases:
//! 1.  **Scanning**: A [`Scanner`] traverses the file system from a given root path. It uses `walkdir` to gather directory entries and then processes them in parallel with `rayon` to identify items that match the configured patterns.
//! 2.  **Cleaning**: The identified items are passed to a [`ParallelCleaner`], which reuses a dedicated `rayon` thread pool to delete the files and directories concurrently. Errors are gathered into a shared report the pool threads update as they work.
//!
//! This two-phase approach allows `mc` to first gather all targets and then present them to the user for confirmation (if required) before any destructive operations are performed.
//!
//! # Examples
//!
//! A basic example of how to use the `mc` library to perform a dry run on the current directory:
//!
//! ```no_run
//! use mc::{Cleaner, Config, Result};
//!
//! fn run_cleaner() -> Result<()> {
//!     // Load the default configuration. In a real application, you might load this
//!     // from a `.mc.toml` file.
//!     let config = Config::default();
//!
//!     // Create and configure a new cleaner.
//!     let cleaner = Cleaner::new(config)
//!         .with_dry_run(true) // Enable dry run mode.
//!         .with_quiet(false); // Ensure output is visible.
//!
//!     // Clean the current directory (".")
//!     let report = cleaner.clean(".")?;
//!
//!     println!("Dry run complete!");
//!     println!(
//!         "Would have freed: {} bytes in {} items.",
//!         report.bytes_freed, report.items_deleted
//!     );
//!
//!     Ok(())
//! }
//! ```

pub mod cli;
pub mod config;
pub mod engine;
pub mod patterns;
pub mod safety;
pub mod types;
pub mod utils;

pub use config::{Config, OptionsConfig, PatternConfig, SafetyConfig};
pub use engine::{prune_nested_items, ParallelCleaner, Scanner};
pub use patterns::{PatternMatcher, BUILTIN_PATTERNS};
pub use safety::SafetyGuard;
pub use types::{CleanError, CleanItem, CleanReport, ItemType, McError, PatternCategory, PatternMatch, PatternSource, Result};
pub use utils::{CategoryTracker, CompactDisplay, NoOpProgress, Progress, ProgressReporter, ScanStats};

use std::path::Path;
use std::sync::Arc;

/// The primary interface for cleaning operations.
///
/// `Cleaner` orchestrates the scanning and deletion process based on the provided
/// configuration. It can be configured for different behaviors like dry runs,
/// verbose output, and quiet mode.
///
/// Internally, it constructs and configures the `Scanner` and `ParallelCleaner`
/// to execute the full cleaning workflow.
#[derive(Clone)]
pub struct Cleaner {
    config: Config,
    dry_run: bool,
    quiet: bool,
    verbose: bool,
}

impl Cleaner {
    /// Creates a new `Cleaner` with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration that defines cleaning patterns, options, and safety checks.
    ///              This struct is typically loaded from a `.mc.toml` file or created with `Config::default()`.
    pub fn new(config: Config) -> Self {
        Self {
            config,
            dry_run: false,
            quiet: false,
            verbose: false,
        }
    }

    /// Sets the dry run mode for the cleaner.
    ///
    /// When dry run is enabled, the cleaner will only report what it would delete,
    /// without actually deleting any files. This is achieved by short-circuiting
    /// the `clean` method in `ParallelCleaner` to a reporting-only function.
    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Sets the quiet mode for the cleaner.
    ///
    /// In quiet mode, no output will be printed to the console during the cleaning process,
    /// except for critical errors. This is handled by swapping the `ProgressReporter`
    /// with a `NoOpProgress` implementation.
    pub fn with_quiet(mut self, quiet: bool) -> Self {
        self.quiet = quiet;
        self
    }

    /// Sets the verbose mode for the cleaner.
    ///
    /// Verbose mode may be used in the future to provide more detailed output.
    /// Currently, this has no effect.
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Performs the cleaning operation on the specified path.
    ///
    /// This method will:
    /// 1. Scan the path for items matching the configured patterns.
    /// 2. If items are found, it will clean them in parallel.
    /// 3. Returns a `CleanReport` summarizing the operation.
    ///
    /// # Arguments
    ///
    /// * `path` - The root path to start cleaning from. It must be a generic that
    ///            can be referenced as a `Path`.
    ///
    /// # Errors
    ///
    /// This function can return [`McError`] for issues like I/O errors during scanning,
    /// pattern compilation problems, or configuration loading failures.
    pub fn clean<P: AsRef<Path>>(&self, path: P) -> Result<CleanReport> {
        let path = path.as_ref();

        // Create pattern matcher
        let matcher = Arc::new(PatternMatcher::new(&self.config.patterns)?);

        // Create scanner
        let scanner = Scanner::new(path.to_path_buf(), matcher.clone())
            .with_max_depth(self.config.safety.max_depth)
            .with_symlinks(!self.config.options.preserve_symlinks);

        // Scan for items
        if !self.quiet {
            println!("🔍 Scanning for files to clean...");
        }

        let scan_start = std::time::Instant::now();
        let (items, scan_errors, entries_scanned) = scanner.scan()?;
        let scan_duration = scan_start.elapsed();

        // Prune nested items to avoid redundant deletions
        let items = prune_nested_items(items);

        if items.is_empty() {
            if !self.quiet {
                println!("✅ No files to clean!");
            }
            let mut report = CleanReport::default();
            report.scan_errors = scan_errors;
            report.scan_duration = scan_duration;
            report.entries_scanned = entries_scanned;
            return Ok(report);
        }

        // Create progress reporter
        let progress = if self.quiet {
            Arc::new(NoOpProgress) as Arc<dyn Progress>
        } else {
            Arc::new(ProgressReporter::new(items.len() as u64)) as Arc<dyn Progress>
        };

        // Create cleaner
        let cleaner = ParallelCleaner::new()
            .with_threads(self.config.options.parallel_threads)
            .with_dry_run(self.dry_run)
            .with_progress(progress.clone());

        // Perform cleaning
        let mut report = cleaner.clean(items)?;
        report.scan_errors = scan_errors;
        report.scan_duration = scan_duration;
        report.entries_scanned = entries_scanned;

        // Finish progress
        progress.finish();

        Ok(report)
    }

    /// Performs a dry run of the cleaning operation.
    ///
    /// This is a convenience method that is equivalent to calling `with_dry_run(true)`
    /// and then `clean()`.
    ///
    /// # Arguments
    ///
    /// * `path` - The root path to start the dry run from.
    ///
    /// # Errors
    ///
    /// This function can return the same errors as [`clean`].
    pub fn dry_run<P: AsRef<Path>>(&self, path: P) -> Result<CleanReport> {
        let mut cleaner = self.clone();
        cleaner.dry_run = true;
        cleaner.clean(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::prelude::*;
    use assert_fs::TempDir;

    fn setup_test_dir() -> TempDir {
        let temp = TempDir::new().unwrap();
        temp.child("node_modules/package/index.js")
            .create_dir_all()
            .unwrap();
        temp.child("target/debug/app.exe").create_dir_all().unwrap();
        temp.child("app.log").touch().unwrap();
        temp
    }

    #[test]
    fn test_dry_run() {
        let temp = setup_test_dir();
        let config = Config::default();
        let cleaner = Cleaner::new(config).with_dry_run(true);

        let report = cleaner.clean(temp.path()).unwrap();

        assert!(report.dry_run);
        assert_eq!(report.items_deleted, 3);
        assert!(report.bytes_freed > 0);

        // Verify that files still exist
        temp.child("node_modules")
            .assert(predicates::path::exists());
        temp.child("target").assert(predicates::path::exists());
        temp.child("app.log").assert(predicates::path::exists());
    }

    #[test]
    fn test_actual_clean() {
        let temp = setup_test_dir();
        let config = Config::default();
        let cleaner = Cleaner::new(config).with_dry_run(false);

        let report = cleaner.clean(temp.path()).unwrap();

        assert!(!report.dry_run);
        assert_eq!(report.items_deleted, 3);
        assert!(report.bytes_freed > 0);

        // Verify that files are deleted
        temp.child("node_modules")
            .assert(predicates::path::missing());
        temp.child("target").assert(predicates::path::missing());
        temp.child("app.log").assert(predicates::path::missing());
    }
}
