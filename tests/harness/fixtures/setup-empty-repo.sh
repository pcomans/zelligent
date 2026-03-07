#!/bin/bash
# Fixture: Empty git repository with no worktrees
# The plugin should show the empty state (logo + hints)
set -euo pipefail

REPO_DIR="/tmp/zelligent-test-repo"
SOURCE_REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"

rm -rf "$REPO_DIR"
git clone "$SOURCE_REPO" "$REPO_DIR" 2>&1

echo "REPO_DIR=$REPO_DIR"
