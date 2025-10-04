# Technical Specification
# Mr. Cleann (mc) - Implementation Details

## 1. Technology Stack

### Core Dependencies
```toml
[dependencies]
clap = { version = "4.5", features = ["derive", "env"] }
rayon = "1.10"
walkdir = "2.5"
glob = "0.3"
toml = "0.8"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
anyhow = "1.0"
thiserror = "1.0"
indicatif = "0.17"
colored = "2.1"
dashmap = "6.0"
crossbeam-channel = "0.5"
chrono = "0.4"
humansize = "2.1"
directories = "5.0"
once_cell = "1.19"
regex = "1.10"
log = "0.4"
env_logger = "0.11"

[dev-dependencies]
tempfile = "3.10"
criterion = "0.5"
proptest = "1.4"
assert_fs = "1.1"
predicates = "3.1"
```

## 2. Implementation Plan

### Phase 1: Core Foundation (Week 1)

#### 2.1.1 Project Setup
```bash
# Initialize project
cargo new mc --bin
cd mc

# Setup workspace structure
mkdir -p src/{cli,config,engine,patterns,parallel,safety,utils}
mkdir -p tests/{unit,integration}
mkdir -p benches
mkdir -p docs
```

#### 2.1.2 Core Types and Structures

```rust
// src/types.rs
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub struct CleanItem {
    pub path: PathBuf,
    pub size: u64,
    pub item_type: ItemType,
    pub pattern: PatternMatch,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemType {
    Directory,
    File,
    Symlink,
}

#[derive(Debug, Clone)]
pub struct PatternMatch {
    pub pattern: String,
    pub priority: u32,
    pub source: PatternSource,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PatternSource {
    BuiltIn,
    Config,
    CLI,
}

#[derive(Debug, Default, Serialize)]
pub struct CleanReport {
    pub items_deleted: usize,
    pub bytes_freed: u64,
    pub errors: Vec<CleanError>,
    pub duration: std::time::Duration,
    pub dry_run: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum CleanError {
    #[error("Permission denied: {path}")]
    PermissionDenied { path: PathBuf },

    #[error("IO error at {path}: {source}")]
    IoError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Pattern error: {0}")]
    PatternError(String),
}
```

### Phase 2: Pattern System (Week 1-2)

#### 2.2.1 Pattern Definitions

```rust
// src/patterns/builtin.rs
use once_cell::sync::Lazy;

pub static BUILTIN_PATTERNS: Lazy<PatternSet> = Lazy::new(|| {
    PatternSet {
        directories: vec![
            // Build outputs
            "dist", "build", ".next", "out", "target",
            // Dependencies
            "node_modules", ".venv", "vendor",
            // Cache
            ".turbo", ".bun", ".pytest_cache", ".benchmark-cache",
            "coverage", ".ropeproject", ".ruby-lsp",
            // Tools
            ".idea", ".flock", ".swarm", ".hive-mind",
            ".claude-flow", ".roo", "memory", "coordination",
        ],
        files: vec![
            "*.log",
            "*.tsbuildinfo",
            "package-lock.json",
            "bun.lock",
            "uv.lock",
            "Gemfile.lock",
        ],
        exclude: vec![
            ".git",
            ".github",
        ],
    }
});

pub struct PatternSet {
    pub directories: Vec<&'static str>,
    pub files: Vec<&'static str>,
    pub exclude: Vec<&'static str>,
}
```

#### 2.2.2 Pattern Matcher

