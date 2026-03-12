#!/bin/bash
# Fixture: Empty git repository with no worktrees
# When opened via new-tab in an existing session, the plugin shows browse mode
# listing session tabs. The empty-state logo only appears in a fresh zelligent
# session started outside Zellij (not testable via the tmux harness).
set -euo pipefail

REPO_DIR="/tmp/zelligent-test-repo"
SOURCE_REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"

rm -rf "$REPO_DIR"
git clone "$SOURCE_REPO" "$REPO_DIR" 2>&1

echo "REPO_DIR=$REPO_DIR"
