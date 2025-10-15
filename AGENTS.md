# Repository Guidelines

## Project Structure & Module Organization
- Core entry point lives in `src/main.rs`, delegating to `mc::run` and orchestrating the CLI flow.
- `src/cli` defines Clap parsers and subcommands, `src/config` loads `.mc.toml`, and `src/patterns` encapsulates include/exclude glob logic.
- Cleaning logic is split between `src/engine` (scanner and parallel executor), `src/safety` (guardrails for git and disk checks), `src/utils` (progress reporting), and shared types in `src/types.rs`.
- Refer to `docs/ARCHITECTURE.md` and `docs/TECHNICAL_SPEC.md` for deeper design notes; treat everything under `target/` as disposable build output.

## Build, Test, and Development Commands
- `cargo check` verifies the crate quickly; run it before opening a PR.
- `cargo build --release` produces the optimized binary used in benchmarks and real runs.
- `cargo fmt --all` and `cargo clippy --all-targets --all-features` enforce formatting and linting; use `--fix` on Clippy only after reviewing warnings.
- `cargo run -- --dry-run` is the recommended smoke test; pass a path (e.g., `cargo run -- ~/projects/app --dry-run`) to exercise real inputs.

## Coding Style & Naming Conventions
- Rustfmt defaults apply (4-space indentation, trailing newline, max width 100); never hand-format files that `cargo fmt` can handle.
- Modules and files stay `snake_case`, types and traits `PascalCase`, and constants `SCREAMING_SNAKE_CASE`; keep module boundaries aligned with the directories listed above.
- Prefer `mc::Result<T>` and error types in `src/types.rs`; surface user-facing errors with `colored` styling just like `main.rs`.

## Testing Guidelines
- Use `cargo test` for unit coverage; colocate `#[cfg(test)]` modules beside the code or add future integration suites under `tests/`.
- Property and filesystem-heavy tests should lean on `proptest`, `tempfile`, and `assert_fs` so patterns mirror production scans.
- Name tests after behavior (`test_scanner_respects_excludes`) and include at least one dry-run assertion when touching the cleaner.

## Commit & Pull Request Guidelines
- No canonical history exists yet; adopt Conventional Commits (`feat:`, `fix:`, `refactor:`) to make future release notes easier.
- Squash noisy WIP commits before review, describe scope and validation in the PR body, and link tracking issues or docs when relevant.
- Attach terminal output or screenshots when the change alters CLI UX, and call out any risk to filesystem safety or nuclear mode defaults.
