#!/bin/bash
# Teardown: clean up all test harness state
# Idempotent — safe to run even if nothing is set up
set -uo pipefail

TEST_SESSIONS=("zelligent-test-repo" "test-harness")

kill_matching_pids() {
  local pattern="$1"
  local pids
  pids=$(ps -eo pid=,args= | grep -F "$pattern" | grep -v grep | awk '{print $1}' || true)
  if [ -n "$pids" ]; then
    kill -9 $pids 2>/dev/null || true
  fi
}

for session in "${TEST_SESSIONS[@]}"; do
  zellij delete-session --force "$session" 2>/dev/null || true
  zellij kill-session "$session" 2>/dev/null || true
done

zellij web --stop 2>/dev/null || true
zellij web --revoke-all-tokens 2>/dev/null || true

zellij_version=$(zellij --version 2>/dev/null | awk '{print $2}')
if [ -n "$zellij_version" ]; then
  socket_base="${TMPDIR:-/tmp}"
  socket_base="${socket_base%/}"
  for session in "${TEST_SESSIONS[@]}"; do
    socket_path="$socket_base/zellij-$(id -u)/$zellij_version/$session"
    kill_matching_pids "zellij --server $socket_path"
    kill_matching_pids "zellij attach $session"
    kill_matching_pids "zellij --session $session"
  done

  sleep 1

  for session in "${TEST_SESSIONS[@]}"; do
    for cache_base in \
      "$HOME/Library/Caches/org.Zellij-Contributors.Zellij" \
      "${XDG_CACHE_HOME:-$HOME/.cache}/zellij"; do
      rm -rf "$cache_base/$zellij_version/session_info/$session" 2>/dev/null || true
    done
    rm -f "$socket_base/zellij-$(id -u)/$zellij_version/$session" 2>/dev/null || true
  done
fi

rm -rf /tmp/zelligent-test-repo
rm -rf /private/tmp/zelligent-test-repo
rm -rf "$HOME/.zelligent/worktrees/zelligent-test-repo"

echo "teardown complete"