```rust
// src/patterns/matcher.rs
use glob::{Pattern, PatternError};
use std::path::Path;

pub struct PatternMatcher {
    directory_patterns: Vec<Pattern>,
    file_patterns: Vec<Pattern>,
    exclude_patterns: Vec<Pattern>,
}

impl PatternMatcher {
    pub fn new(config: &PatternConfig) -> Result<Self, PatternError> {
        Ok(Self {
            directory_patterns: Self::compile_patterns(&config.directories)?,
            file_patterns: Self::compile_patterns(&config.files)?,
            exclude_patterns: Self::compile_patterns(&config.exclude)?,
        })
    }

    fn compile_patterns(patterns: &[String]) -> Result<Vec<Pattern>, PatternError> {
        patterns.iter()
            .map(|p| Pattern::new(p))
            .collect()
    }

    pub fn matches(&self, path: &Path) -> Option<PatternMatch> {
        // Check exclusions first
        if self.is_excluded(path) {
            return None;
        }

        // Check directory patterns
        if path.is_dir() {
            for (idx, pattern) in self.directory_patterns.iter().enumerate() {
                if pattern.matches_path(path) {
                    return Some(PatternMatch {
                        pattern: pattern.as_str().to_string(),
                        priority: idx as u32,
                        source: PatternSource::Config,
                    });
                }
            }
        }

        // Check file patterns
        if path.is_file() {
            for (idx, pattern) in self.file_patterns.iter().enumerate() {
                if pattern.matches_path(path) {
                    return Some(PatternMatch {
                        pattern: pattern.as_str().to_string(),
                        priority: idx as u32,
                        source: PatternSource::Config,
                    });
                }
            }
        }

        None
    }

    fn is_excluded(&self, path: &Path) -> bool {
        self.exclude_patterns.iter()
            .any(|p| p.matches_path(path))
    }
}
```

### Phase 3: File System Scanner (Week 2)

#### 2.3.1 Scanner Implementation

```rust
// src/engine/scanner.rs
use walkdir::{WalkDir, DirEntry};
use rayon::prelude::*;
use std::path::Path;
use std::sync::Arc;
use dashmap::DashMap;

pub struct Scanner {
    root: PathBuf,
    matcher: Arc<PatternMatcher>,
    max_depth: usize,
    follow_symlinks: bool,
    progress: Arc<dyn Progress>,
}

impl Scanner {
    pub fn scan(&self) -> Result<Vec<CleanItem>> {
        let items = Arc::new(DashMap::new());
        let items_clone = items.clone();

        WalkDir::new(&self.root)
            .max_depth(self.max_depth)
            .follow_links(self.follow_symlinks)
            .into_iter()
            .par_bridge()
            .filter_map(|e| e.ok())
            .for_each(|entry| {
                if let Some(item) = self.process_entry(&entry) {
                    items_clone.insert(item.path.clone(), item);
                    self.progress.increment(1);
                }
            });

        Ok(items.into_iter()
            .map(|(_, item)| item)
            .collect())
    }

    fn process_entry(&self, entry: &DirEntry) -> Option<CleanItem> {
        let path = entry.path();

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
                .map(|m| m.len())
                .sum()
        } else {
            metadata.len()
        }
    }
}
```

### Phase 4: Parallel Cleaner (Week 2-3)

#### 2.4.1 Cleaner Implementation

```rust
// src/engine/cleaner.rs
use rayon::prelude::*;
use crossbeam_channel::{bounded, Sender};
use std::fs;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

pub struct ParallelCleaner {
    thread_count: usize,
    chunk_size: usize,
    dry_run: bool,
    progress: Arc<dyn Progress>,
    stats: Arc<Statistics>,
}

#[derive(Default)]
pub struct Statistics {
    items_deleted: AtomicUsize,
    bytes_freed: AtomicU64,
    errors: DashMap<PathBuf, CleanError>,
}

impl ParallelCleaner {
    pub fn clean(&self, items: Vec<CleanItem>) -> Result<CleanReport> {
        if self.dry_run {
            return self.dry_run_clean(items);
        }

        let start = std::time::Instant::now();
        let (error_tx, error_rx) = bounded(100);

        // Process items in parallel
        items.par_chunks(self.chunk_size)
            .for_each_with(error_tx.clone(), |tx, chunk| {
                for item in chunk {
                    match self.delete_item(item) {
                        Ok(()) => {
                            self.stats.items_deleted.fetch_add(1, Ordering::Relaxed);
                            self.stats.bytes_freed.fetch_add(item.size, Ordering::Relaxed);
                            self.progress.increment(1);
                        }
                        Err(e) => {
                            let _ = tx.send((item.path.clone(), e));
                        }
                    }
                }
            });

        drop(error_tx);

        // Collect errors
        let mut errors = Vec::new();
        while let Ok((path, error)) = error_rx.recv() {
            errors.push(CleanError::IoError {
                path,
                source: error,
            });
        }

        Ok(CleanReport {
            items_deleted: self.stats.items_deleted.load(Ordering::Relaxed),
            bytes_freed: self.stats.bytes_freed.load(Ordering::Relaxed),
            errors,
            duration: start.elapsed(),
            dry_run: false,
        })
    }

    fn delete_item(&self, item: &CleanItem) -> io::Result<()> {
        match item.item_type {
            ItemType::Directory => {
                fs::remove_dir_all(&item.path)?;
            }
            ItemType::File => {
                fs::remove_file(&item.path)?;
            }
            ItemType::Symlink => {
                // Handle symlinks specially
                #[cfg(unix)]
                {
                    use std::os::unix::fs;
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

    fn dry_run_clean(&self, items: Vec<CleanItem>) -> Result<CleanReport> {
        let total_size: u64 = items.iter().map(|i| i.size).sum();

        // Display what would be deleted
        for item in &items {
            println!("Would delete: {} ({})",
                item.path.display(),
                humansize::format_size(item.size, humansize::DECIMAL));
        }

        Ok(CleanReport {
            items_deleted: items.len(),
            bytes_freed: total_size,
            errors: Vec::new(),
            duration: std::time::Duration::ZERO,
            dry_run: true,
        })
    }
}
```

