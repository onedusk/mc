//! This module implements the parallel cleaning logic for `mc`.
//!
//! It uses the `rayon` crate to process and delete multiple files and directories
//! concurrently, which significantly speeds up the cleaning process on multi-core systems.
//! The `ParallelCleaner` is the main entry point for this functionality.
//!
//! # Performance
//!
//! The cleaning process is parallelized by chunking the list of items to be deleted
//! and processing each chunk on a separate thread in a `rayon` thread pool. This
//! approach is effective for I/O-bound tasks like file deletion, as it allows the
//! OS to handle multiple deletion requests simultaneously.

use colored::*;
use humansize::{format_size, DECIMAL};
use rayon::{prelude::*, ThreadPool, ThreadPoolBuilder};
use std::fs;
use std::io;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::types::{CleanError, CleanItem, CleanReport, ItemType, McError};
use crate::utils::progress::Progress;

/// A parallel cleaner that deletes items concurrently using a thread pool.
///
/// `ParallelCleaner` is responsible for the actual deletion of files and directories.
/// It can be configured with a specific number of threads, dry run mode, and a
/// progress reporter.
pub struct ParallelCleaner {
    /// The number of threads to spawn in the `rayon` thread pool.
    thread_count: usize,
    /// The number of items to process in each parallel chunk.
    chunk_size: usize,
    /// Reusable thread pool for file operations.
    thread_pool: Arc<ThreadPool>,
    /// If true, no file system modifications will be made.
    dry_run: bool,
    /// If true, suppress human-readable output (for --json or --quiet).
    quiet: bool,
    /// An optional, thread-safe progress reporter.
    progress: Option<Arc<dyn Progress>>,
    /// A container for atomically updated statistics.
    stats: Arc<Statistics>,
}

/// Thread-safe counters updated during parallel deletion.
/// Errors are collected via the `Mutex<Vec>` in the `clean()` method.
#[derive(Default)]
pub struct Statistics {
    /// The number of items successfully deleted.
    pub items_deleted: AtomicUsize,
    /// The total number of bytes freed.
    pub bytes_freed: AtomicU64,
    /// The number of directories successfully deleted.
    pub dirs_deleted: AtomicUsize,
    /// The number of files successfully deleted.
    pub files_deleted: AtomicUsize,
}

impl ParallelCleaner {
    /// Creates a new `ParallelCleaner`.
    ///
    /// Returns an error if the thread pool cannot be created (e.g., resource exhaustion).
    pub fn new() -> std::result::Result<Self, McError> {
        let thread_count = crate::utils::available_parallelism();
        log::debug!("ParallelCleaner: {} threads", thread_count);
        let thread_pool = ThreadPoolBuilder::new()
            .num_threads(thread_count)
            .build()
            .map_err(|e| McError::ThreadPool(e.to_string()))?;
        Ok(Self {
            thread_count,
            chunk_size: 1,
            thread_pool: Arc::new(thread_pool),
            dry_run: false,
            quiet: false,
            progress: None,
            stats: Arc::new(Statistics::default()),
        })
    }

    /// Sets the number of threads to use for cleaning.
    ///
    /// Returns an error if the thread pool cannot be rebuilt.
    pub fn with_threads(mut self, count: usize) -> std::result::Result<Self, McError> {
        self.thread_count = count;
        self.thread_pool = Arc::new(
            ThreadPoolBuilder::new()
                .num_threads(count)
                .build()
                .map_err(|e| McError::ThreadPool(e.to_string()))?,
        );
        Ok(self)
    }

    /// Sets the dry run mode.
    ///
    /// In dry run mode, the cleaner will report what it would delete but will not
    /// perform any actual file system modifications.
    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Sets quiet mode, suppressing human-readable output from dry-run.
    pub fn with_quiet(mut self, quiet: bool) -> Self {
        self.quiet = quiet;
        self
    }

    /// Attaches a progress reporter to the cleaner.
    ///
    /// The progress reporter will be updated as items are cleaned.
    pub fn with_progress(mut self, progress: Arc<dyn Progress>) -> Self {
        self.progress = Some(progress);
        self
    }

