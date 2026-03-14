#!/bin/bash
# Fixture: Git repository with 3 worktrees
# The plugin should show feature-a, feature-b, feature-c in the worktree list
set -euo pipefail

REPO_DIR="/tmp/zelligent-test-repo"
SOURCE_REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"

rm -rf "$REPO_DIR"
git clone "$SOURCE_REPO" "$REPO_DIR" 2>&1
cd "$REPO_DIR"
mkdir -p "$REPO_DIR/.zelligent"
cp "$SOURCE_REPO/share/default-layout.kdl" "$REPO_DIR/.zelligent/layout.kdl"

# Create worktrees in the standard location (~/.zelligent/worktrees/<repo-name>/)
# The repo is named "zelligent-test-repo" so it won't collide with the real "zelligent" repo.
WORKTREE_BASE="$HOME/.zelligent/worktrees/zelligent-test-repo"
rm -rf "$WORKTREE_BASE"
mkdir -p "$WORKTREE_BASE"

git worktree add "$WORKTREE_BASE/feature-a" -b feature-a 2>&1
git worktree add "$WORKTREE_BASE/feature-b" -b feature-b 2>&1
git worktree add "$WORKTREE_BASE/feature-c" -b feature-c 2>&1

echo "REPO_DIR=$REPO_DIR"
echo "WORKTREES=feature-a,feature-b,feature-c"
