# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

Mr. Cleann (mc) is a high-performance parallel build directory cleaner written in Rust. It's designed to safely remove build artifacts, caches, and dependencies from development projects.

## Build and Development Commands

```bash
# Build the project (debug mode)
cargo build

# Build with optimizations (release mode)
cargo build --release

# Run the application
cargo run -- [arguments]

# Run tests (when implemented)
cargo test

# Run benchmarks (when implemented)
cargo bench

# Check code without building
cargo check

# Run linter
cargo clippy

# Format code
cargo fmt

# Install locally
cargo install --path .
```

## Architecture

The codebase follows a modular architecture with clear separation of concerns:

### Module Structure
- **`cli/`**: Command-line interface parsing using Clap
- **`config/`**: Configuration loading, validation, and merging with CLI args
- **`engine/`**: Core cleaning logic with `Scanner` and `ParallelCleaner`
- **`patterns/`**: Pattern matching for identifying files/directories to clean
- **`safety/`**: Safety guards for Git repos, confirmations, and atomic operations
- **`utils/`**: Progress reporting and file system utilities

### Key Components

1. **Main Entry Flow** (`main.rs`):
   - Parses CLI arguments
   - Loads and merges configuration
   - Handles subcommands (init, config)
   - Orchestrates scanning and cleaning

2. **Pattern Matching System**:
   - Built-in patterns for common build artifacts (node_modules, target, dist, etc.)
   - Supports glob patterns and regex
   - Priority-based pattern matching
   - Exclusion patterns for safety

3. **Parallel Processing**:
   - Uses Rayon for parallel file system traversal
   - Work-stealing scheduler for optimal performance
   - DashMap for concurrent state management

4. **Safety Features**:
   - Dry-run mode by default
   - Git repository detection
   - Confirmation prompts
   - Atomic file operations
   - Symlink preservation

## Configuration System

The tool supports hierarchical configuration:
1. Built-in defaults
2. Global config (`~/.config/mc/.mc.toml`)
3. Local config (`.mc.toml` in project)
4. CLI arguments (highest priority)

## Core Types

- `CleanItem`: Represents a file/directory to be cleaned
- `CleanReport`: Statistics about the cleaning operation
- `PatternMatcher`: Handles pattern matching logic
- `SafetyGuard`: Ensures safe operations
- `Progress`: Trait for progress reporting (with `ProgressReporter` and `NoOpProgress` implementations)

## Performance Optimizations

The release profile includes aggressive optimizations:
- Link-time optimization (LTO)
- Single codegen unit
- Binary stripping
- Panic abort mode

## Testing Approach

Tests should be added in:
- Unit tests: In-module using `#[cfg(test)]` blocks
- Integration tests: In `tests/` directory
- Benchmarks: Using Criterion in `benches/` directory

## Common Development Tasks

```bash
# Quick development cycle
cargo run -- --dry-run                    # Test on current directory
cargo run -- --dry-run /path/to/project   # Test on specific path
cargo run -- --yes --quiet                # Skip confirmation, minimal output
cargo run -- init                         # Create config file
cargo run -- config                       # Show current configuration

# Debugging
RUST_LOG=debug cargo run -- --dry-run    # Enable debug logging
cargo run -- --verbose --dry-run         # Verbose output
```

## Important Patterns

1. **Error Handling**: Uses `anyhow` for application errors and `thiserror` for library errors
2. **Progress Reporting**: Abstracted through `Progress` trait for flexibility
3. **Configuration Merging**: CLI args override config file settings
4. **Parallel Safety**: Uses `Arc` for shared state and channels for communication