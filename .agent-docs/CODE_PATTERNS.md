# Identified Code Patterns

## 1. Builder Pattern

**Location:** Multiple structs use method chaining for configuration

```rust
// Scanner (src/engine/scanner.rs)
Scanner::new(path, matcher)
    .with_max_depth(10)
    .with_symlinks(false)
    .with_category_tracker(tracker)

// ParallelCleaner (src/engine/cleaner.rs)
ParallelCleaner::new()
    .with_threads(8)
    .with_dry_run(true)
    .with_progress(progress)

// Cleaner (src/lib.rs)
Cleaner::new(config)
    .with_dry_run(true)
    .with_quiet(false)
    .with_verbose(true)
```

**Usage:** All public structs that need runtime configuration

---

## 2. Parallel Processing Pattern (Rayon)

**Location:** `src/engine/scanner.rs`, `src/engine/cleaner.rs`

```rust
// Streaming parallel scan
WalkDir::new(&self.root)
    .into_iter()
    .par_bridge()           // Convert to parallel iterator
    .fold(                   // Thread-local accumulator
        || ScanAccumulator::default(),
        |mut acc, entry| { /* process */ acc }
    )
    .reduce(                 // Merge accumulators
        || ScanAccumulator::default(),
        |mut acc, other| { acc.items.append(&mut other.items); acc }
    )

// Parallel cleaning with chunk control
items.par_iter()
    .with_min_len(self.chunk_size)  // Control granularity
    .for_each(|item| { /* delete */ })
```

**Pattern:** `fold` + `reduce` for thread-local accumulation then merge

---

## 3. Trait Object Pattern (Progress Reporting)

**Location:** `src/utils/progress.rs`

```rust
pub trait Progress: Send + Sync {
    fn increment(&self, delta: u64);
    fn set_message(&self, msg: &str);
    fn finish(&self);
}

// Implementations
pub struct ProgressReporter { bar: ProgressBar }  // Visual progress
pub struct NoOpProgress;                          // Quiet mode
pub struct CompactDisplay { ... }                 // Compact 3-line display

// Usage with Arc<dyn Progress>
let progress: Arc<dyn Progress> = if quiet {
    Arc::new(NoOpProgress)
} else {
    Arc::new(ProgressReporter::new(total))
};
```

**Pattern:** Strategy pattern via trait objects for swappable implementations

---

## 4. Error Handling Pattern

**Location:** `src/types.rs`

```rust
// Application-level errors with thiserror
#[derive(Debug, thiserror::Error)]
pub enum McError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Configuration parse error: {0}")]
    ConfigParse(#[from] toml::de::Error),
    
    #[error("Pattern error: {0}")]
    Pattern(#[from] glob::PatternError),
    
    #[error("Safety check failed: {0}")]
    Safety(String),
}

// Type alias for convenience
pub type Result<T> = std::result::Result<T, McError>;

// Per-item errors (non-fatal)
#[derive(Debug, Clone, thiserror::Error, Serialize)]
pub enum CleanError {
    #[error("Permission denied: {path}")]
    PermissionDenied { path: PathBuf },
    
    #[error("IO error at {path}: {message}")]
    IoError { path: PathBuf, message: String },
}
```

**Pattern:** Separate error types for fatal (McError) vs recoverable (CleanError) errors

---

## 5. Atomic Statistics Pattern

**Location:** `src/engine/cleaner.rs`, `src/utils/progress.rs`

```rust
// Thread-safe statistics
pub struct Statistics {
    pub items_deleted: AtomicUsize,
    pub bytes_freed: AtomicU64,
    pub errors: DashMap<PathBuf, CleanError>,
}

// Update from parallel threads
stats.items_deleted.fetch_add(1, Ordering::Relaxed);
stats.bytes_freed.fetch_add(item.size, Ordering::Relaxed);

// CategoryTracker uses same pattern
pub struct CategoryTracker {
    counts: DashMap<PatternCategory, AtomicUsize>,
    sizes: DashMap<PatternCategory, AtomicU64>,
}
```

**Pattern:** `Atomic*` types + `DashMap` for lock-free concurrent updates

---

## 6. Configuration Hierarchy Pattern

**Location:** `src/config/mod.rs`

