#!/bin/bash
# Fixture: Empty git repository with no worktrees
# The plugin should show the empty state (logo + hints)
set -euo pipefail

REPO_DIR="/tmp/zelligent-test-repo"

rm -rf "$REPO_DIR"
git clone https://github.com/pcomans/zelligent.git "$REPO_DIR" 2>&1

echo "REPO_DIR=$REPO_DIR"
