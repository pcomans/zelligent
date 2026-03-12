#!/bin/bash
# Teardown: clean up all test harness state
# Idempotent — safe to run even if nothing is set up
set -uo pipefail

tmux -L zt-driver-test kill-server 2>/dev/null || true

rm -rf /tmp/zelligent-test-repo
rm -rf "$HOME/.zelligent/worktrees/zelligent-test-repo"

echo "teardown complete"
