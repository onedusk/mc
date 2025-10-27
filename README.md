---
claude: docs/CLAUDE.md
codex: docs/AGENTS.md
gemini: docs/GEMINI.md
license: docs/LICENSE
contributing: docs/CONTRIBUTING.md
changelog: docs/CHANGELOG.md
conduct: docs/CODE_OF_CONDUCT.md
security: docs/SECURITY.md
---

# Mr. Cleann (mc)

A parallel build directory cleaner for modern development workflows.

## Features

- **Fast**: Parallel processing with Rayon for maximum performance
- **Safe by Default**: Dry-run mode, Git detection, and confirmation prompts
- **Patterns**: Pre-configured patterns for common build artifacts
- **Configurable**: TOML-based configuration with sensible defaults
- **Detailed Statistics**: Track space freed and items cleaned
- **Cross-Platform**: Works on Linux, macOS, and Windows

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/onedusk/mc.git
cd mc

# Build with optimizations
cargo build --release

# Install to PATH (optional)
cargo install --path .
```

## Usage

### Basic Usage

```bash
# Clean current directory (with confirmation)
mc

# Dry run - preview what will be deleted
mc --dry-run

# Clean specific directory
mc /path/to/project

# Skip confirmation prompt
mc --yes

# Quiet mode for scripts
mc --quiet --yes
```

### Advanced Options

```bash
# Exclude specific patterns
mc --exclude "important_cache" --exclude "*.env"

# Include additional patterns
mc --include "*.tmp" --include "temp_*"

# Use custom configuration
mc --config ./my-config.toml

# Parallel threads control
mc --parallel 8

# Nuclear mode - includes dangerous operations
mc --nuclear

# Preserve environment files
mc --preserve-env
```

## Configuration

Create a `.mc.toml` file in your project or home directory:

```toml
[patterns]
# Directories to clean
directories = [
    "dist", "build", ".next", "out", "target",
    "node_modules", ".turbo", "coverage"
]

# Files to clean (glob patterns)
files = [
    "*.log",
    "*.tsbuildinfo"
]

# Patterns to always exclude
exclude = [
    ".git",
    ".env.local"
]

[options]
parallel_threads = 8
require_confirmation = true
show_statistics = true
preserve_symlinks = true

[safety]
check_git_repo = true
max_depth = 10
min_free_space_gb = 1.0
```

### Initialize Configuration

```bash
# Create local config
mc init

# Create global config
mc init --global

# View current config
mc config
```

## Default Cleaning Patterns

### Directories

- **Build Outputs**: `dist/`, `build/`, `.next/`, `out/`, `target/`
- **Dependencies**: `node_modules/`, `.venv/`, `vendor/`
- **Caches**: `.turbo/`, `.pytest_cache/`, `coverage/`, `.bun/`
- **IDE Files**: `.idea/`, `.ruby-lsp/`

### Files

- Log files: `*.log`
- TypeScript build info: `*.tsbuildinfo`
- Package locks: `package-lock.json`, `bun.lock`, `uv.lock`, `Gemfile.lock`

## Safety Features

1. **Dry Run Mode**: Preview deletions without executing
2. **Git Repository Detection**: Warns when operating in Git repos
3. **Confirmation Prompts**: Requires user confirmation by default
4. **Exclusion Patterns**: Never deletes critical files like `.git`
5. **Atomic Operations**: Safe file operations with error recovery

## Performance

Mr. Cleann uses parallel processing to maximize performance:

- Utilizes all CPU cores by default
- Work-stealing scheduler via Rayon
- Efficient I/O batching
- Memory-efficient streaming for large directories

Benchmarks show 5-10x speed improvement over sequential shell scripts on large codebases.

## Examples

### Clean a Node.js Project

```bash
mc --dry-run
# Review what will be deleted
mc --yes
```

### Clean Multiple Projects

```bash
# Create a script
for dir in ~/projects/*; do
    echo "Cleaning $dir"
    mc --quiet --yes "$dir"
done
```

### CI/CD Pipeline Usage

```bash
# In your CI script
mc --yes --quiet --stats
```

### Custom Pattern Cleaning

```bash
# Clean only specific patterns
mc --include "*.cache" --include "tmp/*" --exclude ".env"
```

## Documentation

- [Product Requirements Document](docs/PRD.md)
- [Architecture Guide](docs/ARCHITECTURE.md)
- [Technical Specification](docs/TECHNICAL_SPEC.md)
- [API Documentation](docs/API.md)

## Contributing

Contributions are welcome! Please read the documentation in the `docs/` folder to understand the architecture and follow the guidelines in [`AGENTS.md`](AGENTS.md) when contributing.

## License

MIT

## Acknowledgments

Built with Rust and powered by:

- [Rayon](https://github.com/rayon-rs/rayon) for parallelism
- [Clap](https://github.com/clap-rs/clap) for CLI parsing
- [Indicatif](https://github.com/console-rs/indicatif) for progress bars

---

**Note**: This tool performs destructive operations. Always use `--dry-run` first to preview changes.
