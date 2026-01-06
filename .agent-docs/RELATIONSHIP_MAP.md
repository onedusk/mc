# Component Relationship Map

## Import Graph

```
main.rs
├── mc::cli::Cli, Commands          (argument parsing)
├── mc::config::Config              (configuration loading)
├── mc::engine::ParallelCleaner     (deletion execution)
├── mc::engine::Scanner             (file system traversal)
├── mc::patterns::PatternMatcher    (glob matching)
├── mc::safety::SafetyGuard         (pre-execution validation)
├── mc::utils::*                    (progress, tracking)
└── mc::types::*                    (Result, errors)

lib.rs
├── cli/mod.rs
├── config/mod.rs
│   └── patterns::BUILTIN_PATTERNS
├── engine/mod.rs
│   ├── scanner.rs
│   │   ├── patterns::PatternMatcher
│   │   ├── types::CleanItem, ScanError
│   │   └── utils::CategoryTracker, Progress
│   └── cleaner.rs
│       ├── types::CleanError, CleanItem, CleanReport
│       └── utils::Progress
├── patterns/mod.rs
│   ├── builtin.rs
│   │   └── types::PatternCategory
│   └── matcher.rs
│       ├── config::PatternConfig
│       ├── patterns::BUILTIN_PATTERNS
│       └── types::PatternMatch, PatternCategory, PatternSource
├── safety/mod.rs
│   └── guards.rs
├── types.rs
└── utils/mod.rs
    └── progress.rs
        └── types::PatternCategory
```

## Data Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              MAIN.RS                                     │
│                                                                         │
│  1. Parse CLI args (clap)                                               │
│  2. Load Config (config/mod.rs)                                         │
│  3. Merge CLI args into Config                                          │
│  4. Validate path (SafetyGuard)                                         │
│  5. Create PatternMatcher                                               │
│  6. Scan filesystem (Scanner)                                           │
│  7. Prune nested items                                                  │
│  8. Display summary                                                     │
│  9. Prompt for confirmation                                             │
│  10. Clean items (ParallelCleaner)                                      │
│  11. Print report                                                       │
└─────────────────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                            CONFIG LOADING                               │
│                                                                         │
│  Config::load(path)                                                     │
│    ├── Check CLI-provided path                                          │
│    ├── Search .mc.toml in current dir & ancestors                       │
│    ├── Check global config (~/.config/mc/config.toml)                   │
│    └── Fall back to Config::default()                                   │
│                                                                         │
│  Default values from:                                                   │
│    └── BUILTIN_PATTERNS (patterns/builtin.rs)                          │
└─────────────────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                              SCANNING                                   │
│                                                                         │
│  Scanner::scan()                                                        │
│    ├── WalkDir::new(root)                                              │
│    │     .max_depth(config.safety.max_depth)                           │
│    │     .follow_links(follow_symlinks)                                │
│    ├── .par_bridge() ─── Rayon parallel bridge                         │
│    ├── .fold() ──────── Accumulate items + errors per thread           │
│    │     ├── PatternMatcher::matches_with_type() ── Check patterns     │
│    │     ├── Collect file sizes for directory aggregation              │
│    │     └── Collect errors (IoError, SymlinkCycle)                    │
│    ├── .reduce() ────── Merge thread-local accumulators                │
│    └── Post-process:                                                   │
│          ├── Aggregate file sizes into directory totals                │
│          └── Update CategoryTracker statistics                         │
│                                                                         │
│  Output: (Vec<CleanItem>, Vec<ScanError>)                              │
└─────────────────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         NESTED ITEM PRUNING                             │
│                                                                         │
│  prune_nested_items(items)                                             │
│    ├── Sort by path depth (shortest first)                             │
│    ├── Iterate: skip items whose ancestor is already kept              │
│    └── Return pruned list                                              │
│                                                                         │
│  Example:                                                               │
│    Input:  [node_modules, node_modules/pkg/dist, dist]                 │
│    Output: [node_modules, dist]  (nested pkg/dist removed)             │
└─────────────────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                              CLEANING                                   │
│                                                                         │
│  ParallelCleaner::clean(items)                                         │
│    ├── If dry_run: dry_run_clean() ── Display what would be deleted    │
│    └── Actual cleaning:                                                 │
│          ├── thread_pool.install() ── Use dedicated Rayon pool         │
│          ├── items.par_iter().with_min_len(chunk_size)                 │
│          ├── delete_item() per item:                                   │
│          │     ├── Directory: fs::remove_dir_all()                     │
│          │     ├── File: fs::remove_file()                             │
│          │     └── Symlink: Platform-specific handling                 │
│          ├── Atomic stats update (AtomicUsize, AtomicU64)              │
│          └── Collect errors via Mutex<Vec<CleanError>>                 │
│                                                                         │
│  Output: CleanReport { items_deleted, bytes_freed, errors, duration }  │
└─────────────────────────────────────────────────────────────────────────┘
```

## Cross-Component Dependencies

| Source | Target | Relationship |
|--------|--------|--------------|
| `Scanner` | `PatternMatcher` | Uses for path matching |
| `Scanner` | `CategoryTracker` | Updates statistics during scan |
| `Config` | `BUILTIN_PATTERNS` | Default pattern values |
| `PatternMatcher` | `BUILTIN_PATTERNS` | Category lookup |
| `ParallelCleaner` | `Progress` trait | Reports deletion progress |
| `main.rs` | `SafetyGuard` | Pre-execution validation |
| `lib.rs (Cleaner)` | `Scanner` + `ParallelCleaner` | Orchestrates both |

## Thread Safety Model

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         SHARED STATE                                    │
│                                                                         │
│  Arc<PatternMatcher>     ── Immutable, shared across scan threads      │
│  Arc<CategoryTracker>    ── DashMap + Atomics for concurrent updates   │
│  Arc<dyn Progress>       ── Thread-safe progress reporting             │
│  Arc<Statistics>         ── Atomics for items_deleted, bytes_freed     │
│  Arc<ThreadPool>         ── Reusable Rayon pool in ParallelCleaner     │
└─────────────────────────────────────────────────────────────────────────┘
```

## Type Relationships

```
CleanItem
  ├── path: PathBuf
  ├── size: u64
  ├── item_type: ItemType {Directory, File, Symlink}
  └── pattern: PatternMatch
        ├── pattern: String
        ├── priority: u32
        ├── source: PatternSource {BuiltIn, Config, CLI}
        └── category: PatternCategory {Dependencies, BuildOutputs, Cache, IDE, Logs, Other}

Config
  ├── patterns: PatternConfig
  │     ├── directories: Vec<String>
  │     ├── files: Vec<String>
  │     └── exclude: Vec<String>
  ├── options: OptionsConfig
  │     ├── parallel_threads: usize
  │     ├── require_confirmation: bool
  │     ├── show_statistics: bool
  │     └── preserve_symlinks: bool
  └── safety: SafetyConfig
        ├── check_git_repo: bool
        ├── max_depth: usize
        └── min_free_space_gb: f64

CleanReport
  ├── items_deleted: usize
  ├── bytes_freed: u64
  ├── errors: Vec<CleanError>
  ├── scan_errors: Vec<ScanError>
  ├── duration: Duration
  └── dry_run: bool
```

