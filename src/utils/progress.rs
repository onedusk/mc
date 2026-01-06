//! This module provides utilities for reporting progress during long-running operations.
//!
//! It defines a `Progress` trait that abstracts the progress reporting mechanism,
//! allowing for different implementations, such as a visual progress bar or a no-op
//! reporter for quiet mode. This decouples the core logic from the specifics of
//! the UI representation.

use crate::types::PatternCategory;
use colored::*;
use dashmap::DashMap;
use humansize::{format_size, DECIMAL};
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// A trait for progress reporters.
///
/// This allows different parts of the application to report progress without being
/// coupled to a specific implementation. The trait must be `Send + Sync` to allow
/// it to be shared safely across threads.
pub trait Progress: Send + Sync {
    /// Increments the progress by a given amount.
    fn increment(&self, delta: u64);
    /// Sets a message to be displayed with the progress indicator.
    fn set_message(&self, msg: &str);
    /// Finishes the progress reporting, typically hiding the indicator.
    fn finish(&self);
}

/// Thread-safe statistics for scan operations.
#[derive(Default)]
pub struct ScanStats {
    /// Total entries walked (files + dirs)
    pub entries_scanned: AtomicUsize,
    /// Directories traversed
    pub dirs_scanned: AtomicUsize,
    /// Files examined
    pub files_scanned: AtomicUsize,
    /// Items matched for cleaning
    pub items_matched: AtomicUsize,
    /// Bytes matched for cleaning
    pub bytes_matched: AtomicU64,
}

impl ScanStats {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn inc_entry(&self) {
        self.entries_scanned.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn inc_dir(&self) {
        self.dirs_scanned.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn inc_file(&self) {
        self.files_scanned.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn inc_matched(&self, size: u64) {
        self.items_matched.fetch_add(1, Ordering::Relaxed);
        self.bytes_matched.fetch_add(size, Ordering::Relaxed);
    }

    pub fn entries(&self) -> usize {
        self.entries_scanned.load(Ordering::Relaxed)
    }

    pub fn dirs(&self) -> usize {
        self.dirs_scanned.load(Ordering::Relaxed)
    }

    pub fn files(&self) -> usize {
        self.files_scanned.load(Ordering::Relaxed)
    }

    pub fn matched(&self) -> usize {
        self.items_matched.load(Ordering::Relaxed)
    }

    pub fn matched_bytes(&self) -> u64 {
        self.bytes_matched.load(Ordering::Relaxed)
    }
}

/// A progress reporter that displays a visual progress bar in the console.
///
/// This implementation uses the `indicatif` crate to render a customizable
/// progress bar.
pub struct ProgressReporter {
    bar: ProgressBar,
}

impl ProgressReporter {
    /// Creates a new `ProgressReporter` with a given total number of steps.
    pub fn new(total: u64) -> Self {
        let bar = ProgressBar::new(total);
        bar.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")
                .unwrap()
                .progress_chars("#>-"),
        );

        Self { bar }
    }
}

impl Progress for ProgressReporter {
    fn increment(&self, delta: u64) {
        self.bar.inc(delta);
    }

    fn set_message(&self, msg: &str) {
        self.bar.set_message(msg.to_string());
    }

    fn finish(&self) {
        self.bar.finish_with_message("Complete");
    }
}

/// A no-op progress reporter that does nothing.
///
/// This is used when quiet mode is enabled to avoid printing any progress
/// information to the console.
pub struct NoOpProgress;

impl Progress for NoOpProgress {
    fn increment(&self, _: u64) {}
    fn set_message(&self, _: &str) {}
    fn finish(&self) {}
}

/// Tracks statistics per category for compact display.
#[derive(Default)]
pub struct CategoryTracker {
    /// Number of items per category
    counts: DashMap<PatternCategory, AtomicUsize>,
    /// Total size per category
    sizes: DashMap<PatternCategory, AtomicU64>,
}

impl CategoryTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an item to a category
    pub fn add_item(&self, category: PatternCategory, size: u64) {
        self.counts
            .entry(category)
            .or_insert_with(|| AtomicUsize::new(0))
            .fetch_add(1, Ordering::Relaxed);

        self.sizes
            .entry(category)
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(size, Ordering::Relaxed);
    }

