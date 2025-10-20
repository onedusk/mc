//! # mc (Mr. Clean) CLI
//!
//! This is the command-line interface for `mc`, a high-performance build directory cleaner.
//!
//! It provides a user-friendly way to interact with the `mc` library, allowing users to:
//! - Clean projects based on predefined or custom patterns.
//! - Perform dry runs to see what would be deleted.
//! - Initialize configuration files.
//! - List files that would be cleaned.
//! - Customize behavior with various command-line flags.
//!
//! The CLI is built using `clap` for parsing arguments and commands.
//!
//! # Exit Codes
//!
//! - `0`: Success.
//! - `1`: An error occurred during execution. The error message will be printed to stderr.

use clap::Parser;
use colored::*;
use humansize::{format_size, DECIMAL};
use std::io::{self, Write};
use std::sync::Arc;
use std::process;

use mc::{
    cli::{Cli, Commands},
    config::Config,
    patterns::PatternMatcher,
    engine::{Scanner, ParallelCleaner},
    safety::SafetyGuard,
    utils::{NoOpProgress, CompactDisplay, CategoryTracker, Progress},
    Result,
};

/// The main entry point for the `mc` command-line application.
///
/// This function initializes `env_logger` and calls the `run` function,
/// handling any errors that occur and printing them to stderr.
fn main() {
    env_logger::init();

    if let Err(e) = run() {
        eprintln!("{} {}", "Error:".red().bold(), e);
        process::exit(1);
    }
}

/// The core logic for the `mc` CLI.
///
/// This function is responsible for:
/// - Parsing command-line arguments.
/// - Handling subcommands like `list`, `init`, and `config`.
/// - Loading the configuration.
/// - Merging CLI arguments with the configuration.
/// - Performing safety checks.
/// - Scanning for files to clean.
/// - Prompting for user confirmation.
/// - Executing the cleaning process.
/// - Printing a final report.
///
/// # Returns
///
/// Returns `Ok(())` on success. If an error occurs, it is propagated up to `main`
/// for handling. The specific error types are defined in `mc::McError`.
fn run() -> Result<()> {
    let cli = Cli::parse();

    // Handle subcommands
    if let Some(command) = &cli.command {
        return handle_command(command.clone(), &cli);
    }

    // Load configuration
    let mut config = Config::load(cli.config.as_ref())?;

    // Merge CLI arguments
    config.merge_cli_args(cli.exclude, cli.include, cli.preserve_env);

    // Override config with CLI flags
    if cli.no_git_check {
        config.safety.check_git_repo = false;
    }

    if let Some(threads) = cli.parallel {
        config.options.parallel_threads = threads;
    }

    // Validate path
    let path = cli.path.canonicalize()
        .map_err(|e| mc::McError::Io(e))?;

    // Safety checks
    if config.safety.check_git_repo {
        let guard = SafetyGuard::new(
            config.safety.check_git_repo,
            config.safety.max_depth,
            config.safety.min_free_space_gb,
        );

        if let Err(e) = guard.validate(&path) {
            if !cli.quiet {
                eprintln!("{}", e);
            }
            return Ok(()); // Exit gracefully for safety violations
        }
    }

    // Create pattern matcher
    let matcher = Arc::new(PatternMatcher::new(&config.patterns)?);

    // Create category tracker and compact display for scanning
    let category_tracker = Arc::new(CategoryTracker::new());
    let (items, scan_errors) = if !cli.quiet {
        let display = CompactDisplay::new_for_scanning(Arc::clone(&category_tracker));

        let scanner = Scanner::new(path.clone(), matcher)
            .with_max_depth(config.safety.max_depth)
            .with_symlinks(!config.options.preserve_symlinks)
            .with_category_tracker(Arc::clone(&category_tracker));

        let result = scanner.scan()?;
        display.finish();
        result
    } else {
        let scanner = Scanner::new(path.clone(), matcher)
            .with_max_depth(config.safety.max_depth)
            .with_symlinks(!config.options.preserve_symlinks);
        scanner.scan()?
    };

    // Prune nested items to avoid redundant deletions
    let items = mc::prune_nested_items(items);

    if items.is_empty() {
        if !cli.quiet {
            println!("\nNo files to clean!");
        }
        return Ok(());
    }

    // Calculate total size
    let total_size: u64 = items.iter().map(|i| i.size).sum();

    // Show summary with category breakdown
    if !cli.quiet {
        println!();
        println!("{}", "━".repeat(50).bright_black());
        println!("\n{} {} • {}",
            "Found".dimmed(),
            items.len().to_string().bright_white(),
            format_size(total_size, DECIMAL).bright_green()
        );

        // Show category breakdown
        if category_tracker.total_count() > 0 {
            println!("  {}", category_tracker.format_breakdown());
        }

        println!();
    }

    // Confirmation prompt (unless --yes or dry-run)
    if !cli.yes && !cli.dry_run && config.options.require_confirmation {
        print!("\nProceed with cleaning? [y/N]: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cleaning cancelled");
            return Ok(());
        }
    }

    // Create progress reporter
    let progress = if cli.quiet {
        Arc::new(NoOpProgress) as Arc<dyn mc::Progress>
    } else {
        let display = CompactDisplay::new_for_cleaning(items.len() as u64);
        let worker_count = config.options.parallel_threads;
        display.set_message(&format!("Cleaning ({} workers)", worker_count.to_string().bright_cyan()));
        Arc::new(display) as Arc<dyn mc::Progress>
    };

    let cleaner = ParallelCleaner::new()
        .with_threads(config.options.parallel_threads)
        .with_dry_run(cli.dry_run)
        .with_progress(progress.clone());

    let mut report = cleaner.clean(items)?;
    report.scan_errors = scan_errors;

    progress.finish();

    // Show results
    if cli.stats || config.options.show_statistics || !cli.quiet {
        print_report(&report);
    }

    Ok(())
}

