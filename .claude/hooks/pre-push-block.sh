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
  *DOCS_VERIFIED=1*) ;;
  *)
    echo "Push blocked. Confirm that documentation in docs/ and AGENTS.md has been updated or does not need updates, then re-run with DOCS_VERIFIED=1 git push ..." >&2
    exit 2
    ;;
esac

# Warn if plugin source files were modified (non-blocking)
# Use full push range if upstream exists, otherwise last commit only
if git rev-parse --abbrev-ref --symbolic-full-name @{u} >/dev/null 2>&1; then
  DIFF_RANGE='@{u}..HEAD'
else
  DIFF_RANGE='HEAD~1..HEAD'
fi

if git diff --name-only "$DIFF_RANGE" 2>/dev/null | grep -q '^plugin/src/'; then
  cat >&2 <<'WARN'
Plugin code changed. Before pushing, verify:
  - UI rendering in ui.rs remains a pure function of State (no side effects)
  - State mutations in lib.rs happen in handle_* methods that return Action enums
  - Side effects are confined to execute() and fire_* methods
WARN
fi
