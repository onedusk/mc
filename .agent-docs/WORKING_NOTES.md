# Agent Working Notes

## Critical Files (Modify with Caution)

1. **`src/engine/cleaner.rs`** - Core deletion logic
   - Contains `fs::remove_dir_all()` calls - destructive operations
   - Thread pool management - changing can affect all parallel operations
   - Dry-run logic must be maintained for safety

2. **`src/safety/guards.rs`** - Safety validation
   - Git repository detection - prevents accidental deletion in repos
   - Disk space checks (currently placeholder)
   - Changes here affect all safety guarantees

3. **`src/patterns/builtin.rs`** - Default patterns
   - `BUILTIN_PATTERNS` defines what gets deleted by default
   - Adding patterns affects all users without config override
   - Removing patterns is breaking change

4. **`src/types.rs`** - Core type definitions
   - Changes to `CleanItem`, `CleanReport` affect all modules
   - Error types used throughout codebase
   - Serializable - API contract with external tools

## Common Tasks

### Adding a New Cleaning Pattern

1. Add to `src/patterns/builtin.rs`:
```rust
// In BUILTIN_PATTERNS
categorized_dirs: vec![
    // existing...
    ("new_pattern", PatternCategory::Cache),
]
```

2. If new category needed, add to `src/types.rs`:
```rust
pub enum PatternCategory {
    // existing...
    NewCategory,
}
```

3. Update `PatternCategory::label()` for display

4. Test with `cargo run -- --dry-run`

### Adding a New CLI Flag

1. Add to `Cli` struct in `src/cli/mod.rs`:
```rust
#[arg(long = "new-flag")]
pub new_flag: bool,
```

2. Handle in `src/main.rs` `run()` function

3. Update config merging if needed in `src/config/mod.rs`

### Adding a New Subcommand

1. Add variant to `Commands` enum in `src/cli/mod.rs`:
```rust
#[derive(Subcommand, Clone)]
pub enum Commands {
    // existing...
    NewCommand {
        #[arg(long)]
        some_option: bool,
    },
}
```

2. Handle in `handle_command()` in `src/main.rs`

### Modifying Scanner Behavior

1. Edit `src/engine/scanner.rs`
2. Key method: `Scanner::scan()` 
3. Pattern matching: `PatternMatcher::matches_with_type()`
4. Always test with `cargo run -- --dry-run` first
5. Check benchmarks: `cargo bench --bench performance`

### Running Tests

```bash
cargo test                      # All tests
cargo test test_name            # Specific test
cargo test --lib                # Library tests only
cargo test -- --nocapture       # Show println output
```

### Debugging

```bash
RUST_LOG=debug cargo run -- --dry-run .   # Enable debug logging
cargo run -- --verbose --dry-run .        # Verbose output
```

## Known Issues / TODOs

1. **Disk space check is placeholder** (`src/safety/guards.rs:93`)
   - `get_free_space()` returns `u64::MAX`
   - Should use `fs2` crate or platform-specific APIs

2. **Verbose flag not implemented** (`src/cli/mod.rs:26`)
   - Flag exists but has no effect
   - TODO: Add detailed logging output

3. **No undo/rollback capability**
   - Files are permanently deleted
   - No recovery log maintained
   - Consider adding trash integration (platform-specific)

4. **Symlink handling edge cases**
   - Windows vs Unix differences in `delete_item()`
   - Symlink cycles detected but not all edge cases covered

5. **Configuration validation incomplete**
   - No validation of glob pattern syntax until runtime
   - Could add early validation in `Config::load()`

## Performance Considerations

1. **Scanner is the bottleneck** - Profile here first when optimizing
   - Streamed walk + directory aggregation dominates scan time
   - `par_bridge()` parallelizes well on multi-core systems

2. **Chunk size tuning** in `ParallelCleaner`:
   - Default: 100 items per chunk
   - SSD: Can decrease for more parallelism
   - Network shares: Increase to reduce overhead

3. **Memory usage**:
   - Streaming design avoids buffering entire tree
   - `ScanAccumulator` collects all file sizes for directory totals
   - Large directories with millions of files may accumulate significant data

4. **Thread pool reuse**:
   - `ParallelCleaner` owns `Arc<ThreadPool>` - reused across operations
   - `with_threads()` rebuilds the pool (expensive)

## Testing Checklist

Before merging changes:

- [ ] `cargo check` passes
- [ ] `cargo test` passes
- [ ] `cargo clippy --all-targets --all-features` clean
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo run -- --dry-run <test_path>` shows expected behavior
- [ ] `cargo bench --bench performance` shows no regressions
- [ ] README/docs updated if behavior changes

## Useful Commands

```bash
# Quick validation cycle
cargo check && cargo test && cargo clippy

# Full release build
cargo build --release

# Run on specific directory
cargo run -- --dry-run ~/projects/test-app

# Generate documentation
cargo doc --open

# Watch mode (requires cargo-watch)
cargo watch -x check -x test
```

## External Dependencies of Note

| Crate | Purpose | Notes |
|-------|---------|-------|
| `rayon` | Parallelism | Work-stealing scheduler, thread pool |
| `walkdir` | Directory traversal | Streaming iterator, symlink handling |
| `glob` | Pattern matching | Standard glob syntax |
| `indicatif` | Progress bars | Terminal UI |
| `dashmap` | Concurrent HashMap | Lock-free operations |
| `serde` | Serialization | TOML config, JSON output |
| `clap` | CLI parsing | Derive-based, subcommands |
| `thiserror` | Error types | Derive Error trait |
| `anyhow` | Error handling | Application-level errors |

## Contact Points / Documentation

- Architecture: `docs/dev/ARCHITECTURE.md`
- Technical tuning: `docs/dev/TECHNICAL_SPEC.md`
- API reference: `docs/dev/API.md`
- Agent guidelines: `docs/AGENTS.md`, `docs/CLAUDE.md`