    /// Cleans the given list of `CleanItem`s.
    ///
    /// This is the main method that executes the cleaning process. It distributes
    /// the work across a `rayon` thread pool and collects the results.
    ///
    /// This method will block until all items are processed.
    ///
    /// # Arguments
    ///
    /// * `items` - A vector of `CleanItem`s to be deleted.
    ///
    /// # Returns
    ///
    /// A `CleanReport` summarizing the results of the operation. Errors that occur
    /// during file deletion are collected and included in the report, but they do
    /// not stop the entire cleaning process.
    pub fn clean(&self, mut items: Vec<CleanItem>) -> crate::types::Result<CleanReport> {
        log::debug!("Cleaning {} items (dry_run={})", items.len(), self.dry_run);
        if self.dry_run {
            return self.dry_run_clean(items);
        }

        // Sort by size descending so large directories start processing first.
        // This improves parallelization by avoiding the scenario where one thread
        // grinds through a huge directory at the end while others sit idle.
        items.sort_by(|a, b| b.size.cmp(&a.size));

        self.stats.items_deleted.store(0, Ordering::Relaxed);
        self.stats.bytes_freed.store(0, Ordering::Relaxed);

        let start = Instant::now();
        let progress = self.progress.clone();
        let stats = Arc::clone(&self.stats);
        let errors = Mutex::new(Vec::new());
        let chunk_size = self.chunk_size;

        self.thread_pool.install(|| {
            items.par_iter().with_min_len(chunk_size).for_each(|item| {
                match self.delete_item(item) {
                    Ok(()) => {
                        stats.items_deleted.fetch_add(1, Ordering::Relaxed);
                        stats.bytes_freed.fetch_add(item.size, Ordering::Relaxed);
                        match item.item_type {
                            ItemType::Directory => { stats.dirs_deleted.fetch_add(1, Ordering::Relaxed); }
                            _ => { stats.files_deleted.fetch_add(1, Ordering::Relaxed); }
                        }
                        if let Some(ref progress) = progress {
                            progress.increment(1);
                        }
                    }
                    Err(err) => {
                        log::debug!("Delete failed: {}: {}", item.path.display(), err);
                        let clean_error = CleanError::IoError {
                            path: item.path.clone(),
                            message: err.to_string(),
                        };
                        errors
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .push(clean_error);
                    }
                }
            });
        });

        let errors = match errors.into_inner() {
            Ok(list) => list,
            Err(poisoned) => poisoned.into_inner(),
        };

        log::debug!("Clean done: {} deleted, {} errors",
            stats.items_deleted.load(Ordering::Relaxed), errors.len());

        Ok(CleanReport {
            items_deleted: stats.items_deleted.load(Ordering::Relaxed),
            bytes_freed: stats.bytes_freed.load(Ordering::Relaxed),
            errors,
            scan_errors: Vec::new(),
            duration: start.elapsed(),
            scan_duration: std::time::Duration::ZERO,
            dry_run: false,
            dirs_deleted: stats.dirs_deleted.load(Ordering::Relaxed),
            files_deleted: stats.files_deleted.load(Ordering::Relaxed),
            entries_scanned: 0, // Set by caller
        })
    }