### Phase 5: CLI Implementation (Week 3)

#### 2.5.1 CLI Structure

```rust
// src/cli/mod.rs
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mc")]
#[command(about = "Mr. Cleann - A high-performance build directory cleaner")]
#[command(version)]
pub struct Cli {
    /// Path to clean
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Preview what would be deleted without actually deleting
    #[arg(short = 'd', long = "dry-run")]
    pub dry_run: bool,

    /// Verbose output
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// Suppress non-essential output
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,

    /// Skip confirmation prompts
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,

    /// Exclude patterns (can be repeated)
    #[arg(short = 'e', long = "exclude")]
    pub exclude: Vec<String>,

    /// Additional patterns to clean
    #[arg(short = 'i', long = "include")]
    pub include: Vec<String>,

    /// Use custom configuration file
    #[arg(short = 'c', long = "config")]
    pub config: Option<PathBuf>,

    /// Show statistics after cleaning
    #[arg(short = 's', long = "stats")]
    pub stats: bool,

    /// Number of parallel threads
    #[arg(short = 'p', long = "parallel")]
    pub parallel: Option<usize>,

    /// Don't check for .git directories
    #[arg(long = "no-git-check")]
    pub no_git_check: bool,

    /// Preserve .env files
    #[arg(long = "preserve-env")]
    pub preserve_env: bool,

    /// Include dangerous operations (git, env files)
    #[arg(long = "nuclear")]
    pub nuclear: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List what would be cleaned without deleting
    List {
        /// Format output as JSON
        #[arg(long = "json")]
        json: bool,
    },

    /// Initialize configuration file
    Init {
        /// Global configuration
        #[arg(long = "global")]
        global: bool,
    },

    /// Show current configuration
    Config,
}
```

### Phase 6: Configuration System (Week 3)

#### 2.6.1 Configuration Management

```rust
// src/config/mod.rs
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use directories::ProjectDirs;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub patterns: PatternConfig,
    pub options: OptionsConfig,
    pub safety: SafetyConfig,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PatternConfig {
    pub directories: Vec<String>,
    pub files: Vec<String>,
    pub exclude: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OptionsConfig {
    #[serde(default = "default_parallel_threads")]
    pub parallel_threads: usize,

    #[serde(default = "default_true")]
    pub require_confirmation: bool,

    #[serde(default = "default_true")]
    pub show_statistics: bool,

    #[serde(default = "default_true")]
    pub preserve_symlinks: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SafetyConfig {
    #[serde(default = "default_true")]
    pub check_git_repo: bool,

    #[serde(default = "default_max_depth")]
    pub max_depth: usize,

    #[serde(default = "default_min_free_space")]
    pub min_free_space_gb: f64,
}

impl Config {
    pub fn load(path: Option<&PathBuf>) -> Result<Self> {
        let config_path = path
            .cloned()
            .or_else(Self::find_config_file)
            .unwrap_or_else(Self::default_config_path);

        if config_path.exists() {
            let contents = fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&contents)?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    fn find_config_file() -> Option<PathBuf> {
        // Look for .mc.toml in current directory and parents
        let current = std::env::current_dir().ok()?;

        for ancestor in current.ancestors() {
            let config = ancestor.join(".mc.toml");
            if config.exists() {
                return Some(config);
            }
        }

        None
    }

    fn default_config_path() -> PathBuf {
        ProjectDirs::from("com", "mc", "mc")
            .map(|dirs| dirs.config_dir().join("config.toml"))
            .unwrap_or_else(|| PathBuf::from(".mc.toml"))
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            patterns: PatternConfig {
                directories: BUILTIN_PATTERNS.directories
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                files: BUILTIN_PATTERNS.files
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                exclude: BUILTIN_PATTERNS.exclude
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            },
            options: OptionsConfig::default(),
            safety: SafetyConfig::default(),
        }
    }
}
```

