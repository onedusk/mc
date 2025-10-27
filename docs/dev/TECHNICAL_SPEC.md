# Technical Specification

This document captures the performance-critical behaviour of `mc` and the knobs
available to engineers when tuning scan and clean runs.

## Streaming Scanner

- The scanner now streams `walkdir::WalkDir` entries through `rayon::par_bridge`
  so the traversal no longer buffers the entire directory tree or relies on a
  `DashMap` fan-in.
- File sizes are harvested during that single pass and rolled up to matched
  ancestor directories after the walk, eliminating the recursive size probes that
  previously re-walked every matched directory.
- Pattern checks accept an optional `FileType`, avoiding duplicate metadata syscalls
  when the caller already has that information (e.g. `DirEntry::file_type()`).

## Cleaning Pipeline

- `ParallelCleaner` owns a reusable `rayon::ThreadPool`. The pool is built once
  (and rebuilt via `with_threads`) instead of being recreated for every run.
- Work dispatch uses `par_iter().with_min_len(chunk_size)` so chunk size can be
  tuned to match the underlying storage: shrink for SSD-heavy workloads, grow for
  high-latency network shares.
- Errors are collected via a `Mutex<Vec<CleanError>>` while per-path summaries
  stay in `DashMap`, removing the crossbeam channel handoff.

## Tuning Parameters

| Component | Setting | Location | Notes |
|-----------|---------|----------|-------|
| Scanner | `max_depth` | `.mc.toml` / `Config::safety` | Caps traversal depth when working in giant monorepos. |
| Scanner | `follow_symlinks` | `.mc.toml` / `Config::options` | Disable to avoid re-scanning large symlink trees. |
| Cleaner | `parallel_threads` | `.mc.toml` / `Config::options` | Rebuilds the reusable thread pool with the requested worker count. |
| Cleaner | `chunk_size` | `ParallelCleaner::chunk_size` | Controls the minimum batch size handed to rayon splits. |

## Regression Benchmarks

- Run `cargo bench --bench performance` to execute the synthetic scanner walk
  and pruning benchmarks under `benches/performance.rs`.
- Capture baseline results for new hardware or dataset profiles and store the
  Criterion output under `docs/perf/<date>.md` (directory intentionally left
  untracked) so future runs can diff against them.
- When investigating regressions, profile the scanner first—the streamed walk and
  directory aggregation now dominate scan time in large workspaces.

## Validation Checklist

1. `cargo run -- --dry-run <path>` before and after tuning to validate real-world
   latency improvements.
2. `cargo benchmark --bench performance` to confirm no regressions in scanner or
   prune throughput.
3. `cargo test` to ensure behavioural changes stay covered by unit tests.
