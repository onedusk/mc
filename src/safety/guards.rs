//! This module provides safety checks to prevent accidental data loss.
//!
//! The `SafetyGuard` is used to validate a path before any cleaning operations
//! are performed. It can check for things like the presence of a git repository
//! or sufficient free disk space. These checks are designed to be fail-safe,
//! aborting the operation if any potential risks are detected.

use anyhow::{bail, Result};
use std::path::Path;

/// A guard that performs safety checks before cleaning.
///
/// It is initialized based on the `SafetyConfig` section of the configuration.
pub struct SafetyGuard {
    /// Enables or disables the git repository check.
    check_git: bool,
    /// The maximum scan depth (passed from config, but not used directly in guards).
    _max_depth: usize,
    /// The minimum free space in bytes required on the disk.
    min_free_space: u64,
}

impl SafetyGuard {
    /// Creates a new `SafetyGuard`.
    ///
    /// # Arguments
    ///
    /// * `check_git` - Whether to check for a git repository.
    /// * `max_depth` - The maximum scan depth (currently unused in guard).
    /// * `min_free_space_gb` - The minimum required free disk space in gigabytes.
    pub fn new(check_git: bool, max_depth: usize, min_free_space_gb: f64) -> Self {
        Self {
            check_git,
            _max_depth: max_depth,
            min_free_space: (min_free_space_gb * 1_000_000_000.0) as u64,
        }
    }

    /// Validates the given path against the configured safety checks.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to validate.
    ///
    /// # Returns
    ///
    /// `Ok(())` if all checks pass, otherwise an `Err` with a descriptive message.
    /// The error type is `anyhow::Error` to accommodate different failure modes.
    pub fn validate(&self, path: &Path) -> Result<()> {
        // Check if path exists
        if !path.exists() {
            bail!("Path does not exist: {}", path.display());
        }

        // Check if it's a git repository
        if self.check_git && self.is_git_repo(path) {
            println!("⚠️  Warning: Path is inside a git repository.");
            println!("   Use --no-git-check to override this safety check.");
            bail!("Aborting due to git repository detection");
        }

        // Check available disk space
        #[cfg(unix)]
        {
            if let Ok(space) = self.get_free_space(path) {
                if space < self.min_free_space {
                    bail!(
                        "Insufficient disk space. Need at least {} GB free",
                        self.min_free_space / 1_000_000_000
                    );
                }
            }
        }

        Ok(())
    }

    /// Checks if a path is inside a git repository by looking for a `.git` directory
    /// in the path's ancestors.
    ///
    /// This is a crucial safety feature to prevent accidental deletion of a project's
    /// version control history.
    fn is_git_repo(&self, path: &Path) -> bool {
        path.ancestors().any(|p| p.join(".git").exists())
    }

    /// Gets the available free space on the disk where the path is located.
    ///
    /// Note: This is currently a placeholder and does not perform a real check.
    /// In a production implementation, this would use a library like `fs2` to
    /// get accurate disk space information in a cross-platform way.
    #[cfg(unix)]
    fn get_free_space(&self, _path: &Path) -> Result<u64> {
        // Simplified version - in production would use statvfs
        Ok(u64::MAX) // Return max for now to avoid blocking
    }

    /// Gets the available free space on the disk where the path is located.
    ///
    /// Note: This is currently a placeholder and does not perform a real check.
    /// In a production implementation, this would use Windows-specific APIs to
    /// get disk space information.
    #[cfg(windows)]
    fn get_free_space(&self, _path: &Path) -> Result<u64> {
        // Windows implementation placeholder
        Ok(u64::MAX) // Return max for now to avoid blocking
    }
}