### Phase 7: Safety Mechanisms (Week 4)

#### 2.7.1 Safety Guards

```rust
// src/safety/guards.rs
use std::path::Path;
use anyhow::{Result, bail};

pub struct SafetyGuard {
    check_git: bool,
    max_depth: usize,
    min_free_space: u64,
}

impl SafetyGuard {
    pub fn validate(&self, path: &Path) -> Result<()> {
        // Check if path exists
        if !path.exists() {
            bail!("Path does not exist: {}", path.display());
        }

        // Check if it's a git repository
        if self.check_git && self.is_git_repo(path) {
            bail!("Path is inside a git repository. Use --no-git-check to override.");
        }

        // Check available disk space
        if let Ok(space) = self.get_free_space(path) {
            if space < self.min_free_space {
                bail!("Insufficient disk space. Need at least {} GB free",
                    self.min_free_space / 1_000_000_000);
            }
        }

        Ok(())
    }

    fn is_git_repo(&self, path: &Path) -> bool {
        path.ancestors()
            .any(|p| p.join(".git").exists())
    }

    #[cfg(unix)]
    fn get_free_space(&self, path: &Path) -> Result<u64> {
        use nix::sys::statvfs::statvfs;
        let stat = statvfs(path)?;
        Ok(stat.blocks_available() * stat.block_size())
    }

    #[cfg(windows)]
    fn get_free_space(&self, path: &Path) -> Result<u64> {
        // Windows implementation
        use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
        // Implementation details...
        Ok(0) // Placeholder
    }
}
```

### Phase 8: Progress and Reporting (Week 4)

#### 2.8.1 Progress Reporting

```rust
// src/utils/progress.rs
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::Arc;

pub trait Progress: Send + Sync {
    fn increment(&self, delta: u64);
    fn set_message(&self, msg: &str);
    fn finish(&self);
}

pub struct ProgressReporter {
    bar: ProgressBar,
}

impl ProgressReporter {
    pub fn new(total: u64) -> Self {
        let bar = ProgressBar::new(total);
        bar.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")
                .unwrap()
                .progress_chars("#>-"),
        );

        Self { bar }
    }
}

impl Progress for ProgressReporter {
    fn increment(&self, delta: u64) {
        self.bar.inc(delta);
    }

    fn set_message(&self, msg: &str) {
        self.bar.set_message(msg.to_string());
    }

    fn finish(&self) {
        self.bar.finish_with_message("Complete");
    }
}
```

## 3. Testing Strategy

### 3.1 Unit Tests

```rust
// tests/unit/patterns_test.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_matching() {
        let matcher = PatternMatcher::new(&PatternConfig {
            directories: vec!["node_modules".to_string()],
            files: vec!["*.log".to_string()],
            exclude: vec![".git".to_string()],
        }).unwrap();

        assert!(matcher.matches(Path::new("node_modules")).is_some());
        assert!(matcher.matches(Path::new("test.log")).is_some());
        assert!(matcher.matches(Path::new(".git")).is_none());
    }
}
```

### 3.2 Integration Tests

```rust
// tests/integration/clean_test.rs
use assert_fs::prelude::*;
use predicates::prelude::*;

#[test]
fn test_clean_node_project() {
    let temp = assert_fs::TempDir::new().unwrap();

    // Create test structure
    temp.child("node_modules").create_dir_all().unwrap();
    temp.child("dist").create_dir_all().unwrap();
    temp.child("src/index.js").touch().unwrap();

    // Run cleaner
    let result = run_mc(&[
        "--dry-run",
        temp.path().to_str().unwrap(),
    ]);

    assert!(result.is_ok());

    // Verify directories still exist (dry-run)
    temp.child("node_modules").assert(predicate::path::exists());
    temp.child("dist").assert(predicate::path::exists());
}
```

