//! This module is responsible for scanning the file system to find items for cleaning.
//!
//! The `Scanner` traverses the file system starting from a root path, using the
//! provided `PatternMatcher` to identify files and directories that should be
//! cleaned. It is designed to work in parallel to efficiently scan large directory trees.
//!
//! # Implementation
//!
//! The traversal recurses into sibling subdirectories with rayon `par_iter`, so
//! directory reads, pattern matching, and metadata collection all fan out across
//! the thread pool via work-stealing. Directory sizes are aggregated bottom-up
//! from the recursion's return values, which avoids materialising per-file size
//! records for the whole tree.

use crate::patterns::PatternMatcher;
use crate::types::{CleanItem, ItemType, PatternMatch, ScanError};
use crate::utils::progress::{CategoryTracker, Progress, ScanStats};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

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
    /// An optional scan stats tracker for live progress.
    scan_stats: Option<Arc<ScanStats>>,
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
            scan_stats: None,
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

    /// Attaches scan stats for live progress tracking.
    pub fn with_scan_stats(mut self, stats: Arc<ScanStats>) -> Self {
        self.scan_stats = Some(stats);
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
    pub fn scan(&self) -> crate::types::Result<(Vec<CleanItem>, Vec<ScanError>, usize)> {
        log::debug!(
            "Starting scan from {} (max_depth={})",
            self.root.display(),
            self.max_depth
        );

        let ctx = WalkCtx {
            matcher: &self.matcher,
            progress: self.progress.as_deref(),
            scan_stats: self.scan_stats.as_deref(),
            max_depth: self.max_depth,
            follow_symlinks: self.follow_symlinks,
            items: Mutex::new(Vec::new()),
            errors: Mutex::new(Vec::new()),
            // The root itself counts as one scanned entry.
            entries: AtomicUsize::new(1),
        };

        if self.max_depth > 0 {
            // Cycle detection only matters when following symlinks; the chain
            // starts at the root so a link pointing back at it is caught.
            let root_link = if self.follow_symlinks {
                fs::metadata(&self.root)
                    .ok()
                    .and_then(|md| dir_node(&md))
                    .map(|node| AncestorLink { node, parent: None })
            } else {
                None
            };
            ctx.walk_children(&self.root, 1, root_link.as_ref());
        }

        let items = ctx.items.into_inner().unwrap_or_else(|e| e.into_inner());
        let errors = ctx.errors.into_inner().unwrap_or_else(|e| e.into_inner());

        if let Some(tracker) = &self.category_tracker {
            for item in &items {
                tracker.add_item(item.pattern.category, item.size);
            }
        }

        let entries_scanned = ctx.entries.load(Ordering::Relaxed);
        log::debug!(
            "Scan complete: {} entries scanned, {} items matched",
            entries_scanned,
            items.len()
        );
        Ok((items, errors, entries_scanned))
    }
}

/// Shared state for one recursive scan: matcher, reporters, and the
/// thread-safe accumulators the parallel walk pushes into.
struct WalkCtx<'a> {
    matcher: &'a PatternMatcher,
    progress: Option<&'a dyn Progress>,
    scan_stats: Option<&'a ScanStats>,
    max_depth: usize,
    follow_symlinks: bool,
    items: Mutex<Vec<CleanItem>>,
    errors: Mutex<Vec<ScanError>>,
    entries: AtomicUsize,
}

/// A subdirectory queued for descent, with everything needed to emit its item
/// once the subtree size is known.
struct Subdir {
    path: PathBuf,
    matched: Option<PatternMatch>,
    base_size: u64,
    node: Option<(u64, u64)>,
}

/// Stack-allocated chain of (device, inode) pairs for the directories on the
/// current descent path, used to detect symlink cycles without heap allocation.
#[derive(Clone, Copy)]
struct AncestorLink<'a> {
    node: (u64, u64),
    parent: Option<&'a AncestorLink<'a>>,
}

