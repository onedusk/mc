pub mod scanner;
pub mod cleaner;

pub use scanner::Scanner;
pub use cleaner::{ParallelCleaner, Statistics};

use crate::types::CleanItem;
use std::path::Path;

/// Prunes nested items from a list of CleanItems.
///
/// When a directory is marked for deletion, all items within it will be deleted
/// automatically via `fs::remove_dir_all`. This function removes redundant nested
/// items to:
/// - Improve performance (fewer items to process)
/// - Prevent race conditions in parallel deletion
/// - Provide clearer output to users
///
/// # Algorithm
///
/// 1. Sort items by path length (shortest first)
/// 2. For each item, check if any ancestor path is already in the kept list
/// 3. Keep only items that don't have an ancestor marked for deletion
///
/// # Example
///
/// ```text
/// Input:
///   - /project/node_modules (5 GB)
///   - /project/node_modules/pkg1/dist (50 MB)
///   - /project/node_modules/pkg2/build (30 MB)
///   - /project/dist (200 MB)
///
/// Output:
///   - /project/node_modules (5 GB)
///   - /project/dist (200 MB)
/// ```
///
/// The nested items under `node_modules/` are removed because they'll be
/// deleted when `node_modules/` is removed.
pub fn prune_nested_items(mut items: Vec<CleanItem>) -> Vec<CleanItem> {
    if items.is_empty() {
        return items;
    }

    // Sort by path depth (component count) - shortest paths first
    items.sort_by(|a, b| {
        let a_depth = a.path.components().count();
        let b_depth = b.path.components().count();
        a_depth.cmp(&b_depth).then_with(|| a.path.cmp(&b.path))
    });

    let mut pruned = Vec::new();

    for item in items {
        // Check if any item in our pruned list is an ancestor of this item
        let has_ancestor = pruned.iter().any(|kept: &CleanItem| {
            is_ancestor(&kept.path, &item.path)
        });

        if !has_ancestor {
            pruned.push(item);
        }
    }

    pruned
}

/// Checks if `ancestor` is an ancestor (parent, grandparent, etc.) of `descendant`.
fn is_ancestor(ancestor: &Path, descendant: &Path) -> bool {
    // If paths are equal, ancestor is not strictly an ancestor
    if ancestor == descendant {
        return false;
    }

    // Check if descendant starts with ancestor
    descendant.starts_with(ancestor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ItemType, PatternMatch, PatternSource};
    use std::path::PathBuf;

    fn make_item(path: &str, size: u64) -> CleanItem {
        CleanItem {
            path: PathBuf::from(path),
            size,
            item_type: ItemType::Directory,
            pattern: PatternMatch {
                pattern: "test".to_string(),
                priority: 0,
                source: PatternSource::BuiltIn,
            },
        }
    }

    #[test]
    fn test_prune_nested_items() {
        let items = vec![
            make_item("/project/node_modules/pkg1/dist", 50_000_000),
            make_item("/project/node_modules", 5_000_000_000),
            make_item("/project/node_modules/pkg2/build", 30_000_000),
            make_item("/project/dist", 200_000_000),
            make_item("/project/dist/subdir", 100_000_000),
        ];

        let pruned = prune_nested_items(items);

        // Should keep only top-level items
        // /project/node_modules (keeps, prunes nested pkg1/dist and pkg2/build)
        // /project/dist (keeps, prunes nested subdir)
        assert_eq!(pruned.len(), 2);
        assert!(pruned.iter().any(|i| i.path == PathBuf::from("/project/node_modules")));
        assert!(pruned.iter().any(|i| i.path == PathBuf::from("/project/dist")));

        // Verify nested items were pruned
        assert!(!pruned.iter().any(|i| i.path == PathBuf::from("/project/node_modules/pkg1/dist")));
        assert!(!pruned.iter().any(|i| i.path == PathBuf::from("/project/dist/subdir")));
    }

    #[test]
    fn test_prune_preserves_non_nested() {
        let items = vec![
            make_item("/project/node_modules", 1000),
            make_item("/project/dist", 2000),
            make_item("/other/target", 3000),
        ];

        let pruned = prune_nested_items(items);

        // All should be kept as none are nested
        assert_eq!(pruned.len(), 3);
    }

    #[test]
    fn test_prune_empty_list() {
        let items: Vec<CleanItem> = vec![];
        let pruned = prune_nested_items(items);
        assert_eq!(pruned.len(), 0);
    }

    #[test]
    fn test_is_ancestor() {
        assert!(is_ancestor(
            &PathBuf::from("/a/b"),
            &PathBuf::from("/a/b/c")
        ));
        assert!(is_ancestor(
            &PathBuf::from("/a"),
            &PathBuf::from("/a/b/c/d")
        ));
        assert!(!is_ancestor(
            &PathBuf::from("/a/b"),
            &PathBuf::from("/a/b")
        ));
        assert!(!is_ancestor(
            &PathBuf::from("/a/b"),
            &PathBuf::from("/a/c")
        ));
        assert!(!is_ancestor(
            &PathBuf::from("/x"),
            &PathBuf::from("/y")
        ));
    }
}