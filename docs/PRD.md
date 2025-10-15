# Product Requirements Document (PRD)
# Mr. Cleann (mc) - Build Directory Cleaner

## 1. Executive Summary

Mr. Cleann (`mc`) is a high-performance, Rust-based command-line utility designed to efficiently clean build artifacts, dependencies, and temporary files from large codebases. It serves as a modern, parallelized replacement for traditional shell scripts, providing safety, speed, and cross-platform compatibility.

## 2. Product Overview

### 2.1 Problem Statement
Large polyglot codebases accumulate significant amounts of build artifacts, cache files, and dependencies that:
- Consume substantial disk space (often GBs)
- Slow down file system operations
- Interfere with fresh builds
- Complicate version control operations
- Make project transfers cumbersome

Current shell-script solutions are:
- Platform-specific (bash/PowerShell)
- Sequential and slow on large codebases
- Prone to errors with special characters in paths
- Lacking proper error handling and recovery
- Missing safety features and dry-run capabilities

### 2.2 Solution
Mr. Cleann provides a robust, cross-platform solution that:
- Leverages Rust's performance and safety guarantees
- Utilizes parallel processing via Rayon for speed
- Offers comprehensive pattern matching for various build systems
- Includes safety features (dry-run, exclusions, confirmations)
- Provides detailed reporting and statistics
- Maintains a configuration system for customization

## 3. Target Users

### Primary Users
- **Software Developers**: Clean local development environments
- **DevOps Engineers**: Maintain CI/CD pipelines and build servers
- **System Administrators**: Manage disk space on development servers
- **Open Source Contributors**: Clean multiple project checkouts

### Use Cases
1. **Pre-commit Cleanup**: Remove artifacts before committing
2. **Fresh Build Preparation**: Ensure clean state for builds
3. **Disk Space Recovery**: Free space on development machines
4. **CI/CD Pipeline Steps**: Automated cleanup in pipelines
5. **Project Archival**: Clean projects before archiving
6. **Troubleshooting**: Resolve build issues via clean state

## 4. Functional Requirements

### 4.1 Core Cleaning Capabilities

#### Build Artifacts
- `dist/` - Distribution builds
- `build/` - Build outputs
- `.next/` - Next.js builds
- `out/` - Generic output directories
- `target/` - Rust/Maven builds
- `*.tsbuildinfo` - TypeScript build info

#### Dependencies
- `node_modules/` - Node.js dependencies
- `.venv/` - Python virtual environments (optional)
- `vendor/` - Vendored dependencies

#### Cache Files
- `.turbo/` - Turborepo cache
- `.bun/` - Bun runtime cache
- `.pytest_cache/` - Python test cache
- `.benchmark-cache/` - Benchmark caches
- `coverage/` - Coverage reports

#### Development Tools
- `.idea/` - IntelliJ IDEA files
- `.ruby-lsp/` - Ruby LSP cache
- `.ropeproject/` - Python Rope refactoring cache

#### Package Manager Locks (Optional)
- `package-lock.json`
- `bun.lock`
- `uv.lock`
- `Gemfile.lock`

#### Log Files
- `*.log` - All log files

### 4.2 Command-Line Interface

```bash
mc [OPTIONS] [PATH]

OPTIONS:
    -d, --dry-run           Preview what would be deleted
    -v, --verbose          Verbose output
    -q, --quiet            Suppress non-essential output
    -y, --yes              Skip confirmation prompts
    -e, --exclude <PATTERN> Exclude patterns (can be repeated)
    -i, --include <PATTERN> Additional patterns to clean
    -c, --config <FILE>    Use custom configuration file
    -s, --stats            Show statistics after cleaning
    -p, --parallel <N>     Number of parallel threads (default: CPU count)
    --no-git-check         Don't check for .git directories
    --preserve-env         Preserve .env files
    --nuclear              Include dangerous operations (git, env files)
    -h, --help             Display help
    -V, --version          Display version

ARGUMENTS:
    <PATH>                 Path to clean (default: current directory)
```

### 4.3 Configuration System

Configuration file (`.mc.toml` or `~/.config/mc/config.toml`):

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
# Default options
parallel_threads = 8
require_confirmation = true
show_statistics = true
preserve_symlinks = true