impl AncestorLink<'_> {
    fn contains(&self, node: (u64, u64)) -> bool {
        self.node == node || self.parent.is_some_and(|p| p.contains(node))
    }
}

#[cfg(unix)]
fn dir_node(metadata: &fs::Metadata) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    Some((metadata.dev(), metadata.ino()))
}

#[cfg(not(unix))]
fn dir_node(_metadata: &fs::Metadata) -> Option<(u64, u64)> {
    // No stable identity available; recursion is still bounded by max_depth.
    None
}

impl WalkCtx<'_> {
    fn push_error(&self, error: ScanError) {
        self.errors
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(error);
    }

    fn record_item(&self, item: CleanItem) {
        if let Some(progress) = self.progress {
            progress.increment(1);
        }
        if let Some(stats) = self.scan_stats {
            stats.inc_matched(item.size);
        }
        self.items
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(item);
    }

    /// Walks the children of `dir` (which sit at `depth`), recursing into
    /// subdirectories in parallel. Returns the total size in bytes of the
    /// files in the subtree, which ancestors use for directory size totals.
    fn walk_children(&self, dir: &Path, depth: usize, chain: Option<&AncestorLink<'_>>) -> u64 {
        let reader = match fs::read_dir(dir) {
            Ok(reader) => reader,
            Err(err) => {
                self.entries.fetch_add(1, Ordering::Relaxed);
                self.push_error(ScanError::IoError {
                    path: dir.to_path_buf(),
                    message: err.to_string(),
                });
                return 0;
            }
        };

        let mut file_bytes: u64 = 0;
        let mut subdirs: Vec<Subdir> = Vec::new();

        for entry_result in reader {
            self.entries.fetch_add(1, Ordering::Relaxed);

            let entry = match entry_result {
                Ok(entry) => entry,
                Err(err) => {
                    self.push_error(ScanError::IoError {
                        path: dir.to_path_buf(),
                        message: err.to_string(),
                    });
                    continue;
                }
            };

            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(err) => {
                    self.push_error(ScanError::IoError {
                        path,
                        message: err.to_string(),
                    });
                    continue;
                }
            };

            if let Some(stats) = self.scan_stats {
                stats.inc_entry();
                if file_type.is_dir() {
                    stats.inc_dir();
                } else {
                    stats.inc_file();
                }
            }

            let pattern_match = self.matcher.matches_with_type(&path, Some(file_type));

            if file_type.is_dir() {
                let (base_size, node) = match entry.metadata() {
                    Ok(metadata) => (metadata.len(), dir_node(&metadata)),
                    Err(err) => {
                        self.push_error(ScanError::IoError {
                            path: path.clone(),
                            message: err.to_string(),
                        });
                        (0, None)
                    }
                };
                subdirs.push(Subdir {
                    path,
                    matched: pattern_match,
                    base_size,
                    node,
                });
                continue;
            }

            // File or symlink. Without metadata there is no size to report, so
            // the entry produces an error instead of an item.
            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(err) => {
                    self.push_error(ScanError::IoError {
                        path,
                        message: err.to_string(),
                    });
                    continue;
                }
            };

            let mut size = metadata.len();
            let mut contributes = metadata.is_file();

            if file_type.is_symlink() && self.follow_symlinks {
                if let Ok(target) = fs::metadata(&path) {
                    if target.is_dir() {
                        // Descend into the linked directory for size aggregation,
                        // unless doing so would revisit an ancestor.
                        match dir_node(&target) {
                            Some(node) if chain.is_some_and(|c| c.contains(node)) => {
                                self.push_error(ScanError::SymlinkCycle { path: path.clone() });
                            }
                            node => subdirs.push(Subdir {
                                path: path.clone(),
                                matched: None,
                                base_size: 0,
                                node,
                            }),
                        }
                    } else {
                        size = target.len();
                        contributes = target.is_file();
                    }
                }
            }

            if contributes {
                file_bytes += size;
            }

            if let Some(pattern) = pattern_match {
                let item_type = if file_type.is_symlink() {
                    ItemType::Symlink
                } else {
                    ItemType::File
                };
                self.record_item(CleanItem {
                    path,
                    size,
                    item_type,
                    pattern,
                });
            }
        }

        let descend = |sub: Subdir| -> u64 {
            let subtree = if depth < self.max_depth {
                let link_storage;
                let child_chain = match sub.node {
                    Some(node) if self.follow_symlinks => {
                        link_storage = AncestorLink {
                            node,
                            parent: chain,
                        };
                        Some(&link_storage)
                    }
                    _ => chain,
                };
                self.walk_children(&sub.path, depth + 1, child_chain)
            } else {
                0
            };

            if let Some(pattern) = sub.matched {
                self.record_item(CleanItem {
                    path: sub.path,
                    size: sub.base_size + subtree,
                    item_type: ItemType::Directory,
                    pattern,
                });
            }
            subtree
        };

        let child_bytes = match subdirs.len() {
            0 => 0,
            // Skip the parallel machinery for a lone child; deeper levels fan out.
            1 => descend(subdirs.pop().expect("len checked")),
            _ => subdirs.into_par_iter().map(descend).sum(),
        };

        file_bytes + child_bytes
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

        let (items, errors, entries_scanned) = scanner.scan().unwrap();

        assert_eq!(items.len(), 3);
        assert!(errors.is_empty());
        assert!(entries_scanned > 0);
        assert!(items.iter().any(|item| item.path.ends_with("node_modules")));
        assert!(items.iter().any(|item| item.path.ends_with("target")));
        assert!(items.iter().any(|item| item.path.ends_with("app.log")));
    }

    #[test]
    fn test_directory_size_aggregation() {
        let temp = TempDir::new().unwrap();
        temp.child("node_modules/pkg/a.js")
            .write_binary(&[0u8; 1000])
            .unwrap();
        temp.child("node_modules/pkg/deep/b.js")
            .write_binary(&[0u8; 500])
            .unwrap();
        temp.child("src/main.rs").write_binary(&[0u8; 300]).unwrap();

        let config = Config::default();
        let matcher = Arc::new(PatternMatcher::new(&config.patterns).unwrap());
        let scanner = Scanner::new(temp.path().to_path_buf(), matcher);

        let (items, errors, _) = scanner.scan().unwrap();

        assert!(errors.is_empty());
        let node_modules = items
            .iter()
            .find(|item| item.path.ends_with("node_modules"))
            .expect("node_modules should match");
        // Files beneath must be aggregated (dir entries themselves may add a
        // few extra bytes depending on the filesystem).
        assert!(
            node_modules.size >= 1500,
            "expected aggregated size >= 1500, got {}",
            node_modules.size
        );
        // The unmatched src/main.rs must not leak into the total.
        assert!(node_modules.size < 1800);
    }

    #[test]
    fn test_max_depth_limits_scan() {
        let temp = TempDir::new().unwrap();
        temp.child("a/b/node_modules/pkg.js").touch().unwrap();

        let config = Config::default();
        let matcher = Arc::new(PatternMatcher::new(&config.patterns).unwrap());

        // Depth 3 reaches a/b/node_modules; depth 2 must not.
        let scanner =
            Scanner::new(temp.path().to_path_buf(), Arc::clone(&matcher)).with_max_depth(3);
        let (items, _, _) = scanner.scan().unwrap();
        assert!(items.iter().any(|i| i.path.ends_with("node_modules")));

        let scanner = Scanner::new(temp.path().to_path_buf(), matcher).with_max_depth(2);
        let (items, _, _) = scanner.scan().unwrap();
        assert!(items.is_empty());
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

        let (_, errors, _) = scanner.scan().unwrap();

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

        let (_, errors, _) = scanner.scan().unwrap();

        assert!(!errors.is_empty());
        assert!(matches!(errors[0], ScanError::SymlinkCycle { .. }));
    }
}
