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

use rayon::prelude::*;
use crossbeam_channel::bounded;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;
use dashmap::DashMap;
use humansize::{format_size, DECIMAL};
use colored::*;

use crate::types::{CleanItem, ItemType, CleanReport, CleanError};
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
    /// If true, no file system modifications will be made.
    dry_run: bool,
    /// An optional, thread-safe progress reporter.
    progress: Option<Arc<dyn Progress>>,
    /// A container for atomically updated statistics.
    stats: Arc<Statistics>,
}

/// A thread-safe structure for collecting statistics during the cleaning process.
///
/// `AtomicUsize` and `AtomicU64` are used to prevent race conditions when multiple
/// threads are updating the statistics concurrently. `DashMap` provides a concurrent
/// hash map for storing errors.
#[derive(Default)]
pub struct Statistics {
    /// The number of items successfully deleted.
    pub items_deleted: AtomicUsize,
    /// The total number of bytes freed.
    pub bytes_freed: AtomicU64,
    /// A map of paths to errors that occurred during deletion.
    pub errors: DashMap<PathBuf, CleanError>,
}

impl ParallelCleaner {
    /// Creates a new `ParallelCleaner`.
    ///
    /// By default, it uses a number of threads equal to the number of logical CPU cores.
    pub fn new() -> Self {
        let thread_count = num_cpus::get();
        Self {
            thread_count,
            chunk_size: 100,
            dry_run: false,
            progress: None,
            stats: Arc::new(Statistics::default()),
        }
    }

    /// Sets the number of threads to use for cleaning.
    pub fn with_threads(mut self, count: usize) -> Self {
        self.thread_count = count;
        self
    }

    /// Sets the dry run mode.
    ///
    /// In dry run mode, the cleaner will report what it would delete but will not
    /// perform any actual file system modifications.
    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
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
    pub fn clean(&self, items: Vec<CleanItem>) -> crate::types::Result<CleanReport> {
        if self.dry_run {
            return self.dry_run_clean(items);
        }

        let start = Instant::now();
        let (error_tx, error_rx) = bounded(100);

        // Set up thread pool
        rayon::ThreadPoolBuilder::new()
            .num_threads(self.thread_count)
            .build()
            .unwrap()
            .install(|| {
                // Process items in parallel
                items.par_chunks(self.chunk_size)
                    .for_each_with(error_tx.clone(), |tx, chunk| {
                        for item in chunk {
                            match self.delete_item(item) {
                                Ok(()) => {
                                    self.stats.items_deleted.fetch_add(1, Ordering::Relaxed);
                                    self.stats.bytes_freed.fetch_add(item.size, Ordering::Relaxed);
                                    if let Some(ref progress) = self.progress {
                                        progress.increment(1);
                                    }
                                }
                                Err(e) => {
                                    let _ = tx.send((item.path.clone(), e));
                                }
                            }
                        }
                    });
            });

        drop(error_tx);

        // Collect errors
        let mut errors = Vec::new();
        while let Ok((path, error)) = error_rx.recv() {
            let clean_error = CleanError::IoError {
                path: path.clone(),
                message: error.to_string(),
            };
            errors.push(clean_error.clone());
            self.stats.errors.insert(path, clean_error);
        }

        Ok(CleanReport {
            items_deleted: self.stats.items_deleted.load(Ordering::Relaxed),
            bytes_freed: self.stats.bytes_freed.load(Ordering::Relaxed),
            errors,
            scan_errors: Vec::new(),
            duration: start.elapsed(),
            dry_run: false,
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

        println!("\n{}", "DRY RUN MODE - No files will be deleted".yellow().bold());
        println!("{}", "─".repeat(50).bright_black());

        // Group items by type for better display
        let mut directories = Vec::new();
        let mut files = Vec::new();

        for item in &items {
            match item.item_type {
                ItemType::Directory => directories.push(item),
                _ => files.push(item),
            }
        }

        // Display directories
        if !directories.is_empty() {
            println!("\n{}:", "Directories to remove".cyan().bold());
            for dir in directories.iter().take(20) {
                println!("  {} {} ({})",
                    "📁".bright_blue(),
                    dir.path.display(),
                    format_size(dir.size, DECIMAL).bright_yellow()
                );
            }
            if directories.len() > 20 {
                println!("  ... and {} more directories", directories.len() - 20);
            }
        }

        // Display files
        if !files.is_empty() {
            println!("\n{}:", "Files to remove".cyan().bold());
            for file in files.iter().take(20) {
                println!("  {} {} ({})",
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
        println!("{}: {}", "Space to free".bold(), format_size(total_size, DECIMAL).bright_green());

        Ok(CleanReport {
            items_deleted: items.len(),
            bytes_freed: total_size,
            errors: Vec::new(),
            scan_errors: Vec::new(),
            duration: std::time::Duration::ZERO,
            dry_run: true,
        })
    }
}

/// Returns the number of available logical CPU cores.
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