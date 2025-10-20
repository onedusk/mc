//! This module is responsible for scanning the file system to find items for cleaning.
//!
//! The `Scanner` traverses the file system starting from a root path, using the
//! provided `PatternMatcher` to identify files and directories that should be
//! cleaned. It is designed to work in parallel to efficiently scan large directory trees.
//!
//! # Implementation
//!
//! The scanning process is parallelized by first collecting all directory entries
//! into a vector using `walkdir`, and then iterating over this collection in parallel
//! with `rayon`. This allows multiple entries to be processed for pattern matching
//! concurrently. The results are stored in a `DashMap` to handle concurrent writes
//! from multiple threads.

use crate::patterns::PatternMatcher;
use walkdir::{DirEntry, WalkDir};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::fs;
use dashmap::DashMap;
use crate::types::{CleanItem, ItemType, ScanError};
use crate::utils::progress::{Progress, CategoryTracker};

/// A file system scanner that identifies items to be cleaned.
///
/// The `Scanner` walks the directory tree, applying matching rules to find
/// files and directories that are candidates for deletion. It can be configured
/// with a maximum scan depth and whether to follow symbolic links.
pub struct Scanner {
    /// The starting point of the scan.
    root: PathBuf,
    /// The compiled patterns to match against.
    matcher: Arc<PatternMatcher>,
    /// The maximum directory depth to traverse.
    max_depth: usize,
    /// Whether to follow symbolic links during the scan.
    follow_symlinks: bool,
    /// An optional progress reporter.
    progress: Option<Arc<dyn Progress>>,
    /// An optional category tracker for aggregating statistics.
    category_tracker: Option<Arc<CategoryTracker>>,
}

impl Scanner {
    /// Creates a new `Scanner`.
    ///
    /// # Arguments
    ///
    /// * `root` - The root directory to start scanning from.
    /// * `matcher` - An `Arc` wrapped `PatternMatcher` to identify items to clean.
    pub fn new(root: PathBuf, matcher: Arc<PatternMatcher>) -> Self {
        Self {
            root,
            matcher,
            max_depth: 10,
            follow_symlinks: false,
            progress: None,
            category_tracker: None,
        }
    }

    /// Sets the maximum depth for the directory traversal.
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    /// Sets whether the scanner should follow symbolic links.
    pub fn with_symlinks(mut self, follow: bool) -> Self {
        self.follow_symlinks = follow;
        self
    }

    /// Attaches a progress reporter to the scanner.
    pub fn with_progress(mut self, progress: Arc<dyn Progress>) -> Self {
        self.progress = Some(progress);
        self
    }

    /// Attaches a category tracker to the scanner.
    pub fn with_category_tracker(mut self, tracker: Arc<CategoryTracker>) -> Self {
        self.category_tracker = Some(tracker);
        self
    }

    /// Performs the file system scan.
    ///
    /// This method walks the directory tree from the root, processes entries in parallel,
    /// and returns a vector of `CleanItem`s that match the cleaning patterns.
    ///
    /// # Performance Considerations
    ///
    /// The use of `rayon` for parallel processing can significantly speed up the scanning
    /// of large directories with many entries, as the pattern matching for each entry
    /// can happen concurrently.
    pub fn scan(&self) -> crate::types::Result<(Vec<CleanItem>, Vec<ScanError>)> {
        let items = Arc::new(DashMap::new());
        let scan_errors = Arc::new(DashMap::new());

        // Collect entries first to enable parallel processing
        let entries: Vec<_> = WalkDir::new(&self.root)
            .max_depth(self.max_depth)
            .follow_links(self.follow_symlinks)
            .into_iter()
            .collect();

        // Process entries in parallel
        entries.par_iter().for_each(|entry_result| {
            match entry_result {
                Ok(entry) => {
                    if let Some(item) = self.process_entry(entry) {
                        items.insert(item.path.clone(), item);
                        if let Some(ref progress) = self.progress {
                            progress.increment(1);
                        }
                    }
                }
                Err(err) => {
                    let path = err.path().unwrap_or(&self.root).to_path_buf();
                    let error = if err.loop_ancestor().is_some() {
                        ScanError::SymlinkCycle { path: path.clone() }
                    } else {
                        ScanError::IoError {
                            path: path.clone(),
                            message: err.to_string(),
                        }
                    };
                    scan_errors.insert(path, error);
                }
            }
        });

        // Convert Arc<DashMap> to Vec
        let result_items: Vec<CleanItem> =
            items.iter().map(|entry| entry.value().clone()).collect();
        let result_errors: Vec<ScanError> = scan_errors
            .iter()
            .map(|entry| entry.value().clone())
            .collect();

        Ok((result_items, result_errors))
    }

