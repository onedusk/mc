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

pub struct ParallelCleaner {
    thread_count: usize,
    chunk_size: usize,
    dry_run: bool,
    progress: Option<Arc<dyn Progress>>,
    stats: Arc<Statistics>,
}

#[derive(Default)]
pub struct Statistics {
    pub items_deleted: AtomicUsize,
    pub bytes_freed: AtomicU64,
    pub errors: DashMap<PathBuf, CleanError>,
}

impl ParallelCleaner {
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

    pub fn with_threads(mut self, count: usize) -> Self {
        self.thread_count = count;
        self
    }

    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    pub fn with_progress(mut self, progress: Arc<dyn Progress>) -> Self {
        self.progress = Some(progress);
        self
    }

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
            duration: start.elapsed(),
            dry_run: false,
        })
    }

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
            duration: std::time::Duration::ZERO,
            dry_run: true,
        })
    }
}

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