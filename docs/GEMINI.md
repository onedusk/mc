# GEMINI.md

## Project Overview

This project is a high-performance, parallel build directory cleaner named "Mr. Cleann" (`mc`). It is a command-line interface (CLI) tool written in Rust, designed to efficiently clean up build artifacts, caches, and other temporary files from development projects.

**Key Features:**

*   **Performance:** Utilizes parallel processing with the `rayon` crate to maximize cleaning speed.
*   **Safety:** Implements safety features such as dry-run mode, Git repository detection, and interactive confirmation prompts to prevent accidental data loss.
*   **Configuration:** Supports project-specific (`.mc.toml`) and global configuration files for defining cleaning patterns and behavior.
*   **Pattern Matching:** Uses glob patterns to identify files and directories to be cleaned, with pre-configured patterns for common development environments (Node.js, Rust, etc.).
*   **Cross-Platform:** Designed to run on Linux, macOS, and Windows.

**Architecture:**

The application is structured into several modules:

*   `cli`: Defines the command-line interface using the `clap` crate.
*   `config`: Manages loading and merging of configuration from files and CLI arguments.
*   `engine`: Contains the core logic for scanning and cleaning files, including the `Scanner` and `ParallelCleaner`.
*   `patterns`: Handles the matching of file paths against user-defined and built-in patterns.
*   `safety`: Implements safety checks, such as Git repository validation.
*   `utils`: Provides utility components, like progress reporting.

## Building and Running

### Building

To build the project, use the following `cargo` command:

```bash
# Build in release mode with optimizations
cargo build --release
```

The compiled binary will be located at `target/release/mc`.

### Running

The tool can be run directly using `cargo` or by installing it to your system's PATH.

**Basic Commands:**

```bash
# Run the cleaner on the current directory
cargo run

# Perform a dry run to see what would be deleted
cargo run -- --dry-run

# Clean a specific directory
cargo run -- /path/to/your/project

# Run with --yes to skip confirmation prompts
cargo run -- --yes
```

**Subcommands:**

*   `mc init`: Create a default configuration file (`.mc.toml`) in the current directory.
*   `mc init --global`: Create a global configuration file.
*   `mc config`: Display the current configuration.
*   `mc list`: List the files that would be cleaned without performing any action.

## Development Conventions

*   **CLI:** The command-line interface is defined declaratively using `clap`. All CLI arguments and subcommands are located in `src/cli/mod.rs`.
*   **Parallelism:** The `rayon` crate is used for parallelizing the file scanning and deletion processes to improve performance.
*   **Configuration:** The application uses a TOML-based configuration system, with structures defined in `src/config/mod.rs`.
*   **Error Handling:** The project uses the `anyhow` and `thiserror` crates for robust error handling.
*   **Dependencies:** All dependencies are managed in the `Cargo.toml` file. Key dependencies include `clap`, `rayon`, `tokio`, `glob`, and `serde`.
*   **Documentation:** The `docs` directory contains important project documentation, including the architecture, technical specifications, and API documentation. Contributors should review these documents before making changes.
*   **Testing:** The project uses `criterion` for benchmarking and `proptest` for property-based testing. Tests are located in the `tests` directory (not present in the provided file listing, but inferred from `dev-dependencies`).