    /// Processes a single directory entry to determine if it should be cleaned.
    fn process_entry(&self, entry: &DirEntry) -> Option<CleanItem> {
        let path = entry.path();

        // Skip the root directory itself
        if path == self.root {
            return None;
        }

        if let Some(pattern_match) = self.matcher.matches(path) {
            let metadata = entry.metadata().ok()?;
            let size = self.calculate_size(path, &metadata);

            // Update category tracker if present
            if let Some(ref tracker) = self.category_tracker {
                tracker.add_item(pattern_match.category, size);
            }

            Some(CleanItem {
                path: path.to_path_buf(),
                size,
                item_type: self.determine_type(&metadata),
                pattern: pattern_match,
            })
        } else {
            None
        }
    }

    /// Calculates the size of a file system item.
    ///
    /// For files, it returns the file size. For directories, it recursively calculates
    /// the total size of all files within that directory.
    ///
    /// # Performance Considerations
    ///
    /// Calculating directory size can be an expensive operation, as it requires
    /// traversing the entire subdirectory tree. This is one of the more
    /// time-consuming parts of the scanning phase.
    fn calculate_size(&self, path: &Path, metadata: &fs::Metadata) -> u64 {
        if metadata.is_dir() {
            // Calculate directory size recursively
            WalkDir::new(path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter_map(|e| e.metadata().ok())
                .filter(|m| m.is_file())
                .map(|m| m.len())
                .sum()
        } else {
            metadata.len()
        }
    }

    /// Determines the `ItemType` of a file system item based on its metadata.
    fn determine_type(&self, metadata: &fs::Metadata) -> ItemType {
        if metadata.is_dir() {
            ItemType::Directory
        } else if metadata.is_symlink() {
            ItemType::Symlink
        } else {
            ItemType::File
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use assert_fs::prelude::*;
    use assert_fs::TempDir;
    use std::fs;
    use std::os::unix::fs as unix_fs;
    use std::sync::Arc;

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
    fn test_successful_scan() {
        let temp = setup_test_dir();
        let config = Config::default();
        let matcher = Arc::new(PatternMatcher::new(&config.patterns).unwrap());
        let scanner = Scanner::new(temp.path().to_path_buf(), matcher);

        let (items, errors) = scanner.scan().unwrap();

        assert_eq!(items.len(), 3);
        assert!(errors.is_empty());
        assert!(items
            .iter()
            .any(|item| item.path.ends_with("node_modules")));
        assert!(items.iter().any(|item| item.path.ends_with("target")));
        assert!(items.iter().any(|item| item.path.ends_with("app.log")));
    }

    #[test]
    fn test_permission_error_handling() {
        let temp = TempDir::new().unwrap();
        let restricted_dir = temp.child("restricted");
        restricted_dir.create_dir_all().unwrap();

        // Set permissions to read-only
        let mut perms = fs::metadata(restricted_dir.path()).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(restricted_dir.path(), perms).unwrap();

        let config = Config::default();
        let matcher = Arc::new(PatternMatcher::new(&config.patterns).unwrap());
        let scanner = Scanner::new(temp.path().to_path_buf(), matcher);

        let (_, errors) = scanner.scan().unwrap();

        assert!(!errors.is_empty());
        assert!(matches!(errors[0], ScanError::IoError { .. }));
    }

    #[test]
    fn test_symlink_cycle_detection() {
        let temp = TempDir::new().unwrap();
        let dir_a = temp.child("a");
        let dir_b = dir_a.child("b");
        dir_b.create_dir_all().unwrap();
        let symlink_path = dir_b.child("cycle");

        unix_fs::symlink("../..", symlink_path.path()).unwrap();

        let config = Config::default();
        let matcher = Arc::new(PatternMatcher::new(&config.patterns).unwrap());
        let scanner = Scanner::new(temp.path().to_path_buf(), matcher);

        let (_, errors) = scanner.scan().unwrap();

        assert!(!errors.is_empty());
        assert!(matches!(errors[0], ScanError::SymlinkCycle { .. }));
    }
}