    /// Gets the count for a category
    pub fn get_count(&self, category: PatternCategory) -> usize {
        self.counts
            .get(&category)
            .map(|v| v.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// Gets the size for a category
    pub fn get_size(&self, category: PatternCategory) -> u64 {
        self.sizes
            .get(&category)
            .map(|v| v.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// Gets total count across all categories
    pub fn total_count(&self) -> usize {
        self.counts
            .iter()
            .map(|entry| entry.value().load(Ordering::Relaxed))
            .sum()
    }

    /// Gets total size across all categories
    pub fn total_size(&self) -> u64 {
        self.sizes
            .iter()
            .map(|entry| entry.value().load(Ordering::Relaxed))
            .sum()
    }

    /// Formats the category breakdown for display
    pub fn format_breakdown(&self) -> String {
        let mut parts = Vec::new();

        // Only show categories that have items
        for category in [
            PatternCategory::Dependencies,
            PatternCategory::BuildOutputs,
            PatternCategory::Cache,
            PatternCategory::IDE,
            PatternCategory::Logs,
            PatternCategory::Other,
        ] {
            let count = self.get_count(category);
            if count > 0 {
                let size = self.get_size(category);
                parts.push(format!(
                    "{}: {} ({})",
                    category.label().bright_cyan(),
                    count.to_string().bright_white(),
                    format_size(size, DECIMAL).bright_green()
                ));
            }
        }

        parts.join("  ")
    }
}

/// A compact 3-line progress display for scanning and cleaning operations.
pub struct CompactDisplay {
    bar: ProgressBar,
    category_tracker: Arc<CategoryTracker>,
    scan_stats: Arc<ScanStats>,
    start_time: Instant,
    last_update: AtomicU64,
}

impl CompactDisplay {
    pub fn new_for_scanning(category_tracker: Arc<CategoryTracker>) -> Self {
        let bar = ProgressBar::new_spinner();
        bar.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.cyan} {msg}")
                .unwrap()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
        );
        bar.enable_steady_tick(std::time::Duration::from_millis(80));

        Self {
            bar,
            category_tracker,
            scan_stats: Arc::new(ScanStats::new()),
            start_time: Instant::now(),
            last_update: AtomicU64::new(0),
        }
    }

    pub fn new_for_cleaning(total: u64) -> Self {
        let bar = ProgressBar::new(total);
        bar.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} {msg}\n  [{bar:40.green/dim}] {pos}/{len}\n  {elapsed_precise}")
                .unwrap()
                .progress_chars("█░")
        );

        Self {
            bar,
            category_tracker: Arc::new(CategoryTracker::new()),
            scan_stats: Arc::new(ScanStats::new()),
            start_time: Instant::now(),
            last_update: AtomicU64::new(0),
        }
    }

    /// Gets the shared scan stats for parallel updates
    pub fn get_scan_stats(&self) -> Arc<ScanStats> {
        Arc::clone(&self.scan_stats)
    }

    /// Increments the directory count for scanning (called from parallel threads)
    pub fn inc_dirs(&self) {
        self.scan_stats.inc_dir();
        self.maybe_update_display();
    }

    /// Called from parallel threads to update scan progress
    pub fn inc_entry(&self, is_dir: bool) {
        self.scan_stats.inc_entry();
        if is_dir {
            self.scan_stats.inc_dir();
        } else {
            self.scan_stats.inc_file();
        }
        self.maybe_update_display();
    }

    /// Throttled display update (avoids UI thrashing)
    fn maybe_update_display(&self) {
        let now_ms = self.start_time.elapsed().as_millis() as u64;
        let last = self.last_update.load(Ordering::Relaxed);
        // Update at most every 50ms
        if now_ms.saturating_sub(last) > 50
            && self.last_update.compare_exchange(last, now_ms, Ordering::Relaxed, Ordering::Relaxed).is_ok()
        {
            self.update_scan_display();
        }
    }

    /// Updates the scanning display with current statistics
    fn update_scan_display(&self) {
        let stats = &self.scan_stats;
        let entries = stats.entries();
        let dirs = stats.dirs();
        let matched = self.category_tracker.total_count();
        let matched_size = self.category_tracker.total_size();
        let elapsed = self.start_time.elapsed().as_secs_f64();

        // Calculate scan rate
        let rate = if elapsed > 0.0 {
            (entries as f64 / elapsed) as usize
        } else {
            0
        };

        let line1 = format!(
            "{}  {} found ({}) • {} entries ({}/s)",
            "Scanning".bright_blue(),
            matched.to_string().bright_white(),
            format_size(matched_size, DECIMAL).bright_green(),
            dirs.to_string().dimmed(),
            rate.to_string().dimmed()
        );

        let line2 = self.category_tracker.format_breakdown();

        // Combine into message
        if line2.is_empty() {
            self.bar.set_message(line1);
        } else {
            self.bar.set_message(format!("{}\n  {}", line1, line2));
        }
    }

    /// Force a final display update
    pub fn force_update(&self) {
        self.update_scan_display();
    }

    pub fn get_tracker(&self) -> Arc<CategoryTracker> {
        Arc::clone(&self.category_tracker)
    }

    /// Returns the elapsed time since creation
    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }

    /// Returns total entries scanned
    pub fn entries_scanned(&self) -> usize {
        self.scan_stats.entries()
    }
}

impl Progress for CompactDisplay {
    fn increment(&self, _delta: u64) {
        self.bar.inc(1);
    }

    fn set_message(&self, msg: &str) {
        self.bar.set_message(msg.to_string());
    }

    fn finish(&self) {
        self.bar.finish_and_clear();
    }
}

