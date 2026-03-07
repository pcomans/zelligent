#!/bin/bash
# Teardown: clean up all test harness state
# Idempotent — safe to run even if nothing is set up
set -uo pipefail

zellij kill-session test-harness 2>/dev/null || true
zellij web --stop 2>/dev/null || true
zellij web --revoke-all-tokens 2>/dev/null || true

rm -rf /tmp/zelligent-test-repo
rm -rf "$HOME/.zelligent/worktrees/zelligent-test-repo"

echo "teardown complete"
