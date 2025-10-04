# API Documentation
# Mr. Cleann (mc) - Public API Reference

## Table of Contents

1. [Core Types](#core-types)
2. [Main API](#main-api)
3. [Configuration API](#configuration-api)
4. [Pattern API](#pattern-api)
5. [Scanner API](#scanner-api)
6. [Cleaner API](#cleaner-api)
7. [Progress API](#progress-api)
8. [Error Types](#error-types)
9. [Utilities](#utilities)

## Core Types

### `CleanItem`

Represents a file or directory to be cleaned.

```rust
pub struct CleanItem {
    pub path: PathBuf,
    pub size: u64,
    pub item_type: ItemType,
    pub pattern: PatternMatch,
}
```

**Fields:**
- `path`: Full path to the item
- `size`: Size in bytes (calculated recursively for directories)
- `item_type`: Type of the item (Directory, File, or Symlink)
- `pattern`: The pattern that matched this item

### `ItemType`

Enum representing the type of file system item.

```rust
pub enum ItemType {
    Directory,
    File,
    Symlink,
}
```

### `PatternMatch`

Information about which pattern matched an item.

```rust
pub struct PatternMatch {
    pub pattern: String,
    pub priority: u32,
    pub source: PatternSource,
}
```

**Fields:**
- `pattern`: The pattern string that matched
- `priority`: Priority of the pattern (lower = higher priority)
- `source`: Where the pattern came from

### `PatternSource`

Origin of a pattern.

```rust
pub enum PatternSource {
    BuiltIn,
    Config,
    CLI,
}
```

### `CleanReport`

Report of a cleaning operation.

```rust
pub struct CleanReport {
    pub items_deleted: usize,
    pub bytes_freed: u64,
    pub errors: Vec<CleanError>,
    pub duration: Duration,
    pub dry_run: bool,
}
```

**Fields:**
- `items_deleted`: Number of items deleted
- `bytes_freed`: Total bytes freed
- `errors`: List of errors encountered
- `duration`: Time taken for the operation
- `dry_run`: Whether this was a dry run

## Main API

### `Cleaner`

The main interface for cleaning operations.

```rust
pub struct Cleaner {
    config: Config,
    engine: Engine,
}
```

#### Methods

##### `new`

Creates a new `Cleaner` instance.

```rust
pub fn new(config: Config) -> Self
```

**Parameters:**
- `config`: Configuration for the cleaner

**Returns:**
- A new `Cleaner` instance

**Example:**
```rust
let config = Config::default();
let cleaner = Cleaner::new(config);
```

##### `clean`

Performs the cleaning operation.

```rust
pub fn clean<P: AsRef<Path>>(&self, path: P) -> Result<CleanReport>
```

**Parameters:**
- `path`: The root path to clean

**Returns:**
- `Result<CleanReport>`: Report of the operation or error

**Example:**
```rust
let report = cleaner.clean("/path/to/project")?;
println!("Freed {} bytes", report.bytes_freed);
```

##### `dry_run`

Performs a dry run without deleting anything.

```rust
pub fn dry_run<P: AsRef<Path>>(&self, path: P) -> Result<CleanReport>
```

**Parameters:**
- `path`: The root path to analyze

**Returns:**
- `Result<CleanReport>`: Report with `dry_run: true`

##### `with_progress`

Attaches a progress reporter to the cleaner.

```rust
pub fn with_progress(self, progress: Arc<dyn Progress>) -> Self
```

**Parameters:**
- `progress`: Progress reporter implementation

**Returns:**
- Modified `Cleaner` instance

## Configuration API

### `Config`

Main configuration structure.

```rust
pub struct Config {
    pub patterns: PatternConfig,
    pub options: OptionsConfig,
    pub safety: SafetyConfig,
}
```

#### Methods

##### `load`

Loads configuration from file.

```rust
pub fn load(path: Option<&PathBuf>) -> Result<Self>
```

**Parameters:**
- `path`: Optional path to config file

**Returns:**
- `Result<Self>`: Loaded configuration or error

##### `default`

Returns default configuration.

```rust
pub fn default() -> Self
```

##### `merge_cli_args`

Merges CLI arguments into configuration.

```rust
pub fn merge_cli_args(&mut self, args: &CliArgs)
```

**Parameters:**
- `args`: CLI arguments to merge

### `PatternConfig`

Pattern configuration.

```rust
pub struct PatternConfig {
    pub directories: Vec<String>,
    pub files: Vec<String>,
    pub exclude: Vec<String>,
}
```

**Fields:**
- `directories`: Directory patterns to clean
- `files`: File patterns to clean
- `exclude`: Patterns to exclude from cleaning

### `OptionsConfig`

Options configuration.

```rust
pub struct OptionsConfig {
    pub parallel_threads: usize,
    pub require_confirmation: bool,
    pub show_statistics: bool,
    pub preserve_symlinks: bool,
}
```

### `SafetyConfig`

Safety configuration.

```rust
pub struct SafetyConfig {
    pub check_git_repo: bool,
    pub max_depth: usize,
    pub min_free_space_gb: f64,
}
```

## Pattern API

### `PatternMatcher`

Pattern matching engine.

```rust
pub struct PatternMatcher {
    directory_patterns: Vec<Pattern>,
    file_patterns: Vec<Pattern>,
    exclude_patterns: Vec<Pattern>,
}
```

#### Methods

##### `new`

Creates a new pattern matcher.

```rust
pub fn new(config: &PatternConfig) -> Result<Self, PatternError>
```

##### `matches`

Checks if a path matches any pattern.

```rust
pub fn matches(&self, path: &Path) -> Option<PatternMatch>
```

**Parameters:**
- `path`: Path to check

**Returns:**
- `Option<PatternMatch>`: Match information or None

##### `is_excluded`

Checks if a path is excluded.

```rust
pub fn is_excluded(&self, path: &Path) -> bool
```

## Scanner API

### `Scanner`

File system scanner.

```rust
pub struct Scanner {
    root: PathBuf,
    matcher: Arc<PatternMatcher>,
    max_depth: usize,
    follow_symlinks: bool,
    progress: Arc<dyn Progress>,
}
```

#### Methods

##### `new`

Creates a new scanner.

```rust
pub fn new(root: PathBuf, matcher: Arc<PatternMatcher>) -> Self
```

##### `scan`

Scans the file system for matching items.

```rust
pub fn scan(&self) -> Result<Vec<CleanItem>>
```

**Returns:**
- `Result<Vec<CleanItem>>`: List of items to clean

##### `with_max_depth`

Sets maximum traversal depth.

```rust
pub fn with_max_depth(self, depth: usize) -> Self
```

##### `with_symlinks`

Configures symlink following.

```rust
pub fn with_symlinks(self, follow: bool) -> Self
```

## Cleaner API

### `ParallelCleaner`

Parallel cleaning implementation.

```rust
pub struct ParallelCleaner {
    thread_count: usize,
    chunk_size: usize,
    dry_run: bool,
    progress: Arc<dyn Progress>,
    stats: Arc<Statistics>,
}
```

#### Methods

##### `new`

Creates a new parallel cleaner.

```rust
pub fn new() -> Self
```

##### `clean`

Performs parallel cleaning.

```rust
pub fn clean(&self, items: Vec<CleanItem>) -> Result<CleanReport>
```

##### `with_threads`

Sets thread count.

```rust
pub fn with_threads(self, count: usize) -> Self
```

### `Statistics`

Cleaning statistics.

```rust
pub struct Statistics {
    pub items_deleted: AtomicUsize,
    pub bytes_freed: AtomicU64,
    pub errors: DashMap<PathBuf, CleanError>,
}
```

## Progress API

### `Progress` Trait

Progress reporting interface.

```rust
pub trait Progress: Send + Sync {
    fn increment(&self, delta: u64);
    fn set_message(&self, msg: &str);
    fn finish(&self);
}
```

### `ProgressReporter`

Default progress reporter implementation.

```rust
pub struct ProgressReporter {
    bar: ProgressBar,
}
```

#### Methods

##### `new`

Creates a new progress reporter.

```rust
pub fn new(total: u64) -> Self
```

**Parameters:**
- `total`: Total number of items

### `NoOpProgress`

No-operation progress reporter for quiet mode.

```rust
pub struct NoOpProgress;

impl Progress for NoOpProgress {
    fn increment(&self, _: u64) {}
    fn set_message(&self, _: &str) {}
    fn finish(&self) {}
}
```

## Error Types

### `McError`

Main error type for the application.

```rust
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
```

### `CleanError`

Specific error for cleaning operations.

```rust
#[derive(Debug, Error)]
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

### `Result<T>` Type Alias

Convenience type alias.

```rust
pub type Result<T> = std::result::Result<T, McError>;
```

## Utilities

### File System Utilities

#### `calculate_directory_size`

Calculates total size of a directory.

```rust
pub fn calculate_directory_size(path: &Path) -> Result<u64>
```

**Parameters:**
- `path`: Directory path

**Returns:**
- `Result<u64>`: Total size in bytes

#### `format_size`

Formats byte size for display.

```rust
pub fn format_size(bytes: u64) -> String
```

**Parameters:**
- `bytes`: Size in bytes

**Returns:**
- `String`: Human-readable size (e.g., "1.5 GB")

**Example:**
```rust
let size = format_size(1_500_000_000);
assert_eq!(size, "1.50 GB");
```

### Path Utilities

#### `canonicalize_safe`

Safely canonicalizes a path.

```rust
pub fn canonicalize_safe(path: &Path) -> Result<PathBuf>
```

#### `is_hidden`

Checks if a file/directory is hidden.

```rust
pub fn is_hidden(path: &Path) -> bool
```

### Platform Utilities

#### `get_cpu_count`

Gets the number of CPU cores.

```rust
pub fn get_cpu_count() -> usize
```

#### `get_free_space`

Gets free disk space for a path.

```rust
pub fn get_free_space(path: &Path) -> Result<u64>
```

**Parameters:**
- `path`: Path to check

**Returns:**
- `Result<u64>`: Free space in bytes

## Usage Examples

### Basic Usage

```rust
use mc::{Cleaner, Config};

fn main() -> mc::Result<()> {
    // Load default configuration
    let config = Config::default();

    // Create cleaner
    let cleaner = Cleaner::new(config);

    // Clean current directory
    let report = cleaner.clean(".")?;

    // Display results
    println!("Cleaned {} items", report.items_deleted);
    println!("Freed {}", format_size(report.bytes_freed));

    Ok(())
}
```

### Custom Configuration

```rust
use mc::{Cleaner, Config, PatternConfig};

fn main() -> mc::Result<()> {
    let mut config = Config::default();

    // Add custom patterns
    config.patterns.directories.push("custom_build".to_string());
    config.patterns.files.push("*.tmp".to_string());

    // Exclude specific directories
    config.patterns.exclude.push("important_cache".to_string());

    // Configure options
    config.options.parallel_threads = 8;
    config.options.require_confirmation = false;

    let cleaner = Cleaner::new(config);
    let report = cleaner.clean("/path/to/project")?;

    Ok(())
}
```

### With Progress Reporting

```rust
use mc::{Cleaner, Config, ProgressReporter};
use std::sync::Arc;

fn main() -> mc::Result<()> {
    let config = Config::default();
    let progress = Arc::new(ProgressReporter::new(100));

    let cleaner = Cleaner::new(config)
        .with_progress(progress.clone());

    let report = cleaner.clean(".")?;
    progress.finish();

    Ok(())
}
```

### Dry Run

```rust
use mc::{Cleaner, Config};

fn main() -> mc::Result<()> {
    let config = Config::default();
    let cleaner = Cleaner::new(config);

    // Preview what would be deleted
    let report = cleaner.dry_run(".")?;

    println!("Would delete {} items", report.items_deleted);
    println!("Would free {}", format_size(report.bytes_freed));

    // Confirm and actually clean
    if confirm("Proceed with cleaning?") {
        let report = cleaner.clean(".")?;
        println!("Cleaning complete!");
    }

    Ok(())
}
```

### Error Handling

```rust
use mc::{Cleaner, Config, McError};

fn main() {
    let config = Config::default();
    let cleaner = Cleaner::new(config);

    match cleaner.clean(".") {
        Ok(report) => {
            println!("Success: {} items cleaned", report.items_deleted);
        }
        Err(McError::PermissionDenied { path }) => {
            eprintln!("Permission denied for: {}", path.display());
        }
        Err(McError::Safety(msg)) => {
            eprintln!("Safety check failed: {}", msg);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }
}
```

## Thread Safety

All public types in the API are designed to be thread-safe:

- `Cleaner`: Can be shared across threads using `Arc`
- `Config`: Immutable after creation
- `Progress`: Trait requires `Send + Sync`
- `Statistics`: Uses atomic types and concurrent collections

## Performance Considerations

- Default thread count is set to CPU count
- Chunk size is automatically optimized based on workload
- Directory size calculation is cached
- Pattern matching is optimized with pre-compiled patterns

## Version Compatibility

This API is designed following semantic versioning:

- **Major version**: Breaking API changes
- **Minor version**: New features, backwards compatible
- **Patch version**: Bug fixes, no API changes

Current version: 1.0.0