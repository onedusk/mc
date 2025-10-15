# System Architecture Document
# Mr. Cleann (mc) - Technical Architecture

## 1. System Overview

Mr. Cleann is architected as a modular, high-performance command-line application following Rust best practices and leveraging parallelism for optimal performance.

```
┌─────────────────────────────────────────────────────────┐
│                      CLI Layer                          │
│                   (clap, user interaction)              │
└─────────────────┬───────────────────────────────────────┘
                  │
┌─────────────────▼───────────────────────────────────────┐
│                Configuration Layer                      │
│            (config parsing, validation)                 │
└─────────────────┬───────────────────────────────────────┘
                  │
┌─────────────────▼───────────────────────────────────────┐
│                  Core Engine                            │
│         (orchestration, pattern matching)               │
└────────┬─────────────────────────────┬──────────────────┘
         │                             │
┌────────▼──────────┐         ┌───────▼──────────┐
│  Scanner Module   │         │  Cleaner Module  │
│  (walkdir, glob)  │         │ (fs operations)  │
└───────────────────┘         └──────────────────┘
         │                             │
┌────────▼─────────────────────────────▼──────────┐
│            Parallel Executor (Rayon)             │
│         (work stealing, thread pool)             │
└──────────────────────────────────────────────────┘
         │
┌────────▼─────────────────────────────────────────┐
│                File System Layer                 │
│            (OS-specific operations)              │
└──────────────────────────────────────────────────┘
```

## 2. Component Architecture

### 2.1 Module Structure

```
mc/
├── src/
│   ├── main.rs              # Entry point, CLI setup
│   ├── lib.rs               # Library root
│   ├── cli/
│   │   ├── mod.rs           # CLI module
│   │   ├── args.rs          # Argument parsing
│   │   └── commands.rs      # Command handlers
│   ├── config/
│   │   ├── mod.rs           # Configuration module
│   │   ├── parser.rs        # TOML parsing
│   │   ├── defaults.rs      # Default configurations
│   │   └── validator.rs     # Config validation
│   ├── engine/
│   │   ├── mod.rs           # Core engine
│   │   ├── scanner.rs       # File system scanning
│   │   ├── matcher.rs       # Pattern matching
│   │   ├── cleaner.rs       # Deletion operations
│   │   └── reporter.rs      # Statistics and reporting
│   ├── patterns/
│   │   ├── mod.rs           # Pattern definitions
│   │   ├── builtin.rs       # Built-in patterns
│   │   └── custom.rs        # User-defined patterns
│   ├── parallel/
│   │   ├── mod.rs           # Parallelization logic
│   │   ├── executor.rs      # Rayon executor
│   │   └── scheduler.rs     # Work scheduling
│   ├── safety/
│   │   ├── mod.rs           # Safety mechanisms
│   │   ├── guards.rs        # Safety guards
│   │   └── recovery.rs      # Recovery mechanisms
│   └── utils/
│       ├── mod.rs           # Utilities
│       ├── fs.rs            # File system helpers
│       ├── progress.rs      # Progress reporting
│       └── logger.rs        # Logging utilities
├── tests/
│   ├── integration/         # Integration tests
│   └── unit/               # Unit tests
└── benches/
    └── performance.rs       # Performance benchmarks
```

## 3. Data Flow Architecture

### 3.1 Execution Pipeline

```
User Input
    │
    ▼
Parse Arguments ──► Load Config ──► Merge Settings
    │
    ▼
Validate Paths ──► Check Safety ──► Build Patterns
    │
    ▼
┌───────────────────────────┐
│   Scanning Phase          │
│   - Walk directories      │
│   - Match patterns        │
│   - Build file list       │
└────────────┬──────────────┘
             │
             ▼
┌───────────────────────────┐
│   Analysis Phase          │
│   - Calculate sizes       │
│   - Check permissions     │
│   - Validate operations   │
└────────────┬──────────────┘
             │
             ▼
┌───────────────────────────┐
│   Execution Phase         │
│   - Parallel deletion     │
│   - Progress tracking     │
│   - Error handling        │
└────────────┬──────────────┘
             │
             ▼
┌───────────────────────────┐
│   Reporting Phase         │
│   - Statistics            │
│   - Cleanup summary       │
│   - Error report          │
└───────────────────────────┘
```

### 3.2 Parallel Processing Model

