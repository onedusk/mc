#!/usr/bin/env bash
# release.sh — commit, push current branch, build gem, optional gem push
set -Eeuo pipefail
trap 'echo "✖︎ Error on line $LINENO"; exit 1' ERR

# Usage:
#   ./release.sh [-m "commit msg"] [--release]
# Env:
#   GIT_PROFILE=onedusk   # if you use `git s-p` alias, we’ll call it if configured
#   PUSH_GEM=1            # push built gem to RubyGems (when not using --release)

msg="chore: update"
do_release=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    -m|--message) msg="${2:?}"; shift 2 ;;
    --release)    do_release=1; shift ;;
    *) echo "Unknown arg: $1"; exit 2 ;;
  esac
done

# Optional: switch git profile if alias exists and var provided (defaults to your original)
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

# Push current branch instead of hardcoding 'main'
branch="$(git rev-parse --abbrev-ref HEAD)"
git push -u origin "$branch"

# --- Build / Release the gem ---
have_rake_tasks() {
  [[ -f Rakefile ]] && command -v bundle >/dev/null 2>&1 && bundle exec rake -T >/dev/null
}

if [[ $do_release -eq 1 ]] && have_rake_tasks && bundle exec rake -T | grep -qE '\brake release\b'; then
  # Best path: Bundler's release tags, pushes, builds, and gem pushes in one go
  bundle exec rake release
  exit 0
fi

# Otherwise: build (prefer bundler's build if present)
mkdir -p pkg
if have_rake_tasks && bundle exec rake -T | grep -qE '\brake build\b'; then
  bundle exec rake build
else
  gemspec="$(ls -1 *.gemspec 2>/dev/null | head -n1 || true)"
  [[ -n "$gemspec" ]] || { echo "No .gemspec found."; exit 1; }
  gem build "$gemspec"
  mv -f ./*.gem pkg/ 2>/dev/null || true
fi

# Optional push to RubyGems if requested
if [[ "${PUSH_GEM:-0}" == "1" ]]; then
  gem_file="$(ls -1t pkg/*.gem 2>/dev/null | head -n1 || true)"
  [[ -n "$gem_file" ]] || { echo "No built gem found in pkg/."; exit 1; }
  gem push "$gem_file"
fi
