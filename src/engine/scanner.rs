use walkdir::{WalkDir, DirEntry};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::fs;
use dashmap::DashMap;
use crate::types::{CleanItem, ItemType};
use crate::patterns::PatternMatcher;
use crate::utils::progress::Progress;

pub struct Scanner {
    root: PathBuf,
    matcher: Arc<PatternMatcher>,
    max_depth: usize,
    follow_symlinks: bool,
    progress: Option<Arc<dyn Progress>>,
}

impl Scanner {
    pub fn new(root: PathBuf, matcher: Arc<PatternMatcher>) -> Self {
        Self {
            root,
            matcher,
            max_depth: 10,
            follow_symlinks: false,
            progress: None,
        }
    }

    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = depth;
        self
    }

    pub fn with_symlinks(mut self, follow: bool) -> Self {
        self.follow_symlinks = follow;
        self
    }

    pub fn with_progress(mut self, progress: Arc<dyn Progress>) -> Self {
        self.progress = Some(progress);
        self
    }

    pub fn scan(&self) -> crate::types::Result<Vec<CleanItem>> {
        let items = Arc::new(DashMap::new());
        let items_clone = items.clone();

        // Collect entries first to enable parallel processing
        let entries: Vec<_> = WalkDir::new(&self.root)
            .max_depth(self.max_depth)
            .follow_links(self.follow_symlinks)
            .into_iter()
            .filter_map(|e| e.ok())
            .collect();

        // Process entries in parallel
        entries
            .par_iter()
            .for_each(|entry| {
                if let Some(item) = self.process_entry(entry) {
                    items_clone.insert(item.path.clone(), item);
                    if let Some(ref progress) = self.progress {
                        progress.increment(1);
                    }
                }
            });

        // Convert Arc<DashMap> to Vec
        let result: Vec<CleanItem> = items
            .iter()
            .map(|entry| entry.value().clone())
            .collect();

        Ok(result)
    }

    fn process_entry(&self, entry: &DirEntry) -> Option<CleanItem> {
        let path = entry.path();

        // Skip the root directory itself
        if path == self.root {
            return None;
        }

        if let Some(pattern_match) = self.matcher.matches(path) {
            let metadata = entry.metadata().ok()?;

            Some(CleanItem {
                path: path.to_path_buf(),
                size: self.calculate_size(path, &metadata),
                item_type: self.determine_type(&metadata),
                pattern: pattern_match,
            })
        } else {
            None
        }
    }

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