## 4. Performance Benchmarks

```rust
// benches/performance.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_scan(c: &mut Criterion) {
    let temp = setup_large_project();

    c.bench_function("scan 10k files", |b| {
        b.iter(|| {
            Scanner::new(black_box(&temp.path()))
                .scan()
                .unwrap()
        });
    });
}

fn benchmark_parallel_delete(c: &mut Criterion) {
    c.bench_function("delete 1k files parallel", |b| {
        let files = generate_temp_files(1000);
        b.iter(|| {
            ParallelCleaner::new()
                .clean(black_box(files.clone()))
                .unwrap()
        });
    });
}

criterion_group!(benches, benchmark_scan, benchmark_parallel_delete);
criterion_main!(benches);
```

## 5. Error Handling Strategy

```rust
// src/error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum McError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(#[from] toml::de::Error),

    #[error("Pattern error: {0}")]
    Pattern(#[from] glob::PatternError),

    #[error("Permission denied: {path}")]
    PermissionDenied { path: PathBuf },

    #[error("Safety check failed: {0}")]
    Safety(String),

    #[error("User cancelled operation")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, McError>;
```

## 6. Build and Release Process

### 6.1 Build Configuration

```toml
# Cargo.toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true
panic = "abort"

[profile.bench]
inherits = "release"
```

### 6.2 Cross-Compilation

```bash
# Build for multiple targets
cargo build --release --target x86_64-unknown-linux-gnu
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-pc-windows-msvc
```

### 6.3 CI/CD Pipeline

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  build:
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
          - os: macos-latest
            target: x86_64-apple-darwin
          - os: macos-latest
            target: aarch64-apple-darwin
          - os: windows-latest
            target: x86_64-pc-windows-msvc

    runs-on: ${{ matrix.os }}

    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release --target ${{ matrix.target }}
      - run: cargo test --release
      - run: cargo bench --no-run
```

## 7. Documentation

### 7.1 API Documentation

```rust
//! # Mr. Cleann (mc)
//!
//! A high-performance build directory cleaner for modern development workflows.
//!
//! ## Example
//!
//! ```rust
//! use mc::{Cleaner, Config};
//!
//! let config = Config::default();
//! let cleaner = Cleaner::new(config);
//!
//! let report = cleaner.clean("./project")?;
//! println!("Freed {} bytes", report.bytes_freed);
//! ```

/// Main cleaner interface
pub struct Cleaner {
    config: Config,
    engine: Engine,
}

impl Cleaner {
    /// Creates a new cleaner with the given configuration
    pub fn new(config: Config) -> Self {
        Self {
            config,
            engine: Engine::new(),
        }
    }

    /// Performs the cleaning operation
    ///
    /// # Arguments
    ///
    /// * `path` - The root path to clean
    ///
    /// # Returns
    ///
    /// A `CleanReport` containing statistics about the operation
    pub fn clean<P: AsRef<Path>>(&self, path: P) -> Result<CleanReport> {
        // Implementation
    }
}
```

## 8. Optimization Techniques

### 8.1 Memory Optimization

```rust
// Use SmallVec for small collections
use smallvec::SmallVec;

// Use Arc for shared immutable data
use std::sync::Arc;

// Pool allocations for repeated operations
use crossbeam::sync::WaitGroup;
```

### 8.2 I/O Optimization

```rust
// Batch file operations
const BATCH_SIZE: usize = 100;

// Use memory-mapped files for large reads
use memmap2::Mmap;

// Prefetch directory entries
use std::fs::read_dir;
```

### 8.3 Parallelization Optimization

```rust
// Adaptive chunk sizing based on workload
fn calculate_chunk_size(items: usize) -> usize {
    match items {
        0..=100 => 1,
        101..=1000 => 10,
        1001..=10000 => 50,
        _ => 100,
    }
}

// Work-stealing with Rayon
use rayon::iter::ParallelIterator;
```