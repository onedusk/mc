//! This module provides utilities for reporting progress during long-running operations.
//!
//! It defines a `Progress` trait that abstracts the progress reporting mechanism,
//! allowing for different implementations, such as a visual progress bar or a no-op
//! reporter for quiet mode. This decouples the core logic from the specifics of
//! the UI representation.

use indicatif::{ProgressBar, ProgressStyle};

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