# Project Overview

## Summary

**Mr. Cleann (mc)** is a high-performance, parallel build directory cleaner written in Rust. It efficiently removes build artifacts, caches, and dependencies from development projects, providing 5-10x speedup over traditional shell scripts.

## Technology Stack

| Category | Technology | Version |
|----------|------------|---------|
| **Language** | Rust | 2021 Edition |
| **Parallelism** | Rayon | 1.10 |
| **CLI Framework** | Clap | 4.5 (with derive) |
| **Configuration** | TOML (toml crate) | 0.8 |
| **File Walking** | walkdir | 2.5 |
| **Pattern Matching** | glob | 0.3 |
| **Progress Display** | indicatif | 0.17 |
| **Concurrency** | dashmap, crossbeam-channel | 6.0, 0.5 |
| **Error Handling** | anyhow, thiserror | 1.0 |
| **Serialization** | serde, serde_json | 1.0 |

## Architecture Type

- [x] **Modular CLI Application** - Clear separation of concerns across modules
- [ ] Monolithic
- [ ] Microservices
- [ ] Serverless

## Key Directories

| Directory | Purpose | Key Files |
|-----------|---------|-----------|
| `src/` | Main application code | `main.rs`, `lib.rs`, `types.rs` |
| `src/cli/` | Command-line interface | `mod.rs` (Clap argument parsing) |
| `src/config/` | Configuration management | `mod.rs` (TOML loading, defaults) |
| `src/engine/` | Core cleaning logic | `scanner.rs`, `cleaner.rs`, `mod.rs` |
| `src/patterns/` | Pattern matching | `builtin.rs`, `matcher.rs` |
| `src/safety/` | Safety guards | `guards.rs` (git detection, disk space) |
| `src/utils/` | Utilities | `progress.rs` (progress bars, tracking) |
| `benches/` | Performance benchmarks | `performance.rs` (Criterion suite) |
| `docs/` | Documentation | PRD, Architecture, Technical Spec, API |

## Core Features

1. **Parallel Processing** - Uses Rayon thread pool with work-stealing scheduler
2. **Pattern Matching** - Glob patterns for directories and files with categories
3. **Safety First** - Dry-run mode, git detection, confirmation prompts
4. **Configurable** - TOML configuration with hierarchical loading
5. **Cross-Platform** - Linux, macOS, Windows support
6. **Streaming Scanner** - Processes files without buffering entire tree in memory

## Performance Characteristics

- **Target**: Process 100,000 files in < 10 seconds
- **Parallelism**: Uses all CPU cores by default
- **Memory**: Streaming design keeps usage < 100MB
- **Release Profile**: LTO enabled, single codegen unit, stripped binaries

## Build Commands

```bash
cargo build          # Debug build
cargo build --release # Optimized build with LTO
cargo run -- --dry-run # Test without deleting
cargo test           # Run test suite
cargo bench --bench performance # Performance benchmarks
cargo clippy         # Linting
cargo fmt            # Formatting
```

## Configuration Hierarchy

1. Built-in defaults (in `src/patterns/builtin.rs`)
2. Global config (`~/.config/mc/config.toml`)
3. Local config (`.mc.toml` in project directory)
4. CLI arguments (highest priority)

## Default Cleaning Targets

### Directories
- **Build**: `dist/`, `build/`, `.next/`, `out/`, `target/`
- **Dependencies**: `node_modules/`, `.venv/`, `vendor/`
- **Cache**: `.turbo/`, `.bun/`, `.pytest_cache/`, `coverage/`
- **IDE**: `.idea/`, `.ruby-lsp/`, `.ropeproject/`

### Files
- `*.log`, `*.tsbuildinfo`
- `package-lock.json`, `bun.lock`, `uv.lock`, `Gemfile.lock`

### Always Excluded
- `.git/`, `.github/`

