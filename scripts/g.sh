#!/usr/bin/env bash
# g.sh — commit, push current branch, build release, optional crates.io publish
set -Eeuo pipefail
trap 'echo "✖︎ Error on line $LINENO"; exit 1' ERR

# Usage:
#   ./g.sh [-m "commit msg"] [--release] [--publish]
# Flags:
#   -m, --message   Commit message (default: "chore: update")
#   --release       Bump version, tag, and prepare for release
#   --publish       Publish to crates.io (implies --release)
# Env:
#   GIT_PROFILE=onedusk   # if you use `git s-p` alias

msg="chore: update"
do_release=0
do_publish=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    -m|--message) msg="${2:?}"; shift 2 ;;
    --release)    do_release=1; shift ;;
    --publish)    do_publish=1; do_release=1; shift ;;
    *) echo "Unknown arg: $1"; exit 2 ;;
  esac
done

# Optional: switch git profile if alias exists
: "${GIT_PROFILE:=onedusk}"
if git config --get alias.s-p >/dev/null 2>&1; then
  git s-p "$GIT_PROFILE"
fi

# Ensure we're inside a git repo
git rev-parse --is-inside-work-tree >/dev/null

# Stage & commit only if there are changes
if ! git diff --quiet || ! git diff --cached --quiet; then
  git add -A
  git commit -m "$msg"
fi

# Push current branch
branch="$(git rev-parse --abbrev-ref HEAD)"
git push -u origin "$branch"

# --- Build ---
echo "Building release..."
cargo build --release

# Run tests before any release
if [[ $do_release -eq 1 ]]; then
  echo "Running tests..."
  cargo test --release

  echo "Running clippy..."
  cargo clippy --release -- -D warnings || true
fi

# --- Release workflow ---
if [[ $do_release -eq 1 ]]; then
  # Extract current version from Cargo.toml
  current_version=$(grep -E '^version\s*=' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
  echo "Current version: $current_version"

  # Check if tag already exists
  if git rev-parse "v$current_version" >/dev/null 2>&1; then
    echo "Tag v$current_version already exists. Bump version in Cargo.toml first."
    exit 1
  fi

  # Create git tag
  echo "Creating tag v$current_version..."
  git tag -a "v$current_version" -m "Release v$current_version"
  git push origin "v$current_version"

  echo "✔ Tagged v$current_version"
fi

# --- Publish to crates.io ---
if [[ $do_publish -eq 1 ]]; then
  echo "Publishing to crates.io..."
  cargo publish
  echo "✔ Published to crates.io"
fi

# --- Summary ---
echo ""
echo "✔ Done"
cargo --version
echo "Binary: target/release/mc"
ls -lh target/release/mc 2>/dev/null || true
