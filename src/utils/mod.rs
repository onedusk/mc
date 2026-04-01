pub mod progress;

pub use progress::{
    CategoryTracker, CompactDisplay, NoOpProgress, Progress, ProgressReporter, ScanStats,
};

/// Returns the number of available logical CPU cores.
/// Falls back to 4 if the OS query fails.
pub fn available_parallelism() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

/// Clamps a thread count to a valid range [1, available_parallelism()].
pub fn clamp_parallelism(requested: usize) -> usize {
    let max = available_parallelism();
    requested.clamp(1, max)
}
