//! This module is responsible for scanning the file system to find items for cleaning.
//!
//! The `Scanner` traverses the file system starting from a root path, using the
//! provided `PatternMatcher` to identify files and directories that should be
//! cleaned. It is designed to work in parallel to efficiently scan large directory trees.
//!
//! # Implementation
//!
//! The scanning process streams directory entries using `walkdir` and the
//! `rayon::par_bridge` adaptor so pattern matching and metadata collection can
//! proceed in parallel without first materialising the entire tree in memory.

use crate::patterns::PatternMatcher;
use crate::types::{CleanItem, ItemType, ScanError};
use crate::utils::progress::{CategoryTracker, Progress};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use walkdir::WalkDir;

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
        let matcher = Arc::clone(&self.matcher);
        let progress = self.progress.clone();
        let category_tracker = self.category_tracker.clone();
        let root = self.root.clone();

        let accumulator = WalkDir::new(&self.root)
            .max_depth(self.max_depth)
            .follow_links(self.follow_symlinks)
            .into_iter()
            .par_bridge()
            .fold(
                || ScanAccumulator::default(),
                |mut acc, entry_result| {
                    match entry_result {
                        Ok(entry) => {
                            let path = entry.path();
                            if path == root {
                                return acc;
                            }

                            let file_type = entry.file_type();

                            let path_buf = path.to_path_buf();
                            let pattern_match = matcher.matches_with_type(path, Some(file_type));

                            let mut file_size = None;
                            let mut metadata_available = true;
                            let mut contributes_to_dir = false;
                            let mut dir_base_size = None;

                            if file_type.is_file() {
                                match entry.metadata() {
                                    Ok(metadata) => {
                                        let size = metadata.len();
                                        file_size = Some(size);
                                        contributes_to_dir = true;
                                    }
                                    Err(err) => {
                                        metadata_available = false;
                                        acc.errors.push(ScanError::IoError {
                                            path: path_buf.clone(),
                                            message: err.to_string(),
                                        });
                                    }
                                }
                            } else if file_type.is_dir() {
                                match entry.metadata() {
                                    Ok(metadata) => {
                                        dir_base_size = Some(metadata.len());
                                    }
                                    Err(err) => {
                                        metadata_available = false;
                                        acc.errors.push(ScanError::IoError {
                                            path: path_buf.clone(),
                                            message: err.to_string(),
                                        });
                                    }
                                }
                            } else if file_type.is_symlink() {
                                match entry.metadata() {
                                    Ok(metadata) => {
                                        file_size = Some(metadata.len());
                                        contributes_to_dir = metadata.is_file();
                                    }
                                    Err(err) => {
                                        metadata_available = false;
                                        acc.errors.push(ScanError::IoError {
                                            path: path_buf.clone(),
                                            message: err.to_string(),
                                        });
                                    }
                                }
                            }

                            let item_type = determine_type(&file_type);

                            if let Some(pattern_match) = pattern_match {
                                if !matches!(item_type, ItemType::File | ItemType::Symlink)
                                    || metadata_available
                                {
                                    if let Some(ref progress) = progress {
                                        progress.increment(1);
                                    }

                                    let size = match item_type {
                                        ItemType::File | ItemType::Symlink => {
                                            file_size.unwrap_or(0)
                                        }
                                        ItemType::Directory => 0,
                                    };

                                    acc.items.push(CleanItem {
                                        path: path_buf,
                                        size,
                                        item_type,
                                        pattern: pattern_match,
                                    });
                                }
                            }

                            if let Some(size) = dir_base_size {
                                acc.dir_bases.push((path.to_path_buf(), size));
                            }

                            // Record file sizes for directory aggregation even when the file
                            // itself does not match a pattern.
                            if contributes_to_dir {
                                if let Some(size) = file_size {
                                    acc.file_sizes.push((path.to_path_buf(), size));
                                }
                            }
                        }
                        Err(err) => {
                            let path = err.path().unwrap_or(&root).to_path_buf();
                            let error = if err.loop_ancestor().is_some() {
                                ScanError::SymlinkCycle { path }
                            } else {
                                ScanError::IoError {
                                    path,
                                    message: err.to_string(),
                                }
                            };
                            acc.errors.push(error);
                        }
                    }

                    acc
                },
            )
            .reduce(
                || ScanAccumulator::default(),
                |mut acc, mut other| {
                    acc.items.append(&mut other.items);
                    acc.errors.append(&mut other.errors);
                    acc.file_sizes.append(&mut other.file_sizes);
                    acc.dir_bases.append(&mut other.dir_bases);
                    acc
                },
            );

        let ScanAccumulator {
            mut items,
            errors,
            file_sizes,
            dir_bases,
        } = accumulator;

        if !items.is_empty() {
            let matched_dirs: HashSet<PathBuf> = items
                .iter()
                .filter_map(|item| {
                    if matches!(item.item_type, ItemType::Directory) {
                        Some(item.path.clone())
                    } else {
                        None
                    }
                })
                .collect();

            if !matched_dirs.is_empty() {
                let mut dir_sizes: HashMap<PathBuf, u64> =
                    matched_dirs.into_iter().map(|path| (path, 0)).collect();

                for (dir_path, base) in dir_bases {
                    if let Some(total) = dir_sizes.get_mut(dir_path.as_path()) {
                        *total += base;
                    }
                }

                for (file_path, size) in file_sizes {
                    for ancestor in file_path.ancestors().skip(1) {
                        if !ancestor.starts_with(&root) {
                            break;
                        }
                        if let Some(total) = dir_sizes.get_mut(ancestor) {
                            *total += size;
                        }
                    }
                }

                for item in &mut items {
                    if matches!(item.item_type, ItemType::Directory) {
                        if let Some(size) = dir_sizes.get(&item.path) {
                            item.size = *size;
                        }
                    }
                }
            }
        }

        if let Some(tracker) = category_tracker {
            for item in &items {
                tracker.add_item(item.pattern.category, item.size);
            }
        }

        Ok((items, errors))
    }
}