```rust
// Work distribution using Rayon
pub struct ParallelCleaner {
    thread_pool: ThreadPool,
    chunk_size: usize,
    progress: Arc<Mutex<Progress>>,
}

// Work item for parallel processing
pub struct CleanTask {
    path: PathBuf,
    pattern: Pattern,
    options: CleanOptions,
}

// Execution strategy
impl ParallelCleaner {
    pub fn execute(&self, tasks: Vec<CleanTask>) -> Result<CleanReport> {
        tasks.par_chunks(self.chunk_size)
            .map(|chunk| self.process_chunk(chunk))
            .try_reduce(CleanReport::new, CleanReport::merge)
    }
}
```

## 4. Core Components

### 4.1 CLI Component

**Responsibilities:**
- Parse command-line arguments
- Validate user input
- Display help and version info
- Handle user interaction

**Key Types:**
```rust
pub struct CliArgs {
    pub path: PathBuf,
    pub dry_run: bool,
    pub verbose: bool,
    pub quiet: bool,
    pub yes: bool,
    pub exclude: Vec<String>,
    pub include: Vec<String>,
    pub config: Option<PathBuf>,
    pub stats: bool,
    pub parallel: Option<usize>,
    pub nuclear: bool,
    pub preserve_env: bool,
}
```

### 4.2 Configuration Component

**Responsibilities:**
- Parse configuration files
- Merge CLI args with config
- Validate settings
- Provide defaults

**Configuration Schema:**
```rust
#[derive(Deserialize, Serialize)]
pub struct Config {
    pub patterns: PatternConfig,
    pub options: OptionsConfig,
    pub safety: SafetyConfig,
}

pub struct PatternConfig {
    pub directories: Vec<String>,
    pub files: Vec<String>,
    pub exclude: Vec<String>,
}
```

### 4.3 Scanner Component

**Responsibilities:**
- Traverse file system
- Apply pattern matching
- Build candidate lists
- Handle symlinks

**Scanning Strategy:**
```rust
pub trait Scanner {
    fn scan(&self, root: &Path) -> Result<Vec<ScanResult>>;
    fn matches(&self, path: &Path) -> bool;
}

pub struct ParallelScanner {
    patterns: PatternMatcher,
    max_depth: usize,
    follow_symlinks: bool,
}
```

### 4.4 Cleaner Component

**Responsibilities:**
- Execute deletions
- Handle errors gracefully
- Track progress
- Generate reports

**Deletion Strategy:**
```rust
pub trait Cleaner {
    fn clean(&self, items: Vec<CleanItem>) -> Result<CleanReport>;
    fn dry_run(&self, items: Vec<CleanItem>) -> Result<DryRunReport>;
}

pub struct SafeCleaner {
    confirmation: bool,
    atomic: bool,
    recovery_log: Option<RecoveryLog>,
}
```

### 4.5 Pattern Matching Engine

**Pattern Types:**
1. **Literal Patterns**: Exact directory/file names
2. **Glob Patterns**: Wildcards (*, ?, [])
3. **Regex Patterns**: Complex matching
4. **Composite Patterns**: Combinations

**Pattern Hierarchy:**
```
Built-in Patterns (Highest Priority)
    ↓
User Config Patterns
    ↓
CLI Include Patterns
    ↓
Exclusion Patterns (Override all)
```

## 5. Safety Architecture

### 5.1 Safety Layers

```
┌─────────────────────────────┐
│   Pre-execution Checks      │
│   - Git repo detection      │
│   - Permission validation   │
│   - Space availability      │
└──────────────┬──────────────┘
               ▼
┌─────────────────────────────┐
│   Confirmation Layer        │
│   - User prompts            │
│   - Dry-run preview         │
│   - Statistics display      │
└──────────────┬──────────────┘
               ▼
┌─────────────────────────────┐
│   Execution Guards          │
│   - Atomic operations       │
│   - Error boundaries        │
│   - Recovery logging        │
└──────────────┬──────────────┘
               ▼
┌─────────────────────────────┐
│   Post-execution            │
│   - Verification            │
│   - Report generation       │
│   - Cleanup validation      │
└─────────────────────────────┘
```

### 5.2 Error Handling Strategy

```rust
#[derive(Debug, Error)]
pub enum McError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Permission denied: {path}")]
    PermissionDenied { path: PathBuf },

    #[error("Pattern error: {0}")]
    Pattern(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Safety check failed: {0}")]
    Safety(String),
}

// Recovery mechanism
pub struct Recovery {
    log: Vec<RecoveryEntry>,
    rollback: bool,
}
```

## 6. Performance Optimizations

