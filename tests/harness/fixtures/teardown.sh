#!/bin/bash
# Teardown: clean up all test harness state
# Idempotent — safe to run even if nothing is set up
set -uo pipefail

zellij kill-session test-harness 2>/dev/null || true
zellij web --stop 2>/dev/null || true
zellij web --revoke-all-tokens 2>/dev/null || true

# Remove worktrees properly before deleting the repo
if [ -d /tmp/zelligent-test-repo ]; then
  git -C /tmp/zelligent-test-repo worktree list --porcelain 2>/dev/null | grep '^worktree ' | sed 's/^worktree //' | while read -r wt; do
    if [ "$wt" = "/tmp/zelligent-test-repo" ] || [ "$wt" = "/private/tmp/zelligent-test-repo" ]; then
      continue
    fi
    git -C /tmp/zelligent-test-repo worktree remove --force "$wt" 2>/dev/null || true
  done
fi
rm -rf /tmp/zelligent-test-repo
rm -rf "$HOME/.zelligent/worktrees/zelligent-test-repo"

echo "teardown complete"