```rust
impl Config {
    pub fn load(path: Option<&PathBuf>) -> Result<Self> {
        let config_path = path
            .cloned()
            .or_else(Self::find_config_file)     // Search ancestors
            .unwrap_or_else(Self::default_config_path);  // Global fallback
        
        if config_path.exists() {
            let contents = fs::read_to_string(&config_path)?;
            Ok(toml::from_str(&contents)?)
        } else {
            Ok(Self::default())
        }
    }
    
    pub fn merge_cli_args(&mut self, exclude: Vec<String>, include: Vec<String>, preserve_env: bool) {
        // CLI args override config file
        for pattern in exclude {
            if !self.patterns.exclude.contains(&pattern) {
                self.patterns.exclude.push(pattern);
            }
        }
    }
}
```

**Pattern:** Cascading config sources with merge capability

---

## 7. Lazy Static Initialization

**Location:** `src/patterns/builtin.rs`

```rust
use once_cell::sync::Lazy;

pub static BUILTIN_PATTERNS: Lazy<PatternSet> = Lazy::new(|| {
    PatternSet {
        categorized_dirs: vec![
            ("dist", PatternCategory::BuildOutputs),
            ("node_modules", PatternCategory::Dependencies),
            // ...
        ],
        // ...
    }
});
```

**Pattern:** `once_cell::Lazy` for expensive one-time initialization

---

## 8. Platform-Specific Code Pattern

**Location:** `src/engine/cleaner.rs`, `src/safety/guards.rs`

```rust
fn delete_item(&self, item: &CleanItem) -> io::Result<()> {
    match item.item_type {
        ItemType::Symlink => {
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
        // ...
    }
}
```

**Pattern:** `#[cfg(target_os)]` for platform-specific implementations

---

## 9. Accumulator Pattern for Parallel Reduction

**Location:** `src/engine/scanner.rs`

```rust
#[derive(Default)]
struct ScanAccumulator {
    items: Vec<CleanItem>,
    errors: Vec<ScanError>,
    file_sizes: Vec<(PathBuf, u64)>,
    dir_bases: Vec<(PathBuf, u64)>,
}

// Used with rayon fold/reduce
.fold(
    || ScanAccumulator::default(),  // Thread-local init
    |mut acc, entry| { /* add to acc */ acc }
)
.reduce(
    || ScanAccumulator::default(),
    |mut acc, mut other| {
        acc.items.append(&mut other.items);
        acc.errors.append(&mut other.errors);
        // ...
        acc
    }
)
```

**Pattern:** Custom accumulator struct for complex parallel reductions

---

## 10. Clap Derive Pattern

**Location:** `src/cli/mod.rs`

```rust
#[derive(Parser)]
#[command(name = "mc")]
#[command(about = "Mr. Cleann - A high-performance build directory cleaner")]
pub struct Cli {
    #[arg(default_value = ".")]
    pub path: PathBuf,
    
    #[arg(short = 'd', long = "dry-run")]
    pub dry_run: bool,
    
    #[arg(short = 'e', long = "exclude")]
    pub exclude: Vec<String>,  // Repeatable
    
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Clone)]
pub enum Commands {
    List { #[arg(long = "json")] json: bool },
    Init { #[arg(long = "global")] global: bool },
    Config,
}
```

**Pattern:** Clap derive macros for declarative CLI definition

---

## API Response Pattern

**Format:** `CleanReport` struct returned from all cleaning operations

```rust
CleanReport {
    items_deleted: usize,    // Count of deleted items
    bytes_freed: u64,        // Total bytes freed
    errors: Vec<CleanError>, // Per-item errors (non-fatal)
    scan_errors: Vec<ScanError>, // Errors during scan phase
    duration: Duration,      // Operation time
    dry_run: bool,           // Whether this was a dry run
}
```

---

## Common Coding Conventions

1. **Naming**: `snake_case` for modules/functions, `PascalCase` for types, `SCREAMING_SNAKE_CASE` for constants
2. **Error Handling**: `anyhow` for application errors, `thiserror` for library errors
3. **Parallelism**: `Arc` for shared state, atomics for counters, `DashMap` for concurrent collections
4. **Progress**: Abstract via `Progress` trait, swap implementations based on verbosity
5. **Configuration**: Serde derive for TOML serialization, default functions for computed defaults