### 6.1 Parallelization Strategy

- **Work Stealing**: Rayon's work-stealing scheduler
- **Chunk Size**: Dynamic based on file count
- **Thread Pool**: Configurable, defaults to CPU count
- **I/O Batching**: Group operations to reduce syscalls

### 6.2 Memory Management

- **Streaming**: Process files in chunks
- **Buffer Reuse**: Recycle buffers for path operations
- **Lazy Evaluation**: Defer size calculations
- **Arena Allocation**: For temporary strings

### 6.3 Optimization Techniques

```rust
// Optimized path checking
pub struct PathCache {
    cache: DashMap<PathBuf, bool>,
    max_size: usize,
}

// Batch deletion for efficiency
pub fn batch_delete(paths: &[PathBuf]) -> Result<()> {
    paths.par_chunks(100)
        .try_for_each(|chunk| {
            chunk.iter().try_for_each(|p| fs::remove_dir_all(p))
        })
}
```

## 7. Platform Abstractions

### 7.1 OS-Specific Implementations

```rust
#[cfg(target_os = "windows")]
mod windows {
    pub fn prepare_path(path: &Path) -> PathBuf {
        // Handle long paths, UNC paths
    }
}

#[cfg(unix)]
mod unix {
    pub fn check_permissions(path: &Path) -> Result<()> {
        // Unix permission checks
    }
}
```

### 7.2 File System Operations

- **Cross-platform paths**: Use PathBuf consistently
- **Permission handling**: Abstract OS differences
- **Symbolic links**: Platform-specific behavior
- **Case sensitivity**: Handle appropriately

## 8. Testing Architecture

### 8.1 Test Strategy

```
Unit Tests (70%)
├── Component isolation
├── Mock file systems
└── Edge case coverage

Integration Tests (20%)
├── Component interaction
├── Real file operations
└── Configuration loading

End-to-End Tests (10%)
├── Full command execution
├── Performance validation
└── Cross-platform verification
```

### 8.2 Test Infrastructure

```rust
// Test utilities
pub mod test_utils {
    pub struct TempProject {
        root: TempDir,
        structure: ProjectStructure,
    }

    impl TempProject {
        pub fn node_project() -> Self { /* ... */ }
        pub fn rust_project() -> Self { /* ... */ }
        pub fn mixed_project() -> Self { /* ... */ }
    }
}
```

## 9. Monitoring and Telemetry

### 9.1 Metrics Collection

- **Performance Metrics**: Execution time, files/sec
- **Resource Metrics**: Memory usage, CPU utilization
- **Operation Metrics**: Files scanned, deleted, errors
- **Pattern Metrics**: Match rates, pattern efficiency

### 9.2 Progress Reporting

```rust
pub struct ProgressReporter {
    bar: ProgressBar,
    stats: Arc<RwLock<Statistics>>,
}

impl ProgressReporter {
    pub fn update(&self, event: ProgressEvent) {
        // Real-time progress updates
    }
}
```

## 10. Extension Points

### 10.1 Plugin Architecture (Future)

```rust
pub trait CleanPlugin {
    fn name(&self) -> &str;
    fn patterns(&self) -> Vec<Pattern>;
    fn pre_clean(&self, context: &Context) -> Result<()>;
    fn post_clean(&self, report: &CleanReport) -> Result<()>;
}
```

### 10.2 Custom Patterns

Users can extend pattern matching via:
- Configuration files
- Environment variables
- Command-line arguments
- Plugin system (future)

## 11. Security Considerations

### 11.1 Security Boundaries

- No network operations
- No privilege escalation
- Respect file permissions
- Validate all paths
- Sanitize user input

### 11.2 Threat Model

| Threat | Mitigation |
|--------|------------|
| Path traversal | Canonicalize paths |
| Symlink attacks | Optional symlink following |
| Race conditions | Atomic operations |
| Resource exhaustion | Limits and timeouts |

## 12. Deployment Architecture

### 12.1 Distribution Strategy

```
Binary Distribution
├── GitHub Releases (all platforms)
├── Cargo (crates.io)
├── Homebrew (macOS)
├── AUR (Arch Linux)
└── Chocolatey (Windows)
```

### 12.2 Build Pipeline

```yaml
# CI/CD Pipeline
stages:
  - lint
  - test
  - build
  - benchmark
  - release

platforms:
  - linux-x86_64
  - linux-aarch64
  - macos-x86_64
  - macos-aarch64
  - windows-x86_64
```