[safety]
# Safety checks
check_git_repo = true
max_depth = 10
min_free_space_gb = 1.0
```

### 4.4 Safety Features

1. **Dry Run Mode**: Preview all operations without executing
2. **Git Repository Detection**: Warn when operating in git repos
3. **Confirmation Prompts**: Require user confirmation for destructive operations
4. **Exclusion Patterns**: Never delete certain critical files
5. **Symlink Preservation**: Option to preserve symbolic links
6. **Atomic Operations**: Use atomic file operations where possible
7. **Rollback Log**: Maintain log for potential recovery

## 5. Non-Functional Requirements

### 5.1 Performance
- Process 100,000 files in < 10 seconds on modern hardware
- Utilize all available CPU cores by default
- Memory usage < 100MB for typical operations
- Efficient I/O with batched operations

### 5.2 Reliability
- Graceful handling of permission errors
- Recovery from partial failures
- Clear error messages with actionable solutions
- Exit codes following POSIX standards

### 5.3 Usability
- Single binary distribution
- No runtime dependencies
- Cross-platform compatibility (Windows, macOS, Linux)
- Intuitive command-line interface
- Comprehensive help documentation

### 5.4 Security
- No network operations
- Respect file system permissions
- No elevation of privileges
- Safe handling of special characters in paths

## 6. Technical Constraints

### 6.1 Technology Stack
- **Language**: Rust (latest stable)
- **Parallelization**: Rayon crate
- **CLI Framework**: Clap v4
- **Configuration**: TOML via toml crate
- **File Operations**: std::fs with walkdir
- **Pattern Matching**: glob crate

### 6.2 Platform Support
- Linux (x86_64, ARM64)
- macOS (Intel, Apple Silicon)
- Windows 10+ (x86_64)

### 6.3 Build Requirements
- Rust 1.70+
- Cargo build system
- Cross-compilation support

## 7. Success Metrics

### 7.1 Performance Metrics
- Execution time vs. shell script (target: 5-10x faster)
- Files processed per second
- Memory efficiency
- CPU utilization

### 7.2 Adoption Metrics
- GitHub stars and forks
- Download statistics
- Issue resolution time
- Community contributions

### 7.3 Quality Metrics
- Test coverage (target: >80%)
- Zero critical bugs in production
- Documentation completeness
- Cross-platform compatibility

## 8. Milestones

### Phase 1: MVP (Week 1-2)
- Core cleaning functionality
- Basic CLI interface
- Essential patterns support
- Dry-run mode

### Phase 2: Enhancement (Week 3-4)
- Parallel processing optimization
- Configuration system
- Extended pattern support
- Statistics and reporting

### Phase 3: Polish (Week 5-6)
- Cross-platform testing
- Performance optimization
- Documentation completion
- Distribution packaging

### Phase 4: Release (Week 7-8)
- Binary releases
- Package manager distribution
- Community feedback integration
- Long-term maintenance plan

## 9. Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Data Loss | High | Dry-run default, confirmations, exclusions |
| Performance Regression | Medium | Benchmarking suite, profiling |
| Platform Incompatibility | Medium | CI/CD testing matrix |
| Pattern Matching Errors | Low | Extensive test suite, user feedback |

## 10. Future Enhancements

- **Cloud Integration**: Clean cloud storage buckets
- **Project Templates**: Predefined cleaning profiles
- **Undo Functionality**: Restore recently cleaned files
- **GUI Version**: Graphical interface for non-CLI users
- **Integration Plugins**: IDE and editor plugins
- **Scheduled Cleaning**: Cron/scheduler integration
- **Space Estimation**: Pre-calculate space to be freed
- **Custom Hooks**: Pre/post cleaning scripts

## 11. Appendix

### A. Comparison with Existing Tools

| Feature | Shell Script | Mr. Cleann |
|---------|-------------|------------|
| Speed | Sequential | Parallel |
| Safety | Basic | Advanced |
| Platform | Unix only | Cross-platform |
| Config | Hard-coded | Flexible |
| Dry-run | Manual | Built-in |
| Recovery | None | Logging |

### B. Example Usage Scenarios

```bash
# Basic cleaning
mc

# Dry run with statistics
mc --dry-run --stats

# Clean specific directory with exclusions
mc --exclude "*.env" --exclude "important/" /path/to/project

# Aggressive cleaning with confirmation
mc --nuclear

# Quiet mode for scripts
mc --quiet --yes

# Custom configuration
mc --config ./my-clean-config.toml
```