/// Handles the execution of `mc` subcommands.
///
/// # Arguments
///
/// * `command` - The subcommand to execute, as parsed by `clap`.
/// * `cli` - A reference to the parsed `Cli` arguments for context.
///
/// # Panics
///
/// This function does not panic, but it can return errors from file system
/// operations or configuration parsing.
fn handle_command(command: Commands, cli: &Cli) -> Result<()> {
    match command {
        Commands::List { json } => {
            let config = Config::load(cli.config.as_ref())?;
            let path = cli.path.canonicalize()?;

            let matcher = Arc::new(PatternMatcher::new(&config.patterns)?);
            let scanner = Scanner::new(path, matcher);
            let (items, _scan_errors) = scanner.scan()?;

            if json {
                println!("{}", serde_json::to_string_pretty(&items)?);
            } else {
                for item in items {
                    println!("{} ({})", item.path.display(), format_size(item.size, DECIMAL));
                }
            }
        }
        Commands::Init { global } => {
            let config = Config::default();
            let toml = toml::to_string_pretty(&config)?;

            let config_path = if global {
                directories::ProjectDirs::from("com", "mc", "mc")
                    .map(|dirs| dirs.config_dir().join("config.toml"))
                    .ok_or_else(|| mc::McError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "Could not determine config directory")))?
            } else {
                std::env::current_dir()?.join(".mc.toml")
            };

            // Create parent directory if needed
            if let Some(parent) = config_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            std::fs::write(&config_path, toml)?;
            println!("Created configuration file: {}", config_path.display());
        }
        Commands::Config => {
            let config = Config::load(cli.config.as_ref())?;
            println!("{}", toml::to_string_pretty(&config)?);
        }
    }

    Ok(())
}

/// Prints a formatted report of the cleaning operation.
///
/// # Arguments
///
/// * `report` - A reference to the `CleanReport` generated by the cleaner.
///
/// # Output
///
/// The report is printed to stdout with colors and formatting for readability.
/// It distinguishes between a dry run and an actual cleaning operation.
fn print_report(report: &mc::CleanReport) {
    println!();

    if report.dry_run {
        println!("{} {} items",
            "✓".bright_green(),
            report.items_deleted.to_string().bright_white()
        );
        println!("{} {} would be freed",
            "✓".bright_green(),
            format_size(report.bytes_freed, DECIMAL).bright_green()
        );
        println!("\n{}", "Dry run complete!".yellow());
    } else {
        // Calculate throughput
        let duration_secs = report.duration.as_secs_f64();
        let mb_per_sec = if duration_secs > 0.0 {
            (report.bytes_freed as f64 / duration_secs) / 1_000_000.0
        } else {
            0.0
        };

        println!("{} Cleaned {} items",
            "✓".bright_green(),
            report.items_deleted.to_string().bright_white()
        );
        println!("{} Freed {} in {:.1}s",
            "✓".bright_green(),
            format_size(report.bytes_freed, DECIMAL).bright_green(),
            duration_secs
        );
        if mb_per_sec > 0.0 {
            println!("  {:.1} MB/s", mb_per_sec.to_string().bright_cyan());
        }
        println!("\n{}", "Done!".green());
    }

    // Show errors if any (compact format)
    if !report.scan_errors.is_empty() || !report.errors.is_empty() {
        println!();
        let total_errors = report.scan_errors.len() + report.errors.len();
        println!("{} {} errors occurred",
            "⚠".yellow(),
            total_errors.to_string().yellow()
        );
    }
}