#!/bin/bash
# PreToolUse hook: gate git push behind a docs freshness check.
# Reads JSON from stdin, checks if the command is git push,
# and requires DOCS_VERIFIED=1 prefix to proceed.

COMMAND=$(jq -r '.tool_input.command' < /dev/stdin)

# Only gate git push commands
case "$COMMAND" in
  *git\ push*|*git\ push*) ;;
  *) exit 0 ;;
esac

# Allow if docs verified token is present
case "$COMMAND" in
  *DOCS_VERIFIED=1*) exit 0 ;;
esac

echo "Push blocked. Confirm that documentation in docs/ and AGENTS.md has been updated or does not need updates, then re-run with DOCS_VERIFIED=1 git push ..." >&2
exit 2
