pub mod types;
pub mod config;
pub mod patterns;
pub mod engine;
pub mod safety;
pub mod utils;
pub mod cli;

pub use types::{CleanItem, CleanReport, CleanError, McError, Result};
pub use config::{Config, PatternConfig, OptionsConfig, SafetyConfig};
pub use patterns::{PatternMatcher, BUILTIN_PATTERNS};
pub use engine::{Scanner, ParallelCleaner};
pub use safety::SafetyGuard;
pub use utils::{Progress, ProgressReporter, NoOpProgress};

use std::path::Path;
use std::sync::Arc;

/// Main cleaner interface
pub struct Cleaner {
    config: Config,
    dry_run: bool,
    quiet: bool,
    verbose: bool,
}

impl Cleaner {
    /// Creates a new cleaner with the given configuration
    pub fn new(config: Config) -> Self {
        Self {
            config,
            dry_run: false,
            quiet: false,
            verbose: false,
        }
    }

    /// Sets dry run mode
    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Sets quiet mode
    pub fn with_quiet(mut self, quiet: bool) -> Self {
        self.quiet = quiet;
        self
    }

    /// Sets verbose mode
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Performs the cleaning operation
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

        let items = scanner.scan()?;

        if items.is_empty() {
            if !self.quiet {
                println!("✅ No files to clean!");
            }
            return Ok(CleanReport::default());
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
        let report = cleaner.clean(items)?;

        // Finish progress
        progress.finish();

        Ok(report)
    }

    /// Performs a dry run without deleting anything
    pub fn dry_run<P: AsRef<Path>>(&self, path: P) -> Result<CleanReport> {
        let mut cleaner = self.clone();
        cleaner.dry_run = true;
        cleaner.clean(path)
    }
}

impl Clone for Cleaner {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            dry_run: self.dry_run,
            quiet: self.quiet,
            verbose: self.verbose,
        }
    }
}