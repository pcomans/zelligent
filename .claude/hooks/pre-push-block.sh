#!/bin/bash
# PreToolUse hook: gate git push behind a docs preflight check.
# Reads JSON from stdin, checks if the command is git push,
# and requires PREFLIGHT_VERIFIED=1 prefix to proceed.

COMMAND=$(jq -r '.tool_input.command' < /dev/stdin)

# Only gate git push commands
case "$COMMAND" in
  *git\ push*|*git\ push*) ;;
  *) exit 0 ;;
esac

# Allow if preflight token is present
case "$COMMAND" in
  *PREFLIGHT_VERIFIED=1*) exit 0 ;;
esac

echo "Pre-push blocked. Confirm that documentation in docs/ and AGENTS.md has been updated or does not need updates, then re-run with PREFLIGHT_VERIFIED=1 git push ..." >&2
exit 2
