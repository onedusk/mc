# File Purpose Index

## Core Application Files

| File Path | Purpose | Dependencies | Dependents |
|-----------|---------|--------------|------------|
| `src/main.rs` | CLI entry point, orchestration | `mc::*`, clap, colored | - |
| `src/lib.rs` | Library root, exports public API | All modules | External users |
| `src/types.rs` | Core types: `CleanItem`, `CleanReport`, `McError` | serde, thiserror | All modules |

## Module Index

### CLI Module (`src/cli/`)

| File | Purpose | Key Types/Functions |
|------|---------|---------------------|
| `mod.rs` | Clap argument parsing | `Cli`, `Commands` |

**Details:**
- Uses Clap derive macros for argument parsing
- Defines main CLI struct with flags: `--dry-run`, `--verbose`, `--quiet`, `--yes`
- Subcommands: `list` (JSON output), `init` (config creation), `config` (display current)

### Config Module (`src/config/`)

| File | Purpose | Key Types/Functions |
|------|---------|---------------------|
| `mod.rs` | Configuration loading and merging | `Config`, `PatternConfig`, `OptionsConfig`, `SafetyConfig` |

**Details:**
- Loads from `.mc.toml` files with hierarchical priority
- `Config::load()` - Searches current dir → ancestors → global config
- `Config::merge_cli_args()` - Merges CLI flags into loaded config
- Default values use `num_cpus::get()` for thread count

### Engine Module (`src/engine/`)

| File | Purpose | Key Types/Functions |
|------|---------|---------------------|
| `mod.rs` | Module exports, `prune_nested_items()` | - |
| `scanner.rs` | File system scanning | `Scanner` |
| `cleaner.rs` | Parallel deletion | `ParallelCleaner`, `Statistics` |

**Details:**
- **Scanner**: Streams `walkdir` entries through `rayon::par_bridge` for parallel processing
- **Cleaner**: Owns reusable `rayon::ThreadPool`, uses `par_iter().with_min_len(chunk_size)`
- **Pruning**: Removes nested items to avoid redundant deletions (e.g., files inside `node_modules/`)

### Patterns Module (`src/patterns/`)

| File | Purpose | Key Types/Functions |
|------|---------|---------------------|
| `mod.rs` | Module exports | - |
| `builtin.rs` | Built-in pattern definitions | `BUILTIN_PATTERNS`, `PatternSet` |
| `matcher.rs` | Pattern matching logic | `PatternMatcher` |

**Details:**
- `BUILTIN_PATTERNS`: Lazy-initialized `PatternSet` with categorized directories/files
- `PatternMatcher::matches_with_type()`: Checks path against compiled glob patterns
- Exclusions always take precedence over includes

### Safety Module (`src/safety/`)

| File | Purpose | Key Types/Functions |
|------|---------|---------------------|
| `mod.rs` | Module exports | - |
| `guards.rs` | Safety validation | `SafetyGuard` |

**Details:**
- `SafetyGuard::validate()`: Checks path exists, git repo detection, disk space
- Git detection: Walks ancestors looking for `.git` directory
- Disk space check: Placeholder implementation returns `u64::MAX`

### Utils Module (`src/utils/`)

| File | Purpose | Key Types/Functions |
|------|---------|---------------------|
| `mod.rs` | Module exports | - |
| `progress.rs` | Progress reporting | `Progress` trait, `ProgressReporter`, `NoOpProgress`, `CategoryTracker`, `CompactDisplay` |

**Details:**
- `Progress` trait: `increment()`, `set_message()`, `finish()`
- `ProgressReporter`: Uses `indicatif::ProgressBar`
- `CategoryTracker`: Atomic counters per `PatternCategory` for statistics
- `CompactDisplay`: 3-line spinner/bar display for scanning/cleaning

## Configuration Files

| File | Purpose | Modified Frequency |
|------|---------|-------------------|
| `Cargo.toml` | Dependencies & build config | Rarely |
| `mc.toml` | Example/default config | Rarely |
| `.mc.toml` | Local project config | Per-project |

## Documentation Files

| File | Purpose |
|------|---------|
| `docs/dev/PRD.md` | Product requirements document |
| `docs/dev/ARCHITECTURE.md` | System architecture with diagrams |
| `docs/dev/TECHNICAL_SPEC.md` | Performance tuning parameters |
| `docs/dev/API.md` | Public API reference |
| `docs/CLAUDE.md` | AI assistant guidance |
| `docs/AGENTS.md` | Repository guidelines for contributors |

## Benchmark Files

| File | Purpose |
|------|---------|
| `benches/performance.rs` | Criterion benchmarks for scanner and pruning |

