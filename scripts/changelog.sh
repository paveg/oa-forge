#!/usr/bin/env bash
# Generate or preview CHANGELOG.md using git-cliff.
#
# Usage:
#   ./scripts/changelog.sh          # preview unreleased changes
#   ./scripts/changelog.sh --write  # write CHANGELOG.md
#   ./scripts/changelog.sh --latest # show only latest tag changes

set -euo pipefail

if ! command -v git-cliff &>/dev/null; then
  echo "git-cliff not found. Install: cargo install git-cliff" >&2
  exit 1
fi

case "${1:-}" in
  --write)
    git-cliff --config cliff.toml --output CHANGELOG.md
    echo "CHANGELOG.md updated."
    ;;
  --latest)
    git-cliff --config cliff.toml --latest --strip header
    ;;
  *)
    git-cliff --config cliff.toml --unreleased --strip header
    ;;
esac