    /// Deletes a single `CleanItem` from the file system.
    ///
    /// This function handles the logic for deleting directories, files, and symlinks
    /// appropriately.
    fn delete_item(&self, item: &CleanItem) -> io::Result<()> {
        match item.item_type {
            ItemType::Directory => {
                fs::remove_dir_all(&item.path)?;
            }
            ItemType::File => {
                fs::remove_file(&item.path)?;
            }
            ItemType::Symlink => {
                // Handle symlinks specially
                #[cfg(unix)]
                {
                    fs::remove_file(&item.path)?;
                }
                #[cfg(windows)]
                {
                    if item.path.is_dir() {
                        fs::remove_dir(&item.path)?;
                    } else {
                        fs::remove_file(&item.path)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Performs a dry run, reporting what would be cleaned without deleting anything.
    fn dry_run_clean(&self, items: Vec<CleanItem>) -> crate::types::Result<CleanReport> {
        let total_size: u64 = items.iter().map(|i| i.size).sum();

        // Group items by type
        let mut directories = Vec::new();
        let mut files = Vec::new();

        for item in &items {
            match item.item_type {
                ItemType::Directory => directories.push(item),
                _ => files.push(item),
            }
        }

        if !self.quiet {
            println!(
                "\n{}",
                "DRY RUN MODE - No files will be deleted".yellow().bold()
            );
            println!("{}", "─".repeat(50).bright_black());

            if !directories.is_empty() {
                println!("\n{}:", "Directories to remove".cyan().bold());
                for dir in directories.iter().take(20) {
                    println!(
                        "  {} {} ({})",
                        "📁".bright_blue(),
                        dir.path.display(),
                        format_size(dir.size, DECIMAL).bright_yellow()
                    );
                }
                if directories.len() > 20 {
                    println!("  ... and {} more directories", directories.len() - 20);
                }
            }

            if !files.is_empty() {
                println!("\n{}:", "Files to remove".cyan().bold());
                for file in files.iter().take(20) {
                    println!(
                        "  {} {} ({})",
                        "📄".bright_green(),
                        file.path.display(),
                        format_size(file.size, DECIMAL).bright_yellow()
                    );
                }
                if files.len() > 20 {
                    println!("  ... and {} more files", files.len() - 20);
                }
            }

            println!("\n{}", "─".repeat(50).bright_black());
            println!("{}: {} items", "Total".bold(), items.len());
            println!(
                "{}: {}",
                "Space to free".bold(),
                format_size(total_size, DECIMAL).bright_green()
            );
        }

        let dir_count = directories.len();
        let file_count = files.len();

        Ok(CleanReport {
            items_deleted: items.len(),
            bytes_freed: total_size,
            errors: Vec::new(),
            scan_errors: Vec::new(),
            duration: std::time::Duration::ZERO,
            scan_duration: std::time::Duration::ZERO,
            dry_run: true,
            dirs_deleted: dir_count,
            files_deleted: file_count,
            entries_scanned: 0, // Set by caller
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{PatternCategory, PatternMatch, PatternSource};
    use assert_fs::prelude::*;
    use assert_fs::TempDir;

    fn make_clean_items(paths: &[&std::path::Path], item_type: ItemType) -> Vec<CleanItem> {
        paths
            .iter()
            .map(|p| CleanItem {
                path: p.to_path_buf(),
                size: 100,
                item_type: item_type.clone(),
                pattern: PatternMatch {
                    pattern: "test".to_string(),
                    priority: 0,
                    source: PatternSource::BuiltIn,
                    category: PatternCategory::Other,
                },
            })
            .collect()
    }

    #[test]
    fn test_new_returns_result() {
        let cleaner = ParallelCleaner::new();
        assert!(cleaner.is_ok());
    }

    #[test]
    fn test_with_threads_returns_result() {
        let cleaner = ParallelCleaner::new().unwrap().with_threads(2);
        assert!(cleaner.is_ok());
    }

    #[test]
    fn test_clean_deletes_files() {
        let temp = TempDir::new().unwrap();
        let f1 = temp.child("a.log");
        let f2 = temp.child("b.log");
        let f3 = temp.child("c.log");
        f1.touch().unwrap();
        f2.touch().unwrap();
        f3.touch().unwrap();

        let items = make_clean_items(
            &[f1.path(), f2.path(), f3.path()],
            ItemType::File,
        );

        let cleaner = ParallelCleaner::new()
            .unwrap()
            .with_dry_run(false);
        let report = cleaner.clean(items).unwrap();

        assert_eq!(report.items_deleted, 3);
        assert!(!report.dry_run);
        assert!(!f1.path().exists());
        assert!(!f2.path().exists());
        assert!(!f3.path().exists());
    }

    #[test]
    fn test_clean_dry_run_preserves_files() {
        let temp = TempDir::new().unwrap();
        let f1 = temp.child("a.log");
        f1.touch().unwrap();

        let items = make_clean_items(&[f1.path()], ItemType::File);

        let cleaner = ParallelCleaner::new()
            .unwrap()
            .with_dry_run(true);
        let report = cleaner.clean(items).unwrap();

        assert!(report.dry_run);
        assert_eq!(report.items_deleted, 1);
        assert!(f1.path().exists(), "dry run should not delete files");
    }

    #[test]
    fn test_clean_collects_errors() {
        let temp = TempDir::new().unwrap();
        // Point to a non-existent file so deletion fails
        let missing = temp.path().join("does_not_exist.log");
        let items = make_clean_items(&[missing.as_path()], ItemType::File);

        let cleaner = ParallelCleaner::new()
            .unwrap()
            .with_dry_run(false);
        let report = cleaner.clean(items).unwrap();

        assert_eq!(report.errors.len(), 1);
        match &report.errors[0] {
            CleanError::IoError { path, .. } => {
                assert_eq!(path, &missing);
            }
            other => panic!("Expected IoError, got {:?}", other),
        }
    }
}