#[derive(Default)]
struct ScanAccumulator {
    items: Vec<CleanItem>,
    errors: Vec<ScanError>,
    file_sizes: Vec<(PathBuf, u64)>,
    dir_bases: Vec<(PathBuf, u64)>,
}

fn determine_type(file_type: &fs::FileType) -> ItemType {
    if file_type.is_dir() {
        ItemType::Directory
    } else if file_type.is_symlink() {
        ItemType::Symlink
    } else {
        ItemType::File
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use assert_fs::prelude::*;
    use assert_fs::TempDir;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::{self as unix_fs, PermissionsExt};
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
        assert!(items.iter().any(|item| item.path.ends_with("node_modules")));
        assert!(items.iter().any(|item| item.path.ends_with("target")));
        assert!(items.iter().any(|item| item.path.ends_with("app.log")));
    }

    #[test]
    fn test_permission_error_handling() {
        let temp = TempDir::new().unwrap();
        let restricted_dir = temp.child("restricted");
        restricted_dir.create_dir_all().unwrap();

        // Remove execute permissions so the directory cannot be traversed.
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(restricted_dir.path()).unwrap().permissions();
            perms.set_mode(0o000);
            fs::set_permissions(restricted_dir.path(), perms).unwrap();
        }
        #[cfg(not(unix))]
        {
            let mut perms = fs::metadata(restricted_dir.path()).unwrap().permissions();
            perms.set_readonly(true);
            fs::set_permissions(restricted_dir.path(), perms).unwrap();
        }

        let config = Config::default();
        let matcher = Arc::new(PatternMatcher::new(&config.patterns).unwrap());
        let scanner = Scanner::new(temp.path().to_path_buf(), matcher);

        let (_, errors) = scanner.scan().unwrap();

        assert!(!errors.is_empty());
        assert!(matches!(errors[0], ScanError::IoError { .. }));
    }

    #[cfg(unix)]
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
        let scanner = Scanner::new(temp.path().to_path_buf(), matcher).with_symlinks(true);

        let (_, errors) = scanner.scan().unwrap();

        assert!(!errors.is_empty());
        assert!(matches!(errors[0], ScanError::SymlinkCycle { .. }));
    }
}
