use assert_cmd::Command;
use assert_fs::prelude::*;
use assert_fs::TempDir;
use predicates::prelude::PredicateBooleanExt;

fn mc_cmd() -> Command {
    Command::cargo_bin("mc").unwrap()
}

#[test]
fn test_dry_run_exit_code_zero() {
    let temp = TempDir::new().unwrap();
    temp.child("node_modules/pkg/index.js")
        .create_dir_all()
        .unwrap();

    mc_cmd()
        .arg("--dry-run")
        .arg("--no-git-check")
        .arg(temp.path())
        .assert()
        .success();
}

#[test]
fn test_help_flag() {
    mc_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("build directory cleaner"));
}

#[test]
fn test_version_flag() {
    mc_cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicates::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_verbose_produces_log_output() {
    let temp = TempDir::new().unwrap();

    mc_cmd()
        .env_remove("RUST_LOG") // Prevent env override of --verbose level
        .arg("--verbose")
        .arg("--dry-run")
        .arg("--no-git-check")
        .arg(temp.path())
        .assert()
        .success()
        .stderr(predicates::str::is_empty().not());
}

#[test]
fn test_quiet_suppresses_output() {
    let temp = TempDir::new().unwrap();

    mc_cmd()
        .arg("--quiet")
        .arg("--dry-run")
        .arg("--no-git-check")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicates::str::is_empty());
}

#[test]
fn test_nonexistent_path_returns_error() {
    mc_cmd()
        .arg("/nonexistent/path/abc123xyz")
        .assert()
        .failure();
}
