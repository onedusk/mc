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
                            ItemType::Directory => {
                                stats.dirs_deleted.fetch_add(1, Ordering::Relaxed);
                            }
                            _ => {
                                stats.files_deleted.fetch_add(1, Ordering::Relaxed);
                            }
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

        log::debug!(
            "Clean done: {} deleted, {} errors",
            stats.items_deleted.load(Ordering::Relaxed),
            errors.len()
        );

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
                #[cfg(unix)]
                parallel_remove::remove_dir_all_parallel(&item.path)?;
                #[cfg(not(unix))]
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

/// Parallel recursive directory deletion using directory-fd-relative syscalls.
///
/// `fs::remove_dir_all` walks its tree on a single thread, so one huge directory
/// (e.g. a monorepo `node_modules`) becomes the long pole of the whole clean while
/// other pool threads sit idle. Recursing into sibling subdirectories with rayon
/// lets idle threads steal subtrees, keeping every thread busy issuing unlinks.
///
/// All operations use `openat`/`unlinkat` relative to an open directory descriptor,
/// so each syscall resolves a single name instead of the full path. This matches
/// std's per-operation cost and avoids contended lookups through the shared path
/// prefix that plain path-based deletion would incur from every thread.
#[cfg(unix)]
mod parallel_remove {
    use rayon::prelude::*;
    use std::ffi::{CStr, CString, OsStr};
    use std::io;
    use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    /// Deletes the directory at `path` and everything inside it.
    ///
    /// Like `fs::remove_dir_all`, symlinks are unlinked, never followed:
    /// entries are classified by `d_type`/`lstat`, so a symlink to a directory
    /// takes the file branch, and directories are opened with `O_NOFOLLOW`.
    pub fn remove_dir_all_parallel(path: &Path) -> io::Result<()> {
        let c_path = CString::new(path.as_os_str().as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"))?;
        let fd = unsafe {
            libc::open(
                c_path.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        delete_contents(unsafe { OwnedFd::from_raw_fd(fd) }, path)?;
        std::fs::remove_dir(path)
    }

    /// RAII wrapper closing a `DIR` stream (and its underlying fd) on drop.
    struct Dir(*mut libc::DIR);

    impl Dir {
        fn fd(&self) -> RawFd {
            unsafe { libc::dirfd(self.0) }
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            unsafe { libc::closedir(self.0) };
        }
    }

    /// Consumes `dir_fd` and deletes everything inside the directory it refers to.
    /// `path` is only used for the descriptor-exhaustion fallback.
    fn delete_contents(dir_fd: OwnedFd, path: &Path) -> io::Result<()> {
        let dp = unsafe { libc::fdopendir(dir_fd.as_raw_fd()) };
        if dp.is_null() {
            return Err(io::Error::last_os_error());
        }
        // fdopendir took ownership of the descriptor; Dir's closedir releases it.
        std::mem::forget(dir_fd);
        let dir = Dir(dp);

        // Collect the full listing before unlinking: removing entries while the
        // directory stream is being read can cause entries to be skipped.
        let mut files: Vec<CString> = Vec::new();
        let mut subdirs: Vec<CString> = Vec::new();
        loop {
            let entry = unsafe { libc::readdir(dp) };
            if entry.is_null() {
                break;
            }
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
            let bytes = name.to_bytes();
            if bytes == b"." || bytes == b".." {
                continue;
            }
            let is_dir = match unsafe { (*entry).d_type } {
                libc::DT_DIR => true,
                libc::DT_UNKNOWN => {
                    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
                    let rc = unsafe {
                        libc::fstatat(
                            dir.fd(),
                            name.as_ptr(),
                            &mut stat,
                            libc::AT_SYMLINK_NOFOLLOW,
                        )
                    };
                    if rc != 0 {
                        return Err(io::Error::last_os_error());
                    }
                    stat.st_mode & libc::S_IFMT == libc::S_IFDIR
                }
                _ => false,
            };
            if is_dir {
                subdirs.push(name.to_owned());
            } else {
                files.push(name.to_owned());
            }
        }

        // Files in a single directory are unlinked serially (the filesystem
        // serializes same-directory mutations anyway); parallelism comes from
        // sibling subtrees.
        for name in &files {
            if unsafe { libc::unlinkat(dir.fd(), name.as_ptr(), 0) } != 0 {
                return Err(io::Error::last_os_error());
            }
        }

        let parent = unsafe { BorrowedFd::borrow_raw(dir.fd()) };
        match subdirs.len() {
            0 => Ok(()),
            // Skip the parallel machinery for a lone child; deeper levels fan out.
            1 => remove_tree_at(parent, &subdirs[0], path),
            _ => subdirs
                .par_iter()
                .try_for_each(|name| remove_tree_at(parent, name, path)),
        }
    }

    /// Deletes the directory `name` (relative to the open directory `parent`)
    /// and everything inside it.
    fn remove_tree_at(parent: BorrowedFd<'_>, name: &CStr, parent_path: &Path) -> io::Result<()> {
        let fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            let err = io::Error::last_os_error();
            // Each in-flight recursion holds one descriptor; under descriptor
            // exhaustion fall back to std's one-at-a-time deletion for this subtree.
            return match err.raw_os_error() {
                Some(libc::EMFILE) | Some(libc::ENFILE) => {
                    let child = parent_path.join(OsStr::from_bytes(name.to_bytes()));
                    std::fs::remove_dir_all(child)
                }
                _ => Err(err),
            };
        }

        let child_path = parent_path.join(OsStr::from_bytes(name.to_bytes()));
        delete_contents(unsafe { OwnedFd::from_raw_fd(fd) }, &child_path)?;

        if unsafe { libc::unlinkat(parent.as_raw_fd(), name.as_ptr(), libc::AT_REMOVEDIR) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
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

        let items = make_clean_items(&[f1.path(), f2.path(), f3.path()], ItemType::File);

        let cleaner = ParallelCleaner::new().unwrap().with_dry_run(false);
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

        let cleaner = ParallelCleaner::new().unwrap().with_dry_run(true);
        let report = cleaner.clean(items).unwrap();

        assert!(report.dry_run);
        assert_eq!(report.items_deleted, 1);
        assert!(f1.path().exists(), "dry run should not delete files");
    }

    #[test]
    fn test_clean_deletes_nested_directory_tree() {
        let temp = TempDir::new().unwrap();
        let root = temp.child("node_modules");
        for pkg in 0..8 {
            let nested = root.child(format!("pkg_{pkg}/lib/nested"));
            nested.create_dir_all().unwrap();
            nested.child("index.js").touch().unwrap();
            root.child(format!("pkg_{pkg}/package.json"))
                .touch()
                .unwrap();
        }

        let items = make_clean_items(&[root.path()], ItemType::Directory);

        let cleaner = ParallelCleaner::new().unwrap().with_dry_run(false);
        let report = cleaner.clean(items).unwrap();

        assert_eq!(report.items_deleted, 1);
        assert!(report.errors.is_empty());
        assert!(!root.path().exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_clean_does_not_follow_symlinks_out_of_tree() {
        let temp = TempDir::new().unwrap();
        let outside = temp.child("outside");
        outside.create_dir_all().unwrap();
        let keep = outside.child("keep.txt");
        keep.touch().unwrap();

        let doomed = temp.child("doomed");
        doomed.create_dir_all().unwrap();
        std::os::unix::fs::symlink(outside.path(), doomed.child("link").path()).unwrap();

        let items = make_clean_items(&[doomed.path()], ItemType::Directory);

        let cleaner = ParallelCleaner::new().unwrap().with_dry_run(false);
        let report = cleaner.clean(items).unwrap();

        assert!(report.errors.is_empty());
        assert!(!doomed.path().exists());
        assert!(keep.path().exists(), "symlink target contents must survive");
    }

    #[test]
    fn test_clean_collects_errors() {
        let temp = TempDir::new().unwrap();
        // Point to a non-existent file so deletion fails
        let missing = temp.path().join("does_not_exist.log");
        let items = make_clean_items(&[missing.as_path()], ItemType::File);

        let cleaner = ParallelCleaner::new().unwrap().with_dry_run(false);
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
