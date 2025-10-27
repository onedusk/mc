# Repository Guidelines

## Project Structure & Module Organization
Core entry point sits in `src/main.rs`, handing CLI flow to `mc::run` and wiring Clap parsing from `src/cli`. Configuration readers live in `src/config`, glob handling in `src/patterns`, and cleaning logic is split between `src/engine`, `src/safety`, and `src/utils`, with shared types centralized in `src/types.rs`. Refer to `docs/ARCHITECTURE.md` and `docs/TECHNICAL_SPEC.md` before touching execution flow, and treat `target/` as disposable build output. Tests belong beside their modules or under `tests/` for future integration suites.

## Build, Test, and Development Commands
- `cargo check` validates the crate quickly; run it before every branch push to catch obvious regressions.
- `cargo fmt --all` and `cargo clippy --all-targets --all-features` keep formatting and lints consistent; review warnings before applying `--fix`.
- `cargo build --release` produces the optimized binary used for benchmarks and field runs.
- `cargo run -- --dry-run ~/projects/app` exercises the CLI path and prints planned mutations without touching disk state.

## Coding Style & Naming Conventions
Let rustfmt drive layout (4 spaces, trailing newline, ~100 column width). Modules and files stay `snake_case`, exposed types and traits use `PascalCase`, and constants use `SCREAMING_SNAKE_CASE`. Prefer `mc::Result<T>` plus error helpers in `src/types.rs`, and surface user-facing failures with `colored` styling consistent with `main.rs`. Follow existing module boundaries; add new directories only when the architecture docs justify them.

## Testing Guidelines
`cargo test` covers unit suites; include behavioral test names such as `test_scanner_respects_excludes`. Use `proptest`, `tempfile`, or `assert_fs` when asserting filesystem walks, and include at least one dry-run assertion whenever modifying cleaner logic. Keep fixtures lightweight and colocate them with the code they verify to reduce churn between modules.

## Commit & Pull Request Guidelines
Use Conventional Commits (`feat:`, `fix:`, `refactor:`, etc.) so release tooling can infer changelogs. Squash intermediary WIP commits, describe scope and validation steps in the PR body, and link related issues or docs for context. Provide terminal output or screenshots whenever CLI UX changes, and call out any risk to filesystem safety or nuclear mode defaults.

## Security & Configuration Tips
`src/safety` guards against destructive runs; respect those checks instead of bypassing them locally. When editing `.mc.toml` or repo-level defaults, document new keys in `docs/` and verify `cargo run -- --dry-run` from a clean git worktree before merging.
