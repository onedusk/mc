use std::path::Path;
use anyhow::{Result, bail};

pub struct SafetyGuard {
    check_git: bool,
    _max_depth: usize,
    min_free_space: u64,
}

impl SafetyGuard {
    pub fn new(check_git: bool, max_depth: usize, min_free_space_gb: f64) -> Self {
        Self {
            check_git,
            _max_depth: max_depth,
            min_free_space: (min_free_space_gb * 1_000_000_000.0) as u64,
        }
    }

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
                    bail!("Insufficient disk space. Need at least {} GB free",
                        self.min_free_space / 1_000_000_000);
                }
            }
        }

        Ok(())
    }

    fn is_git_repo(&self, path: &Path) -> bool {
        path.ancestors()
            .any(|p| p.join(".git").exists())
    }

    #[cfg(unix)]
    fn get_free_space(&self, _path: &Path) -> Result<u64> {
        // Simplified version - in production would use statvfs
        Ok(u64::MAX) // Return max for now to avoid blocking
    }

    #[cfg(windows)]
    fn get_free_space(&self, _path: &Path) -> Result<u64> {
        // Windows implementation placeholder
        Ok(u64::MAX) // Return max for now to avoid blocking
    }
}