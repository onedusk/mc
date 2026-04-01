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
    pub fn validate(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            bail!("Path does not exist: {}", path.display());
        }

        if self.check_git && self.is_git_repo(path) {
            eprintln!("⚠️  Warning: Path is inside a git repository.");
            eprintln!("   Use --no-git-check to override this safety check.");
            bail!("Aborting due to git repository detection");
        }

        self.check_disk_space(path)?;

        Ok(())
    }

    /// Checks if a path is inside a git repository by looking for a `.git` directory
    /// in the path's ancestors.
    fn is_git_repo(&self, path: &Path) -> bool {
        log::debug!("Checking git repo at {}", path.display());
        path.ancestors().any(|p| p.join(".git").exists())
    }

    /// Checks that free disk space meets the configured minimum.
    fn check_disk_space(&self, path: &Path) -> Result<()> {
        let free = self.get_free_space(path)?;
        if free < self.min_free_space {
            bail!(
                "Insufficient disk space. Have {} GB free, need at least {} GB",
                free / 1_000_000_000,
                self.min_free_space / 1_000_000_000
            );
        }
        log::debug!(
            "Disk space check passed: {} GB free (minimum: {} GB)",
            free / 1_000_000_000,
            self.min_free_space / 1_000_000_000
        );
        Ok(())
    }

    /// Gets free disk space via statvfs on Unix.
    #[cfg(unix)]
    fn get_free_space(&self, path: &Path) -> Result<u64> {
        use std::ffi::CString;
        use std::mem::MaybeUninit;
        use std::os::unix::ffi::OsStrExt;

        let c_path = CString::new(path.as_os_str().as_bytes())
            .map_err(|_| anyhow::anyhow!("Invalid path for statvfs"))?;

        let mut stat = MaybeUninit::<libc::statvfs>::uninit();
        // SAFETY: c_path is a valid null-terminated C string, stat is properly aligned
        let ret = unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) };
        if ret != 0 {
            let err = std::io::Error::last_os_error();
            log::warn!("statvfs failed for {}: {}", path.display(), err);
            return Ok(u64::MAX); // Fail open — don't block cleaning
        }

        // SAFETY: statvfs returned 0, so stat is initialized
        let stat = unsafe { stat.assume_init() };
        #[allow(clippy::unnecessary_cast)]
        let free_bytes = (stat.f_bavail as u64).saturating_mul(stat.f_frsize as u64);
        Ok(free_bytes)
    }

    /// Stub for Windows — returns u64::MAX with a warning.
    #[cfg(windows)]
    fn get_free_space(&self, _path: &Path) -> Result<u64> {
        log::warn!("Disk space check not implemented on Windows");
        Ok(u64::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_nonexistent_path() {
        let guard = SafetyGuard::new(false, 10, 1.0);
        let result = guard.validate(Path::new("/nonexistent/path/abc123"));
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("does not exist"), "got: {}", msg);
    }

    #[test]
    fn test_is_git_repo_detects_git_dir() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(temp.path().join(".git")).unwrap();

        let guard = SafetyGuard::new(true, 10, 0.0);
        let result = guard.validate(temp.path());
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("git repository"), "got: {}", msg);
    }

    #[test]
    fn test_is_git_repo_returns_false_without_git() {
        let temp = tempfile::TempDir::new().unwrap();
        // No .git dir — guard should pass (check_git=true but no repo found)
        let guard = SafetyGuard::new(true, 10, 0.0);
        let result = guard.validate(temp.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_disk_space_passes_when_sufficient() {
        let temp = tempfile::TempDir::new().unwrap();
        let guard = SafetyGuard::new(false, 10, 0.0);
        let result = guard.validate(temp.path());
        assert!(result.is_ok());
    }

    #[test]
    #[cfg(unix)] // Windows stub returns u64::MAX, so this test only works on Unix
    fn test_check_disk_space_fails_when_insufficient() {
        let temp = tempfile::TempDir::new().unwrap();
        let guard = SafetyGuard::new(false, 10, 999_999.0);
        let result = guard.validate(temp.path());
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Insufficient disk space"), "got: {}", msg);
    }
}
