#!/bin/bash
# Teardown: clean up all test harness state
# Idempotent — safe to run even if nothing is set up
set -uo pipefail

HARNESS_SESSION_NAME="${HARNESS_SESSION_NAME:-test-harness}"
TEST_SESSIONS=("zelligent-test-repo" "test-harness" "$HARNESS_SESSION_NAME")

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
tmux -L zt-driver-test kill-server 2>/dev/null || true

# Never derive on-disk paths from `zellij --version`: sockets live under
# `<socket_dir>/<version>/<session>` and serialized state under
# `<cache_base>/<version-or-contract>/session_info/<session>`, but the dir
# name doesn't have to match the binary's version (e.g. zellij 0.44.x
# writes `contract_version_1`), and leftovers from a previous version can
# never match it. Glob every version dir instead — same style as
# zelligent.sh's #179 socket sweep.
socket_base="${TMPDIR:-/tmp}"
socket_base="${socket_base%/}"
socket_dir="$socket_base/zellij-$(id -u)"
for session in "${TEST_SESSIONS[@]}"; do
  for socket_path in "$socket_dir"/*/"$session"; do
    [ -e "$socket_path" ] || continue
    kill_matching_pids "zellij --server $socket_path"
  done
  kill_matching_pids "zellij attach $session"
  kill_matching_pids "zellij --session $session"
done

sleep 1

for session in "${TEST_SESSIONS[@]}"; do
  for cache_base in \
    "$HOME/Library/Caches/org.Zellij-Contributors.Zellij" \
    "${XDG_CACHE_HOME:-$HOME/.cache}/zellij"; do
    rm -rf "$cache_base"/*/session_info/"$session" 2>/dev/null || true
  done
  rm -f "$socket_dir"/*/"$session" 2>/dev/null || true
done

rm -rf /tmp/zelligent-test-repo
rm -rf /private/tmp/zelligent-test-repo
rm -rf "$HOME/.zelligent/worktrees/zelligent-test-repo"

echo "teardown complete"
