#!/bin/bash
# Fixture: Git repository with 3 worktrees
# The plugin should show feature-a, feature-b, feature-c in the worktree list
set -euo pipefail

REPO_DIR="/tmp/zelligent-test-repo"

rm -rf "$REPO_DIR"
git clone https://github.com/pcomans/zelligent.git "$REPO_DIR" 2>&1
cd "$REPO_DIR"

# Create worktrees in the standard location (~/.zelligent/worktrees/<repo-name>/)
# The repo is named "zelligent-test-repo" so it won't collide with the real "zelligent" repo.
WORKTREE_BASE="$HOME/.zelligent/worktrees/zelligent-test-repo"
mkdir -p "$WORKTREE_BASE"

git worktree add "$WORKTREE_BASE/feature-a" -b feature-a 2>&1
git worktree add "$WORKTREE_BASE/feature-b" -b feature-b 2>&1
git worktree add "$WORKTREE_BASE/feature-c" -b feature-c 2>&1

echo "REPO_DIR=$REPO_DIR"
echo "WORKTREES=feature-a,feature-b,feature-c"
