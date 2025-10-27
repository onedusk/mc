# Implementation Notes - Mr. Cleann

## Style Guidelines

### Repository Standards (per CLAUDE.md)
- **NO EMOJIS**: All documentation should avoid emoji usage
- **Clean, professional style**: Focus on clarity and technical accuracy
- **Consistent formatting**: Use standard markdown without decorative elements

### Applied Changes
- Removed emojis from README.md (title and feature list)
- Updated all documentation to follow clean style guidelines
- Future contributions should maintain this standard

## Directory Structure Clarification

### Actual Implementation vs. Planned Structure

During implementation, some architectural decisions were made that differ from the initial plan:

#### Parallel Processing Module
**Planned**: Separate `src/parallel/` module with:
- `executor.rs`
- `scheduler.rs`

**Actual Implementation**: Integrated directly into `src/engine/cleaner.rs`
- Reason: Simpler architecture, less abstraction overhead
- The `ParallelCleaner` struct handles all parallel execution
- Rayon's built-in work-stealing scheduler is used directly
- No need for separate executor/scheduler abstractions

### Current Module Structure

```
src/
├── cli/           ✅ Command-line interface
│   └── mod.rs
├── config/        ✅ Configuration management
│   └── mod.rs
├── engine/        ✅ Core engine (includes parallel processing)
│   ├── mod.rs
│   ├── scanner.rs    # Parallel scanning with Rayon
│   └── cleaner.rs    # Parallel cleaning with Rayon
├── patterns/      ✅ Pattern matching
│   ├── mod.rs
│   ├── builtin.rs
│   └── matcher.rs
├── safety/        ✅ Safety guards
│   ├── mod.rs
│   └── guards.rs
├── utils/         ✅ Utilities
│   ├── mod.rs
│   └── progress.rs
├── lib.rs         ✅ Public API
├── main.rs        ✅ CLI application
└── types.rs       ✅ Type definitions
```

## Parallel Processing Implementation Details

### Location: `src/engine/cleaner.rs`

The parallel processing is implemented using:

1. **Rayon's Thread Pool**:
   ```rust
   rayon::ThreadPoolBuilder::new()
       .num_threads(self.thread_count)
       .build()
   ```

2. **Parallel Iteration**:
   ```rust
   items
       .par_iter()
       .with_min_len(self.chunk_size)
       .for_each(|item| {
           // Process each item in parallel
       })
   ```

3. **Concurrent Data Structures**:
   - `DashMap` for concurrent hash maps
   - `AtomicUsize` and `AtomicU64` for statistics
   - `Mutex`-protected error accumulator shared across worker threads

### Why No Separate Parallel Module?

1. **Simplicity**: Rayon provides excellent abstractions that don't need wrapping
2. **Performance**: Direct use of Rayon avoids additional abstraction overhead
3. **Maintainability**: Less code to maintain, clearer data flow
4. **Integration**: Parallel processing is tightly coupled with cleaning logic

## Other Implementation Decisions

### 1. Error Handling
- Changed `std::io::Error` to `String` in `CleanError::IoError` to enable `Clone`
- This was necessary for storing errors in `DashMap`

### 2. Pattern Matching
- Patterns match on file/directory names, not full paths
- This is more intuitive and matches user expectations
- Exclusion patterns are checked first for safety

### 3. Progress Reporting
- Optional progress reporting via trait abstraction
- `NoOpProgress` for quiet mode
- `ProgressReporter` with indicatif for visual feedback

### 4. Safety Guards
- Git detection walks up directory tree
- Free space check simplified for initial implementation
- Max depth prevents infinite recursion

## Future Improvements

### Potential Parallel Module
If more complex parallel operations are needed in the future, consider:

1. **Batch Processing Pipeline**:
   - Separate scanning and deletion stages
   - Pipeline parallelism for better throughput

2. **Advanced Scheduling**:
   - Priority-based deletion
   - Size-based chunking
   - Dynamic thread pool sizing

3. **Parallel Pattern Matching**:
   - Concurrent pattern compilation
   - Parallel pattern evaluation

### Performance Optimizations
1. **I/O Batching**: Group file operations to reduce syscalls
2. **Memory Pooling**: Reuse allocations for path operations
3. **Lazy Evaluation**: Defer size calculations until needed
4. **Cache Warming**: Pre-fetch directory metadata

## Compilation Notes

### Known Issues
Some compilation warnings may appear related to:
- Unused imports (cleaned up in production)
- Clone implementations (required for DashMap storage)
- Type conversions (handled safely)

### Build Optimization
The release profile is configured for maximum performance:
```toml
[profile.release]
opt-level = 3        # Maximum optimization
lto = true          # Link-time optimization
codegen-units = 1   # Single codegen unit for better optimization
strip = true        # Strip symbols for smaller binary
panic = "abort"     # Abort on panic (smaller binary)
```

## Testing Considerations

### Unit Tests (Pending)
- Pattern matching tests
- Configuration loading tests
- Safety guard validation tests

### Integration Tests (Pending)
- Full cleaning workflow tests
- Dry-run verification tests
- Error handling tests

### Performance Benchmarks (Pending)
- Scanning performance with varying file counts
- Parallel deletion benchmarks
- Memory usage profiling

## Platform-Specific Notes

### Unix/Linux
- Symlink handling uses Unix-specific APIs
- File permissions checked with standard Unix permissions

### Windows
- Symlink handling differs (directory vs file symlinks)
- Path length limitations (260 chars) may apply
- Case-insensitive pattern matching considerations

### macOS
- Similar to Unix with some specific behaviors
- .DS_Store files commonly found (not in default patterns)
- Resource forks not specifically handled

## Security Considerations

1. **Path Traversal**: Paths are canonicalized to prevent traversal attacks
2. **TOCTOU**: Time-of-check to time-of-use gaps minimized but not eliminated
3. **Resource Limits**: Max depth and timeout prevent resource exhaustion
4. **Privilege Escalation**: No privilege elevation attempted

## Maintenance Notes

### Adding New Patterns
1. Add to `BUILTIN_PATTERNS` in `src/patterns/builtin.rs`
2. Update documentation in README.md
3. Consider safety implications

### Modifying Parallel Behavior
1. Adjust `chunk_size` in `ParallelCleaner` for different workloads
2. Thread count can be controlled via CLI or config
3. Consider memory usage with large file counts

### Debugging
- Use `RUST_LOG=debug` for verbose output
- `--verbose` flag for user-facing debug info
- Progress bars can be disabled with `--quiet`
