#!/bin/bash
# Fixture: Git repository with 10 worktrees — enough to force viewport scrolling
# in a short pane. Includes a dir!=branch worktree (agent/mouse-test) and a
# long branch name to exercise truncation.
set -euo pipefail

REPO_DIR="/tmp/zelligent-test-repo"
SOURCE_REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"

bash "$(dirname "$0")/teardown.sh" >/dev/null 2>&1 || true
git clone "$SOURCE_REPO" "$REPO_DIR" 2>&1
cd "$REPO_DIR"
mkdir -p "$REPO_DIR/.zelligent"
cp "$SOURCE_REPO/share/default-layout.kdl" "$REPO_DIR/.zelligent/layout.kdl"

WORKTREE_BASE="$HOME/.zelligent/worktrees/zelligent-test-repo"
rm -rf "$WORKTREE_BASE"
mkdir -p "$WORKTREE_BASE"

for i in 01 02 03 04 05 06 07 08; do
  git worktree add "$WORKTREE_BASE/wt-$i" -b "wt-$i" 2>&1
done
# dir != branch: sanitized dir name for a slash branch
git worktree add "$WORKTREE_BASE/agent-mouse-test" -b "agent/mouse-test" 2>&1
# long name for truncation (…) check
git worktree add "$WORKTREE_BASE/feature-very-long-branch-name-for-truncation-check" \
  -b "feature-very-long-branch-name-for-truncation-check" 2>&1

echo "REPO_DIR=$REPO_DIR"
echo "WORKTREES=wt-01..wt-08,agent/mouse-test,feature-very-long-branch-name-for-truncation-check"
