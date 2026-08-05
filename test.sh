#!/bin/bash

# Test suite for zelligent.sh
# Unit tests run anywhere. Integration tests require Zellij to be installed.

# Fork-bomb guard: the prompt-delivery harness spawns zelligent with a mock
# claude. If that mock is ever bypassed (e.g. a login shell resets PATH), the
# real `claude -p "run all tests..."` can re-enter this script recursively. This
# guard makes that recursion structurally impossible regardless of mock health.
if [ -n "$ZELLIGENT_TEST_ACTIVE" ]; then
  echo "test.sh: refusing recursive invocation (ZELLIGENT_TEST_ACTIVE set)" >&2
  exit 1
fi
export ZELLIGENT_TEST_ACTIVE=1

PASS=0
FAIL=0
CLEANUP_WORKTREE_PATHS=()
CLEANUP_WORKTREE_BRANCHES=()
SCRIPT="$(cd "$(dirname "$0")" && pwd)/zelligent.sh"
# The checkout this test.sh actually lives in. In a `git worktree` checkout,
# --git-common-dir resolves to the *main* repo's .git (REPO_ROOT below), not
# this worktree — so any check that reads this branch's own file contents
# must use SCRIPT_DIR, not REPO_ROOT.
SCRIPT_DIR="$(dirname "$SCRIPT")"
GIT_COMMON_DIR="$(git -C "$(dirname "$0")" rev-parse --path-format=absolute --git-common-dir)"
REPO_ROOT="${GIT_COMMON_DIR%/.git}"
REPO_NAME="$(basename "$REPO_ROOT")"
# Spawn now requires a resolvable plugin path and layout asset.
export ZELLIGENT_PLUGIN_SRC="${ZELLIGENT_PLUGIN_SRC:-$SCRIPT}"
export ZELLIGENT_DEFAULT_LAYOUT_SRC="${ZELLIGENT_DEFAULT_LAYOUT_SRC:-$REPO_ROOT/share/default-layout.kdl}"
# The test harness drives zelligent without a controlling terminal (commands
# are captured via `$(...)`), so the production-only TTY guard at the spawn
# entrypoint must be skipped — the mock zellij stubs never read the terminal.
export ZELLIGENT_SKIP_TTY_CHECK=1

TEST_REPO_LAYOUT="$REPO_ROOT/.zelligent/layout.kdl"
TEST_REPO_LAYOUT_BAK="$REPO_ROOT/.zelligent/layout.kdl.test-bak"
if [ -f "$TEST_REPO_LAYOUT" ]; then
  cp "$TEST_REPO_LAYOUT" "$TEST_REPO_LAYOUT_BAK"
fi
cp "$ZELLIGENT_DEFAULT_LAYOUT_SRC" "$TEST_REPO_LAYOUT"

pass() { echo "  ✅ $1"; ((PASS++)); }
fail() { echo "  ❌ $1"; ((FAIL++)); }

restore_test_layout() {
  if [ -f "$TEST_REPO_LAYOUT_BAK" ]; then
    mv "$TEST_REPO_LAYOUT_BAK" "$TEST_REPO_LAYOUT"
  else
    rm -f "$TEST_REPO_LAYOUT"
  fi
}

restore_setup() { :; }

register_cleanup_worktree() {
  local worktree_path="$1" branch="$2"
  CLEANUP_WORKTREE_PATHS+=("$worktree_path")
  CLEANUP_WORKTREE_BRANCHES+=("$branch")
}

register_managed_cleanup() {
  local home_dir="$1" branch="$2"
  register_cleanup_worktree "$home_dir/.zelligent/worktrees/$REPO_NAME/$branch" "$branch"
}

cleanup_registered_worktrees() {
  local i
  for i in "${!CLEANUP_WORKTREE_PATHS[@]}"; do
    git -C "$REPO_ROOT" worktree remove --force "${CLEANUP_WORKTREE_PATHS[$i]}" &>/dev/null || true
    git -C "$REPO_ROOT" branch -D "${CLEANUP_WORKTREE_BRANCHES[$i]}" &>/dev/null || true
  done
}

cleanup_test_artifacts() {
  cleanup_registered_worktrees
  restore_setup
  restore_test_layout
}

trap cleanup_test_artifacts EXIT INT TERM

check() {
  local desc="$1" expected="$2" actual="$3"
  if [ "$actual" = "$expected" ]; then
    pass "$desc"
  else
    fail "$desc (expected: '$expected', got: '$actual')"
  fi
}

contains() {
  local desc="$1" needle="$2" haystack="$3"
  if echo "$haystack" | grep -qF -- "$needle"; then
    pass "$desc"
  else
    fail "$desc (expected to contain: '$needle')"
  fi
}

not_contains() {
  local desc="$1" needle="$2" haystack="$3"
  if echo "$haystack" | grep -qF -- "$needle"; then
    fail "$desc (expected NOT to contain: '$needle')"
  else
    pass "$desc"
  fi
}

excludes() { not_contains "$@"; }

count_equals() {
  local desc="$1" needle="$2" expected="$3" haystack="$4"
  local actual
  actual=$(echo "$haystack" | grep -oF -- "$needle" | wc -l | tr -d ' ')
  if [ "$actual" -eq "$expected" ]; then
    pass "$desc"
  else
    fail "$desc (expected $expected occurrences of '$needle', got $actual)"
  fi
}

cleanup_test_branch_for_home() {
  local home_dir="$1" branch="$2"
  git -C "$REPO_ROOT" worktree remove --force \
    "$home_dir/.zelligent/worktrees/$REPO_NAME/$branch" &>/dev/null || true
  git -C "$REPO_ROOT" branch -D "$branch" &>/dev/null || true
}

# ── Session name generation ────────────────────────────────────────────────────
echo "Session name generation:"

check "simple branch" \
  "mybranch" \
  "$(bash -c 'BRANCH_NAME=mybranch; echo "${BRANCH_NAME//\//-}"')"

check "feature/ prefix becomes dash" \
  "feature-fiddlesticks" \
  "$(bash -c 'BRANCH_NAME=feature/fiddlesticks; echo "${BRANCH_NAME//\//-}"')"

check "nested slashes all replaced" \
  "a-b-c" \
  "$(bash -c 'BRANCH_NAME=a/b/c; echo "${BRANCH_NAME//\//-}"')"

# ── Layout file generation (via the script with mock zellij) ──────────────────
echo "Layout file generation:"

MOCK_BIN_LAYOUT=$(mktemp -d)
cat > "$MOCK_BIN_LAYOUT/zellij" <<'MOCK'
#!/bin/bash
echo "zellij $*"
for arg in "$@"; do
  if [ -f "$arg" ]; then cat "$arg"; fi
done
MOCK
cat > "$MOCK_BIN_LAYOUT/lazygit" <<'MOCK'
#!/bin/bash
MOCK
chmod +x "$MOCK_BIN_LAYOUT/zellij" "$MOCK_BIN_LAYOUT/lazygit"

# Run the script inside-Zellij mode so it calls new-tab and emits the layout
register_managed_cleanup "$HOME" test-layout-branch
out=$(ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN_LAYOUT:$PATH" \
  "$SCRIPT" spawn test-layout-branch claude 2>&1)
# Cleanup worktree/branch created by the script
git -C "$REPO_ROOT" worktree remove --force \
  "$HOME/.zelligent/worktrees/$REPO_NAME/test-layout-branch" &>/dev/null || true
git -C "$REPO_ROOT" branch -D test-layout-branch &>/dev/null || true

EXPECTED_CWD="$HOME/.zelligent/worktrees/$REPO_NAME/test-layout-branch"
contains "layout contains agent command"  'exec claude'              "$out"
contains "layout contains worktree cwd"   "cwd=\"$EXPECTED_CWD\""   "$out"
contains "layout contains lazygit"        'command="lazygit"'        "$out"
contains "layout contains sidebar plugin" 'plugin location="file:'    "$out"
count_equals "L1: exactly one vertical split per tab (sidebar left)" 'split_direction="Vertical"' 1 "$out"
excludes "layout omits tab-bar"           'zellij:tab-bar'            "$out"
contains "layout contains status-bar"     'zellij:status-bar'         "$out"
excludes "inside zellij layout: no tab{} wrapper" 'tab name='        "$out"
contains "new worktree: setup.sh runs as preamble" 'setup.sh'        "$out"
contains "new worktree: agent starts via exec"     'exec claude'     "$out"
excludes "new worktree: no invalid KDL \\$ escape" '"\$'             "$out"
contains "new worktree: setup failure prompt keeps literal \$?" "Setup failed (exit '\$?'). Press Enter to close." "$out"

# Test: existing worktree should NOT include setup.sh preamble
# Re-create the worktree so it already exists, then run the script again
register_managed_cleanup "$HOME" test-layout-branch
git -C "$REPO_ROOT" worktree add -b test-layout-branch \
  "$HOME/.zelligent/worktrees/$REPO_NAME/test-layout-branch" HEAD &>/dev/null
out_existing=$(ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN_LAYOUT:$PATH" \
  "$SCRIPT" spawn test-layout-branch claude 2>&1)
git -C "$REPO_ROOT" worktree remove --force \
  "$HOME/.zelligent/worktrees/$REPO_NAME/test-layout-branch" &>/dev/null || true
git -C "$REPO_ROOT" branch -D test-layout-branch &>/dev/null || true

contains "existing worktree: uses direct command" 'exec claude' "$out_existing"
excludes "existing worktree: no setup preamble"   'setup.sh'         "$out_existing"

# Test (#174): branch checked out in a worktree whose DIRECTORY name differs
# from the branch (renamed branch / out-of-band worktree). Spawn must resolve
# the existing worktree by branch and reuse it — the old canonical-path check
# missed it and `git worktree add` fataled with "already used by worktree at".
register_cleanup_worktree "$HOME/.zelligent/worktrees/$REPO_NAME/original-dir-name" renamed-dir-branch
git -C "$REPO_ROOT" worktree add -b renamed-dir-branch \
  "$HOME/.zelligent/worktrees/$REPO_NAME/original-dir-name" HEAD &>/dev/null
out_renamed=$(ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN_LAYOUT:$PATH" \
  "$SCRIPT" spawn renamed-dir-branch claude 2>&1); code_renamed=$?

check "renamed-branch worktree: spawn exits 0" "0" "$code_renamed"
contains "renamed-branch worktree: reuses existing worktree" "Worktree already exists" "$out_renamed"
contains "renamed-branch worktree: cwd is the real worktree dir" \
  "cwd=\"$HOME/.zelligent/worktrees/$REPO_NAME/original-dir-name\"" "$out_renamed"
not_contains "renamed-branch worktree: no git fatal" "already used by worktree" "$out_renamed"
check "renamed-branch worktree: no duplicate dir created" "false" \
  "$([ -d "$HOME/.zelligent/worktrees/$REPO_NAME/renamed-dir-branch" ] && echo true || echo false)"
excludes "renamed-branch worktree: no setup preamble (existing worktree)" 'setup.sh' "$out_renamed"

git -C "$REPO_ROOT" worktree remove --force \
  "$HOME/.zelligent/worktrees/$REPO_NAME/original-dir-name" &>/dev/null || true
git -C "$REPO_ROOT" branch -D renamed-dir-branch &>/dev/null || true

# Test (#174 follow-up): the branch checked out in the MAIN repository must
# never be "reused" as a worktree — `git worktree list` includes the main
# checkout, and opening an agent tab there breaks the isolation model.
#
# Built on a throwaway repo rather than this checkout. `actions/checkout` on a
# pull_request lands on a DETACHED HEAD, where `git branch --show-current` is
# empty: the old version then ran `spawn ""`, which exits 1 down the
# argument-validation path, so only the message assertion noticed that the
# guard was no longer being exercised at all. A fixture repo with a known
# branch checked out reproduces the scenario identically everywhere.
MAIN_CO_REPO=$(mktemp -d)/repo
MAIN_CO_HOME=$(mktemp -d)
mkdir -p "$MAIN_CO_REPO" "$MAIN_CO_HOME/.zelligent"
cp "$REPO_ROOT/share/default-layout.kdl" "$MAIN_CO_HOME/.zelligent/layout.kdl"
git -C "$MAIN_CO_REPO" init -q
# symbolic-ref rather than `init -b`: portable to git < 2.28
git -C "$MAIN_CO_REPO" symbolic-ref HEAD refs/heads/main-co-branch
echo "fixture" > "$MAIN_CO_REPO/file.txt"
git -C "$MAIN_CO_REPO" add -A
git -C "$MAIN_CO_REPO" -c user.email=t@example.com -c user.name=test commit -qm init
out_mainwt=$(cd "$MAIN_CO_REPO" && HOME="$MAIN_CO_HOME" ZELLIJ=1 ZELLIJ_SESSION_NAME=fake \
  PATH="$MOCK_BIN_LAYOUT:$PATH" "$SCRIPT" spawn main-co-branch claude 2>&1); code_mainwt=$?
check "main-checkout branch: spawn exits non-zero" "1" "$code_mainwt"
contains "main-checkout branch: names the main repository" \
  "checked out in the main repository" "$out_mainwt"
excludes "main-checkout branch: never reuses the main checkout" \
  "Worktree already exists" "$out_mainwt"
excludes "main-checkout branch: no tab opened" 'cwd="'"$MAIN_CO_REPO"'"' "$out_mainwt"
rm -rf "$(dirname "$MAIN_CO_REPO")" "$MAIN_CO_HOME"

# Test (#174 follow-up): a stale registration (worktree dir deleted without
# `git worktree remove`) must produce an actionable error, not git's
# confusing "already used by worktree" fatal from `git worktree add`.
register_cleanup_worktree "$HOME/.zelligent/worktrees/$REPO_NAME/stale-reg-dir" stale-reg-branch
git -C "$REPO_ROOT" worktree add -b stale-reg-branch \
  "$HOME/.zelligent/worktrees/$REPO_NAME/stale-reg-dir" HEAD &>/dev/null
rm -rf "$HOME/.zelligent/worktrees/$REPO_NAME/stale-reg-dir"
out_stale=$(ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN_LAYOUT:$PATH" \
  "$SCRIPT" spawn stale-reg-branch claude 2>&1); code_stale=$?
check "stale registration: spawn exits non-zero" "1" "$code_stale"
contains "stale registration: explains the missing directory" \
  "that directory is missing" "$out_stale"
contains "stale registration: suggests git worktree prune" \
  "git worktree prune" "$out_stale"
not_contains "stale registration: no raw git fatal" "already used by worktree" "$out_stale"
git -C "$REPO_ROOT" worktree prune &>/dev/null || true
git -C "$REPO_ROOT" branch -D stale-reg-branch &>/dev/null || true

# Test: new worktree WITHOUT setup.sh should use direct command
SETUP_SH="$REPO_ROOT/.zelligent/setup.sh"
SETUP_SH_BAK="$SETUP_SH.bak"
restore_setup() {
  if [ -e "$SETUP_SH_BAK" ]; then
    mv "$SETUP_SH_BAK" "$SETUP_SH"
  fi
}
mv "$SETUP_SH" "$SETUP_SH_BAK"
register_managed_cleanup "$HOME" test-no-setup-branch
out_no_setup=$(ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN_LAYOUT:$PATH" \
  "$SCRIPT" spawn test-no-setup-branch claude 2>&1)
restore_setup
git -C "$REPO_ROOT" worktree remove --force \
  "$HOME/.zelligent/worktrees/$REPO_NAME/test-no-setup-branch" &>/dev/null || true
git -C "$REPO_ROOT" branch -D test-no-setup-branch &>/dev/null || true

contains "no setup.sh: uses direct command"  'exec claude' "$out_no_setup"
excludes "no setup.sh: no setup preamble"    '.zelligent/setup.sh'    "$out_no_setup"

# Test: multi-word AGENT_CMD with arguments
register_managed_cleanup "$HOME" test-multi-cmd-branch
out_multi=$(ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN_LAYOUT:$PATH" \
  "$SCRIPT" spawn test-multi-cmd-branch 'claude "pls fix the bug" --model claude-sonnet-4-6' 2>&1)
git -C "$REPO_ROOT" worktree remove --force \
  "$HOME/.zelligent/worktrees/$REPO_NAME/test-multi-cmd-branch" &>/dev/null || true
git -C "$REPO_ROOT" branch -D test-multi-cmd-branch &>/dev/null || true

contains "multi-word cmd: contains full command" 'claude \"pls fix the bug\" --model claude-sonnet-4-6' "$out_multi"
contains "multi-word cmd: contains exec"         'exec claude'   "$out_multi"
contains "multi-word cmd: contains model flag"   'claude-sonnet-4-6' "$out_multi"

rm -rf "$MOCK_BIN_LAYOUT"

# ── Layout source resolution and validation ──────────────────────────────────
echo "Layout source resolution:"

MOCK_BIN_LAYOUT_SOURCE=$(mktemp -d)
cat > "$MOCK_BIN_LAYOUT_SOURCE/zellij" <<'MOCK'
#!/bin/bash
echo "zellij $*"
for arg in "$@"; do
  if [ -f "$arg" ]; then cat "$arg"; fi
done
MOCK
cat > "$MOCK_BIN_LAYOUT_SOURCE/lazygit" <<'MOCK'
#!/bin/bash
MOCK
chmod +x "$MOCK_BIN_LAYOUT_SOURCE/zellij" "$MOCK_BIN_LAYOUT_SOURCE/lazygit"

LAYOUT_TEST_HOME=$(mktemp -d)
mkdir -p "$LAYOUT_TEST_HOME/.zelligent"
cat > "$LAYOUT_TEST_HOME/.zelligent/layout.kdl" <<'KDL'
// user-layout-marker
pane split_direction="Vertical" {
    pane size="31%" {
        {{zelligent_sidebar}}
    }
    {{zelligent_children}}
}
pane size=1 borderless=true {
    plugin location="zellij:status-bar"
}
KDL

cat > "$TEST_REPO_LAYOUT" <<'KDL'
// repo-layout-marker
pane split_direction="Vertical" {
    pane size="30%" {
        {{zelligent_sidebar}}
    }
    {{zelligent_children}}
}
pane size=1 borderless=true {
    plugin location="zellij:status-bar"
}
KDL

register_managed_cleanup "$LAYOUT_TEST_HOME" test-layout-precedence-repo
out=$(HOME="$LAYOUT_TEST_HOME" ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN_LAYOUT_SOURCE:$PATH" \
  "$SCRIPT" spawn test-layout-precedence-repo claude 2>&1)
cleanup_test_branch_for_home "$LAYOUT_TEST_HOME" test-layout-precedence-repo
contains "layout precedence: repo layout wins" "repo-layout-marker" "$out"
contains "layout precedence: repo fragment controls sidebar width" 'size="30%"' "$out"
not_contains "layout precedence: repo layout hides user layout" "user-layout-marker" "$out"

rm -f "$TEST_REPO_LAYOUT"
register_managed_cleanup "$LAYOUT_TEST_HOME" test-layout-precedence-user
out=$(HOME="$LAYOUT_TEST_HOME" ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN_LAYOUT_SOURCE:$PATH" \
  "$SCRIPT" spawn test-layout-precedence-user claude 2>&1)
cleanup_test_branch_for_home "$LAYOUT_TEST_HOME" test-layout-precedence-user
contains "layout precedence: user layout used when repo layout missing" "user-layout-marker" "$out"
contains "layout precedence: user fragment controls sidebar width" 'size="31%"' "$out"

rm -f "$LAYOUT_TEST_HOME/.zelligent/layout.kdl"
register_managed_cleanup "$LAYOUT_TEST_HOME" test-layout-missing
out=$(HOME="$LAYOUT_TEST_HOME" ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN_LAYOUT_SOURCE:$PATH" \
  "$SCRIPT" spawn test-layout-missing claude 2>&1); code=$?
check "layout precedence: missing layout exits non-zero" "1" "$code"
contains "layout precedence: missing layout prints error" "no layout found" "$out"
cleanup_test_branch_for_home "$LAYOUT_TEST_HOME" test-layout-missing

cat > "$TEST_REPO_LAYOUT" <<'KDL'
// repo-layout-leading-comment
pane {
    {{zelligent_children}}
}
// repo-layout-trailing-comment
KDL
register_managed_cleanup "$LAYOUT_TEST_HOME" test-layout-invalid-missing
out=$(HOME="$LAYOUT_TEST_HOME" ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN_LAYOUT_SOURCE:$PATH" \
  "$SCRIPT" spawn test-layout-invalid-missing claude 2>&1); code=$?
check "layout validation: missing sidebar placeholder exits non-zero" "1" "$code"
contains "layout validation: missing sidebar placeholder prints error" "must contain {{zelligent_sidebar}} and {{zelligent_children}} exactly once" "$out"
cleanup_test_branch_for_home "$LAYOUT_TEST_HOME" test-layout-invalid-missing

cat > "$TEST_REPO_LAYOUT" <<'KDL'
pane {
    // {{zelligent_sidebar}}
    {{zelligent_children}}
}
KDL
register_managed_cleanup "$LAYOUT_TEST_HOME" test-layout-invalid-comment-only
out=$(HOME="$LAYOUT_TEST_HOME" ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN_LAYOUT_SOURCE:$PATH" \
  "$SCRIPT" spawn test-layout-invalid-comment-only claude 2>&1); code=$?
check "layout validation: commented sidebar placeholder exits non-zero" "1" "$code"
contains "layout validation: commented sidebar placeholder prints error" "must contain {{zelligent_sidebar}} and {{zelligent_children}} exactly once" "$out"
cleanup_test_branch_for_home "$LAYOUT_TEST_HOME" test-layout-invalid-comment-only

cat > "$TEST_REPO_LAYOUT" <<'KDL'
pane split_direction="Vertical" {
    pane size="24%" {
        {{zelligent_sidebar}}
        {{zelligent_sidebar}}
    }
    {{zelligent_children}}
}
KDL
register_managed_cleanup "$LAYOUT_TEST_HOME" test-layout-invalid-duplicate
out=$(HOME="$LAYOUT_TEST_HOME" ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN_LAYOUT_SOURCE:$PATH" \
  "$SCRIPT" spawn test-layout-invalid-duplicate claude 2>&1); code=$?
check "layout validation: duplicate sidebar placeholder exits non-zero" "1" "$code"
contains "layout validation: duplicate sidebar placeholder prints error" "must contain {{zelligent_sidebar}} and {{zelligent_children}} exactly once" "$out"
cleanup_test_branch_for_home "$LAYOUT_TEST_HOME" test-layout-invalid-duplicate

cat > "$TEST_REPO_LAYOUT" <<'KDL'
pane split_direction="Vertical" {
    pane size="24%" {
        {{zelligent_sidebar}}
    }
}
KDL
register_managed_cleanup "$LAYOUT_TEST_HOME" test-layout-invalid-missing-children
out=$(HOME="$LAYOUT_TEST_HOME" ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN_LAYOUT_SOURCE:$PATH" \
  "$SCRIPT" spawn test-layout-invalid-missing-children claude 2>&1); code=$?
check "layout validation: missing children placeholder exits non-zero" "1" "$code"
contains "layout validation: missing children placeholder prints error" "must contain {{zelligent_sidebar}} and {{zelligent_children}} exactly once" "$out"
cleanup_test_branch_for_home "$LAYOUT_TEST_HOME" test-layout-invalid-missing-children

cp "$ZELLIGENT_DEFAULT_LAYOUT_SRC" "$TEST_REPO_LAYOUT"
rm -rf "$LAYOUT_TEST_HOME" "$MOCK_BIN_LAYOUT_SOURCE"

# ── Quoted agent command ─────────────────────────────────────────────────────
echo "Quoted agent command:"

MOCK_BIN_QUOTE=$(mktemp -d)
cat > "$MOCK_BIN_QUOTE/zellij" <<'MOCK'
#!/bin/bash
echo "zellij $*"
for arg in "$@"; do
  if [ -f "$arg" ]; then cat "$arg"; fi
done
MOCK
cat > "$MOCK_BIN_QUOTE/lazygit" <<'MOCK'
#!/bin/bash
MOCK
chmod +x "$MOCK_BIN_QUOTE/zellij" "$MOCK_BIN_QUOTE/lazygit"

register_managed_cleanup "$HOME" test-quoted-branch
out=$(ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN_QUOTE:$PATH" \
  "$SCRIPT" spawn test-quoted-branch 'claude -p "Sag Hallo auf Deutsch"' 2>&1)
git -C "$REPO_ROOT" worktree remove --force \
  "$HOME/.zelligent/worktrees/$REPO_NAME/test-quoted-branch" &>/dev/null || true
git -C "$REPO_ROOT" branch -D test-quoted-branch &>/dev/null || true

contains "quoted cmd: quotes are escaped" 'exec claude -p \"Sag Hallo auf Deutsch\"' "$out"

rm -rf "$MOCK_BIN_QUOTE"

# ── Prompt delivery harness ──────────────────────────────────────────────────
# Verify that the prompt actually reaches the claude binary after the full
# KDL → bash -c execution path, including setup.sh running first.
echo "Prompt delivery harness:"

if ! command -v python3 &>/dev/null; then
  echo "  ⚠️  python3 not found, skipping prompt delivery tests"
else

MOCK_BIN_PROMPT=$(mktemp -d)
PROMPT_LOG=$(mktemp)
KDL_PARSER=$(mktemp)

# Regex-based extractor for double-quoted strings from the KDL `args` node.
# Not a full KDL parser — only handles the \" escape sequences that zelligent
# actually emits. Sufficient for verifying prompt delivery in tests.
cat > "$KDL_PARSER" <<'PYEOF'
#!/usr/bin/env python3
"""Extract args from a zelligent layout file and run them via bash.

Finds the first `args` line, extracts double-quoted string arguments using
regex, unescapes KDL \" sequences, and runs: bash <arg1> <arg2> ...
Rewrites 'exec claude' to the absolute mock-claude path ($MOCK_CLAUDE) so the
mock binary is invoked even though the command runs through `bash -lc` — a login
shell that re-sources the user's profile and resets PATH, which would otherwise
resolve `claude` to the REAL binary and fork-bomb the suite.
"""
import os, re, subprocess, sys

mock_claude = os.environ.get("MOCK_CLAUDE", "claude")
layout_file = sys.argv[1]
with open(layout_file) as f:
    for line in f:
        stripped = line.strip()
        if stripped.startswith("args "):
            tokens = re.findall(r'"((?:[^"\\]|\\.)*)"', stripped)
            args = [t.replace('\\"', '"').replace('\\\\', '\\') for t in tokens]
            args = [a.replace('exec claude', mock_claude) for a in args]
            result = subprocess.run(["bash"] + args)
            sys.exit(result.returncode)

print("ERROR: no args line found in layout", file=sys.stderr)
sys.exit(1)
PYEOF
chmod +x "$KDL_PARSER"

# Mock claude that logs its arguments
cat > "$MOCK_BIN_PROMPT/claude" <<MOCK
#!/bin/bash
printf '%s\n' "\$@" > "$PROMPT_LOG"
MOCK
# Mock zellij that finds the layout file and runs it through the KDL parser
cat > "$MOCK_BIN_PROMPT/zellij" <<MOCK
#!/bin/bash
for arg in "\$@"; do
  if [ -f "\$arg" ]; then
    python3 "$KDL_PARSER" "\$arg"
    exit \$?
  fi
done
MOCK
cat > "$MOCK_BIN_PROMPT/lazygit" <<'MOCK'
#!/bin/bash
MOCK
chmod +x "$MOCK_BIN_PROMPT/claude" "$MOCK_BIN_PROMPT/zellij" "$MOCK_BIN_PROMPT/lazygit"

# Absolute path to the mock claude. The KDL parser rewrites `exec claude` to this
# so a login shell (`bash -lc`) that resets PATH can't reach the real binary.
export MOCK_CLAUDE="$MOCK_BIN_PROMPT/claude"

prompt_test_cleanup() {
  local branch="$1"
  git -C "$REPO_ROOT" worktree remove --force \
    "$HOME/.zelligent/worktrees/$REPO_NAME/$branch" &>/dev/null || true
  git -C "$REPO_ROOT" branch -D "$branch" &>/dev/null || true
}

# Move setup.sh aside so tests don't invoke the repo's real one (e.g. sleep)
mv "$SETUP_SH" "$SETUP_SH_BAK" 2>/dev/null || true

# Test 1: positional prompt (interactive mode with initial prompt)
register_managed_cleanup "$HOME" test-prompt-branch
out_prompt=$(ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN_PROMPT:$PATH" \
  "$SCRIPT" spawn test-prompt-branch 'claude "fix the login bug"' 2>&1)
PROMPT_ARGS=$(cat "$PROMPT_LOG")
contains "prompt delivery: positional prompt reaches claude" "fix the login bug" "$PROMPT_ARGS"
prompt_test_cleanup test-prompt-branch

# Test 2: -p flag (non-interactive)
> "$PROMPT_LOG"
register_managed_cleanup "$HOME" test-pflag-branch
out_pflag=$(ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN_PROMPT:$PATH" \
  "$SCRIPT" spawn test-pflag-branch 'claude -p "run all tests and fix failures"' 2>&1)
PROMPT_ARGS=$(cat "$PROMPT_LOG")
contains "prompt delivery: -p flag reaches claude" "-p" "$PROMPT_ARGS"
contains "prompt delivery: -p prompt text reaches claude" "run all tests and fix failures" "$PROMPT_ARGS"
prompt_test_cleanup test-pflag-branch

# Test 3: prompt with --model flag
> "$PROMPT_LOG"
register_managed_cleanup "$HOME" test-model-prompt-branch
out_model=$(ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN_PROMPT:$PATH" \
  "$SCRIPT" spawn test-model-prompt-branch 'claude --model claude-sonnet-4-6 "refactor the auth module"' 2>&1)
PROMPT_ARGS=$(cat "$PROMPT_LOG")
contains "prompt delivery: model flag reaches claude" "--model" "$PROMPT_ARGS"
contains "prompt delivery: model value reaches claude" "claude-sonnet-4-6" "$PROMPT_ARGS"
contains "prompt delivery: prompt with model flag reaches claude" "refactor the auth module" "$PROMPT_ARGS"
prompt_test_cleanup test-model-prompt-branch

# Test 4: bare claude (no prompt) — should still work, no args logged
> "$PROMPT_LOG"
register_managed_cleanup "$HOME" test-bare-branch
out_bare=$(ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN_PROMPT:$PATH" \
  "$SCRIPT" spawn test-bare-branch claude 2>&1)
BARE_ARGS=$(cat "$PROMPT_LOG")
check "prompt delivery: bare claude has no args" "" "$BARE_ARGS"
prompt_test_cleanup test-bare-branch

rm -rf "$MOCK_BIN_PROMPT" "$PROMPT_LOG" "$KDL_PARSER"
restore_setup

fi # python3 check

# ── --version and --help ──────────────────────────────────────────────────────
echo "Version and help:"

out=$("$SCRIPT" --version 2>&1); code=$?
check "--version exits 0" "0" "$code"
contains "--version prints zelligent" "zelligent" "$out"

out=$("$SCRIPT" --help 2>&1); code=$?
check "--help exits 0" "0" "$code"
contains "--help prints usage" "Usage:" "$out"
contains "--help lists doctor" "doctor" "$out"
contains "--help lists spawn" "spawn" "$out"

out=$("$SCRIPT" help 2>&1); code=$?
check "help exits 0" "0" "$code"
contains "help prints usage" "Usage:" "$out"

# ── No-args behavior ─────────────────────────────────────────────────────────
echo "No-args behavior:"

# No args outside git repo: exits non-zero with git error
NONGIT_NOARGS=$(mktemp -d)
out=$(cd "$NONGIT_NOARGS" && "$SCRIPT" 2>&1); code=$?
check "no args in non-git dir exits non-zero" "1" "$code"
contains "no args in non-git dir prints git error" "not inside a git repository" "$out"
rm -rf "$NONGIT_NOARGS"

# No args with plugin not installed: tells user to run doctor
# Use a restricted PATH without zelligent and no ZELLIGENT_PLUGIN_SRC
MOCK_NOARGS_BIN_NONE=$(mktemp -d)
MOCK_NOARGS_HOME_NONE=$(mktemp -d)
cat > "$MOCK_NOARGS_BIN_NONE/git" <<'MOCK'
#!/bin/bash
# Proxy to real git
/usr/bin/git "$@"
MOCK
chmod +x "$MOCK_NOARGS_BIN_NONE/git"
out=$(ZELLIJ="" ZELLIJ_SESSION_NAME="" HOME="$MOCK_NOARGS_HOME_NONE" ZELLIGENT_PLUGIN_SRC="" PATH="$MOCK_NOARGS_BIN_NONE:/usr/bin:/bin" "$SCRIPT" 2>&1); code=$?
check "no args without plugin exits non-zero" "1" "$code"
contains "no args without plugin: suggests doctor" "zelligent doctor" "$out"
rm -rf "$MOCK_NOARGS_BIN_NONE" "$MOCK_NOARGS_HOME_NONE"

# No args inside Zellij: prints spawn suggestion
# zelligent is in PATH (we're running from the repo), so the check passes
out=$(ZELLIJ=1 ZELLIGENT_PLUGIN_SRC="$SCRIPT" "$SCRIPT" 2>&1); code=$?
check "no args inside zellij exits 0" "0" "$code"
contains "no args inside zellij: suggests spawn" "zelligent spawn" "$out"

# No args outside Zellij with plugin available: starts with zelligent layout
MOCK_NOARGS_LAYOUT_BIN=$(mktemp -d)
FAKE_NOARGS_WASM_DIR=$(mktemp -d)
FAKE_NOARGS_WASM="$FAKE_NOARGS_WASM_DIR/zelligent-plugin.wasm"
echo "fake-wasm" > "$FAKE_NOARGS_WASM"
cat > "$MOCK_NOARGS_LAYOUT_BIN/zellij" <<'MOCK'
#!/bin/bash
if [ "$1" = "list-sessions" ]; then echo ""; exit 0; fi
echo "zellij $*"
for arg in "$@"; do
  if [ -f "$arg" ]; then cat "$arg"; fi
done
MOCK
cat > "$MOCK_NOARGS_LAYOUT_BIN/zelligent" <<'MOCK'
#!/bin/bash
MOCK
chmod +x "$MOCK_NOARGS_LAYOUT_BIN/zellij" "$MOCK_NOARGS_LAYOUT_BIN/zelligent"
out=$(ZELLIJ="" SHELL="/bin/zsh" ZELLIGENT_PLUGIN_SRC="$FAKE_NOARGS_WASM" PATH="$MOCK_NOARGS_LAYOUT_BIN:$PATH" "$SCRIPT" 2>&1); code=$?
check "no args with plugin: exits 0" "0" "$code"
contains "no args with plugin: uses session layout" "--new-session-with-layout" "$out"
contains "no args with plugin: sets default tab template" "default_tab_template" "$out"
contains "no args with plugin: layout has sidebar plugin" 'plugin location="file:' "$out"
contains "no args with plugin: layout names sidebar pane" 'pane name="zelligent"' "$out"
contains "no args with plugin: layout has status-bar" 'plugin location="zellij:status-bar"' "$out"
# 2, not 1: default_tab_template and new_tab_template (#139) each render
# their own copy of the sidebar's outer Vertical split.
count_equals "no args with plugin: default template and new tab template each have a sidebar split" 'split_direction="Vertical"' 2 "$out"
contains "no args with plugin: startup honors SHELL" 'agent_cmd "/bin/zsh"' "$out"
contains "no args with plugin: startup shell reaches layout args" 'exec /bin/zsh' "$out"
# #139: manual tabs (`zellij action new-tab --name X` with no --layout) use
# `new_tab_template`, not `default_tab_template`'s unfillable nested
# {{zelligent_children}} marker — see write_session_layout for why. It must
# carry real sidebar+shell+lazygit content, not just the sidebar.
contains "no args with plugin: sets new tab template"            "new_tab_template"                    "$out"
contains "no args with plugin: new tab template has shell pane"  'pane name="shell"'                   "$out"
contains "no args with plugin: new tab template has lazygit"     'command="lazygit"'                   "$out"

# No args outside Zellij with plugin but no layout: fails clearly
NOARGS_LAYOUT_BACKUP=$(mktemp)
cp "$TEST_REPO_LAYOUT" "$NOARGS_LAYOUT_BACKUP"
rm -f "$TEST_REPO_LAYOUT"
NOARGS_EMPTY_HOME=$(mktemp -d)
out=$(HOME="$NOARGS_EMPTY_HOME" ZELLIJ="" SHELL="/bin/zsh" ZELLIGENT_PLUGIN_SRC="$FAKE_NOARGS_WASM" PATH="$MOCK_NOARGS_LAYOUT_BIN:$PATH" "$SCRIPT" 2>&1); code=$?
check "no args without layout exits non-zero" "1" "$code"
contains "no args without layout: prints error" "no layout found" "$out"
contains "no args without layout: suggests doctor" "zelligent doctor" "$out"
mv "$NOARGS_LAYOUT_BACKUP" "$TEST_REPO_LAYOUT"
rm -rf "$NOARGS_EMPTY_HOME"
rm -rf "$MOCK_NOARGS_LAYOUT_BIN" "$FAKE_NOARGS_WASM_DIR"

# ── Stale socket timeout ──────────────────────────────────────────────────────
echo "Stale socket timeout:"

# Mock zellij that hangs forever (simulates stale socket)
MOCK_HANG_BIN=$(mktemp -d)
FAKE_HANG_WASM_DIR=$(mktemp -d)
FAKE_HANG_WASM="$FAKE_HANG_WASM_DIR/zelligent-plugin.wasm"
echo "fake-wasm" > "$FAKE_HANG_WASM"
cat > "$MOCK_HANG_BIN/zellij" <<'MOCK'
#!/bin/bash
if [ "$1" = "list-sessions" ]; then sleep 60; fi
echo "zellij $*"
MOCK
cat > "$MOCK_HANG_BIN/lazygit" <<'MOCK'
#!/bin/bash
MOCK
cat > "$MOCK_HANG_BIN/zelligent" <<'MOCK'
#!/bin/bash
MOCK
chmod +x "$MOCK_HANG_BIN/zellij" "$MOCK_HANG_BIN/lazygit" "$MOCK_HANG_BIN/zelligent"

# No args outside Zellij with hanging zellij: should time out and create new session
out=$(ZELLIJ="" ZELLIGENT_PLUGIN_SRC="$FAKE_HANG_WASM" TMPDIR="/tmp/fake-zellij-$$" \
  PATH="$MOCK_HANG_BIN:$PATH" "$SCRIPT" 2>&1); code=$?
check "stale socket: exits 0 (falls through to create)" "0" "$code"
contains "stale socket: prints timeout warning" "timed out" "$out"
contains "stale socket: creates session anyway" "Creating Zellij session" "$out"

# Spawn outside Zellij with hanging zellij: should time out and create new session
register_managed_cleanup "$HOME" some-branch
out=$(ZELLIJ="" ZELLIJ_SESSION_NAME="" ZELLIGENT_PLUGIN_SRC="$FAKE_HANG_WASM" TMPDIR="/tmp/fake-zellij-$$" \
  PATH="$MOCK_HANG_BIN:$PATH" "$SCRIPT" spawn some-branch 2>&1); code=$?
cleanup_stale() {
  git -C "$REPO_ROOT" worktree remove --force \
    "$HOME/.zelligent/worktrees/$REPO_NAME/some-branch" &>/dev/null || true
  git -C "$REPO_ROOT" branch -D some-branch &>/dev/null || true
}
cleanup_stale
check "stale socket spawn: exits 0" "0" "$code"
contains "stale socket spawn: prints timeout warning" "timed out" "$out"
contains "stale socket spawn: creates new session" "Creating Zellij session" "$out"

# When TMPDIR has a zellij socket dir, the warning includes cleanup command
FAKE_TMPDIR=$(mktemp -d)
mkdir -p "$FAKE_TMPDIR/zellij-fake"
out=$(ZELLIJ="" ZELLIGENT_PLUGIN_SRC="$FAKE_HANG_WASM" TMPDIR="$FAKE_TMPDIR" \
  PATH="$MOCK_HANG_BIN:$PATH" "$SCRIPT" 2>&1); code=$?
contains "stale socket: shows cleanup command" "rm -rf" "$out"
rm -rf "$FAKE_TMPDIR"

rm -rf "$MOCK_HANG_BIN" "$FAKE_HANG_WASM_DIR"

# ── Stale session reconciliation (#155/#157/#158) ──────────────────────────────
echo "Stale session reconciliation:"

# Pull the reconciliation functions straight out of zelligent.sh (same
# awk-extraction pattern as the pane_name_for_agent_cmd unit tests above) and
# drive them directly against a fabricated cache dir, so these tests never
# touch a real zellij cache. ZELLIGENT_ZELLIJ_CACHE_ROOTS points the glob
# helpers at the fake dir instead of the real platform cache bases.
RECONCILE_FNS=$(
  awk '/^run_with_timeout\(\) \{/,/^\}$/' "$SCRIPT"
  awk '/^zellij_cache_roots\(\) \{/,/^\}$/' "$SCRIPT"
  awk '/^serialized_session_dirs\(\) \{/,/^\}$/' "$SCRIPT"
  awk '/^serialized_layout_files\(\) \{/,/^\}$/' "$SCRIPT"
  awk '/^all_serialized_layout_files\(\) \{/,/^\}$/' "$SCRIPT"
  awk '/^zellij_list_sessions_long\(\) \{/,/^\}$/' "$SCRIPT"
  awk '/^session_state\(\) \{/,/^\}$/' "$SCRIPT"
  awk '/^extract_plugin_file_urls\(\) \{/,/^\}$/' "$SCRIPT"
  awk '/^validate_plugin_url\(\) \{/,/^\}$/' "$SCRIPT"
  awk '/^layout_stale_kind\(\) \{/,/^\}$/' "$SCRIPT"
  awk '/^drop_stale_session\(\) \{/,/^\}$/' "$SCRIPT"
  awk '/^reconcile_serialized_session\(\) \{/,/^\}$/' "$SCRIPT"
)

FAKE_ZCACHE=$(mktemp -d)
FAKE_ZPLUGINS=$(mktemp -d)
mkdir -p "$FAKE_ZCACHE/contract_version_1/session_info"
CURRENT_WASM="$FAKE_ZPLUGINS/current/zelligent-plugin.wasm"
OLD_PATH_WASM="$FAKE_ZPLUGINS/old/zelligent-plugin.wasm"
SCRIPT_BYTES_WASM="$FAKE_ZPLUGINS/scriptdir/zelligent-plugin.wasm"
mkdir -p "$(dirname "$CURRENT_WASM")" "$(dirname "$OLD_PATH_WASM")" "$(dirname "$SCRIPT_BYTES_WASM")"
printf '\0asm\x01\x00\x00\x00' > "$CURRENT_WASM"
printf '\0asm\x01\x00\x00\x00' > "$OLD_PATH_WASM"
printf '#!/bin/bash\necho hi\n' > "$SCRIPT_BYTES_WASM"

mk_fake_session() {
  local name="$1" url="$2"
  local dir="$FAKE_ZCACHE/contract_version_1/session_info/$name"
  mkdir -p "$dir"
  if [ -n "$url" ]; then
    printf 'layout {\n    pane { plugin location="file:%s" }\n}\n' "$url" > "$dir/session-layout.kdl"
  else
    printf 'layout {\n    pane { plugin location="zellij:status-bar" }\n}\n' > "$dir/session-layout.kdl"
  fi
}

mk_fake_session "healthy-sess"     "$CURRENT_WASM"
mk_fake_session "missing-sess"     "$FAKE_ZPLUGINS/gone/zelligent-plugin.wasm"
mk_fake_session "scriptbytes-sess" "$SCRIPT_BYTES_WASM"
mk_fake_session "wrongpath-sess"   "$OLD_PATH_WASM"
mk_fake_session "nofileurl-sess"   ""
mk_fake_session "alive-sess"       "$OLD_PATH_WASM"

MOCK_RECONCILE=$(mktemp -d)
RECONCILE_DELETE_LOG=$(mktemp)
cat > "$MOCK_RECONCILE/zellij" <<MOCK
#!/bin/bash
if [ "\$1" = "list-sessions" ]; then
  cat <<LIST
healthy-sess [Created 5s ago] (EXITED - attach to resurrect)
missing-sess [Created 5s ago] (EXITED - attach to resurrect)
scriptbytes-sess [Created 5s ago] (EXITED - attach to resurrect)
wrongpath-sess [Created 5s ago] (EXITED - attach to resurrect)
nofileurl-sess [Created 5s ago] (EXITED - attach to resurrect)
alive-sess [Created 5s ago]
LIST
  exit 0
fi
if [ "\$1" = "delete-session" ]; then
  echo "DELETED:\$3" >> "$RECONCILE_DELETE_LOG"
  exit 0
fi
exit 0
MOCK
chmod +x "$MOCK_RECONCILE/zellij"

run_reconcile() {
  local name="$1"
  PATH="$MOCK_RECONCILE:$PATH" ZELLIGENT_ZELLIJ_CACHE_ROOTS="$FAKE_ZCACHE" \
    bash -c "$RECONCILE_FNS
reconcile_serialized_session '$name' '$CURRENT_WASM'" 2>&1
}

out=$(run_reconcile healthy-sess)
check "reconcile: healthy wasm URL prints nothing" "" "$out"
not_contains "reconcile: healthy-sess not deleted" "DELETED:healthy-sess" "$(cat "$RECONCILE_DELETE_LOG")"

out=$(run_reconcile missing-sess)
contains "reconcile: missing plugin file dropped" "Dropped stale saved session 'missing-sess'" "$out"
contains "reconcile: missing-sess delete-session called" "DELETED:missing-sess" "$(cat "$RECONCILE_DELETE_LOG")"

out=$(run_reconcile scriptbytes-sess)
contains "reconcile: script-bytes (bad magic) dropped" "Dropped stale saved session 'scriptbytes-sess'" "$out"
contains "reconcile: scriptbytes-sess delete-session called" "DELETED:scriptbytes-sess" "$(cat "$RECONCILE_DELETE_LOG")"

out=$(run_reconcile wrongpath-sess)
contains "reconcile: valid wasm at wrong (drifted) path dropped" "Dropped stale saved session 'wrongpath-sess'" "$out"
contains "reconcile: wrongpath-sess delete-session called" "DELETED:wrongpath-sess" "$(cat "$RECONCILE_DELETE_LOG")"

out=$(run_reconcile nofileurl-sess)
check "reconcile: no file: URLs (fail-open) prints nothing" "" "$out"
not_contains "reconcile: nofileurl-sess not deleted" "DELETED:nofileurl-sess" "$(cat "$RECONCILE_DELETE_LOG")"

out=$(run_reconcile alive-sess)
check "reconcile: alive session with stale URL untouched" "" "$out"
not_contains "reconcile: alive-sess never deleted" "DELETED:alive-sess" "$(cat "$RECONCILE_DELETE_LOG")"

rm -rf "$MOCK_RECONCILE" "$RECONCILE_DELETE_LOG" "$FAKE_ZCACHE" "$FAKE_ZPLUGINS"

# ── Argument validation ────────────────────────────────────────────────────────
echo "Argument validation:"

out=$("$SCRIPT" remove 2>&1); code=$?
check "remove without branch exits non-zero" "1" "$code"
contains "remove without branch prints usage" "Usage:" "$out"

out=$("$SCRIPT" spawn 2>&1); code=$?
check "spawn without branch exits non-zero" "1" "$code"
contains "spawn without branch prints usage" "Usage:" "$out"

out=$("$SCRIPT" bogus 2>&1); code=$?
check "unknown command exits non-zero" "1" "$code"
contains "unknown command prints usage" "Usage:" "$out"

# ── Environment checks ────────────────────────────────────────────────────────
echo "Environment checks:"

NONGIT=$(mktemp -d)
out=$(cd "$NONGIT" && "$SCRIPT" spawn some-branch 2>&1); code=$?
check "non-git dir exits non-zero" "1" "$code"
contains "non-git dir prints error" "not inside a git repository" "$out"
rm -rf "$NONGIT"

# ── Nuke subcommand ───────────────────────────────────────────────────────────
echo "Nuke subcommand:"

# nuke inside zellij: exits non-zero
out=$(ZELLIJ=1 "$SCRIPT" nuke 2>&1); code=$?
check "nuke inside zellij exits non-zero" "1" "$code"
contains "nuke inside zellij prints error" "cannot nuke from inside" "$out"

# nuke with no session: exits 0 (idempotent)
MOCK_NUKE=$(mktemp -d)
cat > "$MOCK_NUKE/zellij" <<'MOCK'
#!/bin/bash
if [ "$1" = "delete-session" ]; then exit 1; fi
if [ "$1" = "--version" ]; then echo "zellij 0.43.1"; exit 0; fi
MOCK
# Mock ps/kill so nuke tests never touch real processes
cat > "$MOCK_NUKE/ps" <<'MOCK'
#!/bin/bash
echo ""
MOCK
cat > "$MOCK_NUKE/kill" <<'MOCK'
#!/bin/bash
exit 0
MOCK
cat > "$MOCK_NUKE/sleep" <<'MOCK'
#!/bin/bash
exit 0
MOCK
chmod +x "$MOCK_NUKE/zellij" "$MOCK_NUKE/ps" "$MOCK_NUKE/kill" "$MOCK_NUKE/sleep"
FAKE_HOME=$(mktemp -d)
out=$(cd "$REPO_ROOT" && ZELLIJ="" HOME="$FAKE_HOME" XDG_CACHE_HOME="$FAKE_HOME/.cache" TMPDIR="$FAKE_HOME/tmp" PATH="$MOCK_NUKE:$PATH" "$SCRIPT" nuke 2>&1); code=$?
check "nuke no session exits 0" "0" "$code"
contains "nuke no session prints success" "start fresh" "$out"
rm -rf "$MOCK_NUKE" "$FAKE_HOME"

# nuke with existing session: exits 0
MOCK_NUKE2=$(mktemp -d)
cat > "$MOCK_NUKE2/zellij" <<'MOCK'
#!/bin/bash
if [ "$1" = "delete-session" ]; then exit 0; fi
if [ "$1" = "--version" ]; then echo "zellij 0.43.1"; exit 0; fi
MOCK
cat > "$MOCK_NUKE2/ps" <<'MOCK'
#!/bin/bash
echo ""
MOCK
cat > "$MOCK_NUKE2/kill" <<'MOCK'
#!/bin/bash
exit 0
MOCK
cat > "$MOCK_NUKE2/sleep" <<'MOCK'
#!/bin/bash
exit 0
MOCK
chmod +x "$MOCK_NUKE2/zellij" "$MOCK_NUKE2/ps" "$MOCK_NUKE2/kill" "$MOCK_NUKE2/sleep"
FAKE_HOME2=$(mktemp -d)
out=$(cd "$REPO_ROOT" && ZELLIJ="" HOME="$FAKE_HOME2" XDG_CACHE_HOME="$FAKE_HOME2/.cache" TMPDIR="$FAKE_HOME2/tmp" PATH="$MOCK_NUKE2:$PATH" "$SCRIPT" nuke 2>&1); code=$?
check "nuke existing session exits 0" "0" "$code"
contains "nuke existing session prints success" "start fresh" "$out"
rm -rf "$MOCK_NUKE2" "$FAKE_HOME2"

# nuke cache glob (#158): zellij --version reports "0.43.1" but the actual
# on-disk cache dir is contract_version_1 (the 0.44+ naming scheme) — the
# pre-#158 code hardcoded the version-named path and silently found nothing.
# Glob for any dir with a session_info/<name> entry instead.
MOCK_NUKE3=$(mktemp -d)
cat > "$MOCK_NUKE3/zellij" <<'MOCK'
#!/bin/bash
if [ "$1" = "delete-session" ]; then exit 0; fi
if [ "$1" = "--version" ]; then echo "zellij 0.43.1"; exit 0; fi
MOCK
cat > "$MOCK_NUKE3/ps" <<'MOCK'
#!/bin/bash
echo ""
MOCK
cat > "$MOCK_NUKE3/kill" <<'MOCK'
#!/bin/bash
exit 0
MOCK
cat > "$MOCK_NUKE3/sleep" <<'MOCK'
#!/bin/bash
exit 0
MOCK
chmod +x "$MOCK_NUKE3/zellij" "$MOCK_NUKE3/ps" "$MOCK_NUKE3/kill" "$MOCK_NUKE3/sleep"
FAKE_HOME3=$(mktemp -d)
FAKE_CACHE_DIR="$FAKE_HOME3/.cache/zellij/contract_version_1/session_info/$REPO_NAME"
mkdir -p "$FAKE_CACHE_DIR"
echo "layout stub" > "$FAKE_CACHE_DIR/session-layout.kdl"
echo "metadata stub" > "$FAKE_CACHE_DIR/session-metadata.kdl"
out=$(cd "$REPO_ROOT" && ZELLIJ="" HOME="$FAKE_HOME3" XDG_CACHE_HOME="$FAKE_HOME3/.cache" TMPDIR="$FAKE_HOME3/tmp" PATH="$MOCK_NUKE3:$PATH" "$SCRIPT" nuke 2>&1); code=$?
check "nuke cache glob: exits 0" "0" "$code"
[ ! -d "$FAKE_CACHE_DIR" ]
check "nuke cache glob: removes contract_version_N session_info dir despite version mismatch" "0" "$?"
rm -rf "$MOCK_NUKE3" "$FAKE_HOME3"

# nuke from non-git dir: exits non-zero
NONGIT_NUKE=$(mktemp -d)
out=$(cd "$NONGIT_NUKE" && "$SCRIPT" nuke 2>&1); code=$?
check "nuke non-git dir exits non-zero" "1" "$code"
rm -rf "$NONGIT_NUKE"

# --help lists nuke
out=$("$SCRIPT" --help 2>&1)
contains "--help lists nuke" "nuke" "$out"

# ── Session name budget (#179) ───────────────────────────────────────────────
echo "Session name budget:"

# zellij session names must fit the Unix-socket budget (104 bytes on macOS /
# 108 on Linux for <sock_dir>/<name>). Repos with long directory names must
# get a derived (prefix + hash) session name; short ones keep the repo name.
SNB_LIMIT=108
[ "$(uname)" = "Darwin" ] && SNB_LIMIT=104

MOCK_SNB=$(mktemp -d)
cat > "$MOCK_SNB/zellij" <<'MOCK'
#!/bin/bash
echo "zellij $*"
MOCK
cat > "$MOCK_SNB/lazygit" <<'MOCK'
#!/bin/bash
MOCK
chmod +x "$MOCK_SNB/zellij" "$MOCK_SNB/lazygit"

SNB_HOME=$(mktemp -d)
mkdir -p "$SNB_HOME/.zelligent"
cp "$ZELLIGENT_DEFAULT_LAYOUT_SRC" "$SNB_HOME/.zelligent/layout.kdl"

SNB_REPO_PARENT=$(mktemp -d)
SNB_REPO="$SNB_REPO_PARENT/interview-coding-projects-with-a-very-long-name"
git init -q -b main "$SNB_REPO" && git -C "$SNB_REPO" -c user.email=t@t -c user.name=t commit -q --allow-empty -m init

# A socket dir long enough that the 46-char repo name cannot fit on either
# platform: budget = LIMIT - len(<dir>/contract_version_1) - 2. Must live
# directly under /tmp with a FIXED total length: a mktemp-based base sits
# under macOS's ~49-byte /var/folders $TMPDIR, where 40 more bytes lands in
# the pathological no-budget fallback and the test fails for the wrong
# reason. len(base)=63 → len(sock_dir)=82 → budget 20 (macOS) / 24 (Linux).
SNB_SOCK_ROOT=$(mktemp -d /tmp/zsnb-XXXXXX)   # fixed 16-byte length, unique
SNB_SOCK_BASE="$SNB_SOCK_ROOT/$(perl -e "print 'x' x (62 - length('$SNB_SOCK_ROOT'))")"
mkdir -p "$SNB_SOCK_BASE"
SNB_SOCK_DIR="$SNB_SOCK_BASE/contract_version_1"
SNB_BUDGET=$(( SNB_LIMIT - ${#SNB_SOCK_DIR} - 2 ))

snb_run() {
  (cd "$SNB_REPO" && HOME="$SNB_HOME" ZELLIJ="" ZELLIJ_SOCKET_DIR="$SNB_SOCK_BASE" \
    ZELLIGENT_PLUGIN_SRC="$MOCK_SNB/zellij" PATH="$MOCK_SNB:$PATH" "$SCRIPT" 2>&1)
}
out_snb=$(snb_run)
contains "long repo name: prints shortened-session note" "Note: using session name" "$out_snb"
SNB_SESSION=$(printf '%s\n' "$out_snb" | grep -o -- "--session [^ ]*" | awk '{print $2}' | head -1)
check "long repo name: session name is shortened" "false" \
  "$([ "$SNB_SESSION" = "interview-coding-projects-with-a-very-long-name" ] && echo true || echo false)"
check "long repo name: socket path fits the budget" "true" \
  "$([ $(( ${#SNB_SOCK_DIR} + 1 + ${#SNB_SESSION} )) -lt "$SNB_LIMIT" ] && echo true || echo false)"
check "long repo name: session name within computed budget" "true" \
  "$([ "${#SNB_SESSION}" -le "$SNB_BUDGET" ] && echo true || echo false)"
contains "long repo name: shortened name keeps a readable prefix" "interview" "$SNB_SESSION"

# Deterministic: a second run derives the identical session name (sessions
# must be re-attachable across runs)
out_snb2=$(snb_run)
SNB_SESSION2=$(printf '%s\n' "$out_snb2" | grep -o -- "--session [^ ]*" | awk '{print $2}' | head -1)
check "long repo name: derivation is deterministic" "$SNB_SESSION" "$SNB_SESSION2"

# Distinct long names must not collide
SNB_REPO_B="$SNB_REPO_PARENT/interview-coding-projects-with-a-very-long-nomen"
git init -q -b main "$SNB_REPO_B" && git -C "$SNB_REPO_B" -c user.email=t@t -c user.name=t commit -q --allow-empty -m init
out_snb_b=$( (cd "$SNB_REPO_B" && HOME="$SNB_HOME" ZELLIJ="" ZELLIJ_SOCKET_DIR="$SNB_SOCK_BASE" \
  ZELLIGENT_PLUGIN_SRC="$MOCK_SNB/zellij" PATH="$MOCK_SNB:$PATH" "$SCRIPT" 2>&1) )
SNB_SESSION_B=$(printf '%s\n' "$out_snb_b" | grep -o -- "--session [^ ]*" | awk '{print $2}' | head -1)
check "distinct long repo names get distinct sessions" "false" \
  "$([ "$SNB_SESSION" = "$SNB_SESSION_B" ] && echo true || echo false)"

# Budget boundaries: a name of EXACTLY budget length is kept verbatim; one
# byte over is shortened (guards the strict-less-than off-by-one)
SNB_NAME_AT=$(perl -e "print 'b' x $SNB_BUDGET")
SNB_REPO_AT="$SNB_REPO_PARENT/$SNB_NAME_AT"
git init -q -b main "$SNB_REPO_AT" && git -C "$SNB_REPO_AT" -c user.email=t@t -c user.name=t commit -q --allow-empty -m init
out_at=$( (cd "$SNB_REPO_AT" && HOME="$SNB_HOME" ZELLIJ="" ZELLIJ_SOCKET_DIR="$SNB_SOCK_BASE" \
  ZELLIGENT_PLUGIN_SRC="$MOCK_SNB/zellij" PATH="$MOCK_SNB:$PATH" "$SCRIPT" 2>&1) )
contains "name exactly at budget: kept verbatim" "--session $SNB_NAME_AT" "$out_at"
not_contains "name exactly at budget: no note" "Note: using session name" "$out_at"

SNB_NAME_OVER="${SNB_NAME_AT}b"
SNB_REPO_OVER="$SNB_REPO_PARENT/$SNB_NAME_OVER"
git init -q -b main "$SNB_REPO_OVER" && git -C "$SNB_REPO_OVER" -c user.email=t@t -c user.name=t commit -q --allow-empty -m init
out_over=$( (cd "$SNB_REPO_OVER" && HOME="$SNB_HOME" ZELLIJ="" ZELLIJ_SOCKET_DIR="$SNB_SOCK_BASE" \
  ZELLIGENT_PLUGIN_SRC="$MOCK_SNB/zellij" PATH="$MOCK_SNB:$PATH" "$SCRIPT" 2>&1) )
contains "one byte over budget: shortened" "Note: using session name" "$out_over"

# Multibyte repo name: the budget is BYTES (sun_path), not characters — a
# 24-char UTF-8 name can be 48 bytes. The derived session's socket path must
# fit in bytes (wc -c), which only holds with LC_ALL=C length semantics —
# and the derived name must be pure ASCII: truncating raw bytes could split
# a multibyte character and hand zellij invalid UTF-8. Force a UTF-8 locale
# on the invocation so the test bites even when the suite runs under LC_ALL=C
# (if the locale is unavailable bash falls back to C, which is byte-exact
# anyway — the assertions hold either way).
SNB_NAME_UTF8=$(perl -e "binmode(STDOUT); print \"\xc3\xa9\" x 24")
SNB_REPO_UTF8="$SNB_REPO_PARENT/$SNB_NAME_UTF8"
git init -q -b main "$SNB_REPO_UTF8" && git -C "$SNB_REPO_UTF8" -c user.email=t@t -c user.name=t commit -q --allow-empty -m init
out_utf8=$( (cd "$SNB_REPO_UTF8" && HOME="$SNB_HOME" ZELLIJ="" LC_ALL=en_US.UTF-8 ZELLIJ_SOCKET_DIR="$SNB_SOCK_BASE" \
  ZELLIGENT_PLUGIN_SRC="$MOCK_SNB/zellij" PATH="$MOCK_SNB:$PATH" "$SCRIPT" 2>&1) )
SNB_SESSION_UTF8=$(printf '%s\n' "$out_utf8" | grep -o -- "--session [^ ]*" | awk '{print $2}' | head -1)
check "multibyte repo name: socket path fits the BYTE budget" "true" \
  "$([ "$(printf '%s' "$SNB_SOCK_DIR/$SNB_SESSION_UTF8" | wc -c)" -lt "$SNB_LIMIT" ] && echo true || echo false)"
check "multibyte repo name: derived session name is pure ASCII" "0" \
  "$(printf '%s' "$SNB_SESSION_UTF8" | LC_ALL=C grep -c '[^A-Za-z0-9._-]' || true)"

# Short repo names are untouched — no note, session name == repo name
SNB_REPO_SHORT="$SNB_REPO_PARENT/tiny"
git init -q -b main "$SNB_REPO_SHORT" && git -C "$SNB_REPO_SHORT" -c user.email=t@t -c user.name=t commit -q --allow-empty -m init
out_short=$( (cd "$SNB_REPO_SHORT" && HOME="$SNB_HOME" ZELLIJ="" ZELLIJ_SOCKET_DIR="$SNB_SOCK_BASE" \
  ZELLIGENT_PLUGIN_SRC="$MOCK_SNB/zellij" PATH="$MOCK_SNB:$PATH" "$SCRIPT" 2>&1) )
not_contains "short repo name: no shortened-session note" "Note: using session name" "$out_short"
contains "short repo name: session keeps the repo name" "--session tiny" "$out_short"

# Against the REAL zellij binary (when present): the full long name must
# trip zellij's own validator under this socket dir, and the derived name
# must pass it — this pins the budget math to the actual binary, so a future
# zellij release changing the socket layout fails here instead of in the
# field.
if command -v zellij &>/dev/null; then
  snb_real_rejected=$(ZELLIJ_SOCKET_DIR="$SNB_SOCK_BASE" zellij \
    --session interview-coding-projects-with-a-very-long-name action dump-layout 2>&1 || true)
  contains "real zellij rejects the full long name" "session name must be less than" "$snb_real_rejected"
  snb_real_accepted=$(ZELLIJ_SOCKET_DIR="$SNB_SOCK_BASE" zellij \
    --session "$SNB_SESSION" action dump-layout 2>&1 || true)
  not_contains "real zellij accepts the derived name" "session name must be less than" "$snb_real_accepted"
  snb_real_utf8=$(ZELLIJ_SOCKET_DIR="$SNB_SOCK_BASE" zellij \
    --session "$SNB_SESSION_UTF8" action dump-layout 2>&1 || true)
  not_contains "real zellij accepts the multibyte-derived name" "session name must be less than" "$snb_real_utf8"
  not_contains "real zellij sees valid unicode in the derived name" "invalid" "$snb_real_utf8"
else
  echo "  ⚠️  Zellij not found, skipping real-binary session-name validation"
fi

rm -rf "$MOCK_SNB" "$SNB_HOME" "$SNB_REPO_PARENT" "$SNB_SOCK_ROOT"

# ── Doctor subcommand ────────────────────────────────────────────────────────
echo "Doctor subcommand:"

# doctor without zellij in PATH: exits non-zero
MOCK_DR_NOZELLIJ=$(mktemp -d)
MOCK_DR_HOME=$(mktemp -d)
# Keep system PATH for basic commands, but ensure no zellij
out=$(HOME="$MOCK_DR_HOME" PATH="$MOCK_DR_NOZELLIJ:/usr/bin:/bin" "$SCRIPT" doctor 2>&1); code=$?
check "doctor without zellij exits non-zero" "1" "$code"
contains "doctor without zellij: prints error" "not found" "$out"
rm -rf "$MOCK_DR_NOZELLIJ" "$MOCK_DR_HOME"

# doctor happy path: patches config with plugin reference
MOCK_DR_BIN=$(mktemp -d)
MOCK_DR_HOME=$(mktemp -d)
FAKE_WASM_DIR=$(mktemp -d)
FAKE_WASM="$FAKE_WASM_DIR/zelligent-plugin.wasm"
echo "fake-wasm-content" > "$FAKE_WASM"
cat > "$MOCK_DR_BIN/zellij" <<'MOCK'
#!/bin/bash
MOCK
chmod +x "$MOCK_DR_BIN/zellij"

out=$(HOME="$MOCK_DR_HOME" ZELLIGENT_PLUGIN_SRC="$FAKE_WASM" \
  PATH="$MOCK_DR_BIN:/usr/bin:/bin" "$SCRIPT" doctor 2>&1); code=$?
check "doctor exits 0" "0" "$code"
check "doctor creates config.kdl" "true" \
  "$([ -f "$MOCK_DR_HOME/.config/zellij/config.kdl" ] && echo true || echo false)"
CONFIG_CONTENT=$(cat "$MOCK_DR_HOME/.config/zellij/config.kdl")
not_contains "doctor does not add launcher keybinding" "Ctrl y" "$CONFIG_CONTENT"
contains "doctor adds Alt-z focus keybinding" 'bind "Alt z"' "$CONFIG_CONTENT"
contains "doctor keybinding pipes zelligent-focus" "zelligent-focus" "$CONFIG_CONTENT"
not_contains "doctor keybinding embeds no plugin path" "file:" "$CONFIG_CONTENT"
contains "doctor reports keybinding added" "keybinding: added Alt-z (focus sidebar)" "$out"
check "doctor creates user layout" "true" \
  "$([ -f "$MOCK_DR_HOME/.zelligent/layout.kdl" ] && echo true || echo false)"
check "doctor copies shipped default layout" \
  "$(cat "$ZELLIGENT_DEFAULT_LAYOUT_SRC")" \
  "$(cat "$MOCK_DR_HOME/.zelligent/layout.kdl")"

# doctor writes permissions for the plugin path
if [ "$(uname)" = "Darwin" ]; then
  PERM_FILE="$MOCK_DR_HOME/Library/Caches/org.Zellij-Contributors.Zellij/permissions.kdl"
else
  PERM_FILE="$MOCK_DR_HOME/.cache/zellij/permissions.kdl"
fi
check "doctor creates permissions.kdl" "true" \
  "$([ -f "$PERM_FILE" ] && echo true || echo false)"
PERM_CONTENT=$(cat "$PERM_FILE")
contains "doctor permissions use bare path" "$FAKE_WASM" "$PERM_CONTENT"
not_contains "doctor permissions omit file: prefix" "file:$FAKE_WASM" "$PERM_CONTENT"
contains "doctor without claude CLI skips plugin" "claude plugin: claude CLI not found" "$out"

# doctor idempotent: run again, should say "ok" / "already"
CONFIG_BEFORE=$(cat "$MOCK_DR_HOME/.config/zellij/config.kdl")
LAYOUT_BEFORE=$(cat "$MOCK_DR_HOME/.zelligent/layout.kdl")
out2=$(HOME="$MOCK_DR_HOME" ZELLIGENT_PLUGIN_SRC="$FAKE_WASM" \
  PATH="$MOCK_DR_BIN:/usr/bin:/bin" "$SCRIPT" doctor 2>&1); code2=$?
check "doctor idempotent exits 0" "0" "$code2"
contains "doctor idempotent: plugin ok" "plugin: ok" "$out2"
contains "doctor idempotent: keybinding ok" "keybinding: ok (Alt z" "$out2"
contains "doctor idempotent: claude plugin skipped" "claude plugin: claude CLI not found" "$out2"
contains "doctor idempotent: layout ok" "layout: ok" "$out2"
CONFIG_AFTER=$(cat "$MOCK_DR_HOME/.config/zellij/config.kdl")
LAYOUT_AFTER=$(cat "$MOCK_DR_HOME/.zelligent/layout.kdl")
check "doctor idempotent: config unchanged" "$CONFIG_BEFORE" "$CONFIG_AFTER"
check "doctor idempotent: layout unchanged" "$LAYOUT_BEFORE" "$LAYOUT_AFTER"

# doctor with drifted user layout: reports overwrite command but does not rewrite
cat > "$MOCK_DR_HOME/.zelligent/layout.kdl" <<'KDL'
// drifted-layout
pane split_direction="Vertical" {
    pane size="40%" {
        {{zelligent_sidebar}}
    }
    {{zelligent_children}}
}
KDL
DRIFTED_LAYOUT_BEFORE=$(cat "$MOCK_DR_HOME/.zelligent/layout.kdl")
out_drift=$(HOME="$MOCK_DR_HOME" ZELLIGENT_PLUGIN_SRC="$FAKE_WASM" \
  PATH="$MOCK_DR_BIN:/usr/bin:/bin" "$SCRIPT" doctor 2>&1); code_drift=$?
check "doctor drift exits 0" "0" "$code_drift"
contains "doctor drift: reports custom layout" "layout: custom user layout differs from shipped default" "$out_drift"
contains "doctor drift: prints overwrite command" "Overwrite with: cp" "$out_drift"
check "doctor drift: does not rewrite layout" "$DRIFTED_LAYOUT_BEFORE" "$(cat "$MOCK_DR_HOME/.zelligent/layout.kdl")"

rm -rf "$MOCK_DR_BIN" "$MOCK_DR_HOME" "$FAKE_WASM_DIR"

# doctor sweep (#157): auto-fixes an EXITED session whose own sidebar URL is
# stale, but only warns (never deletes) for an alive session with a stale
# URL and for an EXITED session broken only by a third-party plugin's URL.
MOCK_DR_SWEEP=$(mktemp -d)
MOCK_DR_SWEEP_HOME=$(mktemp -d)
FAKE_SWEEP_WASM_DIR=$(mktemp -d)
FAKE_SWEEP_WASM="$FAKE_SWEEP_WASM_DIR/zelligent-plugin.wasm"
printf '\0asm\x01\x00\x00\x00' > "$FAKE_SWEEP_WASM"

SWEEP_CACHE="$MOCK_DR_SWEEP_HOME/.cache/zellij/contract_version_1/session_info"
mkdir -p "$SWEEP_CACHE/exited-zelligent-stale" "$SWEEP_CACHE/alive-stale" "$SWEEP_CACHE/exited-thirdparty-stale"
printf 'layout {\n    pane { plugin location="file:%s/gone/zelligent-plugin.wasm" }\n}\n' "$FAKE_SWEEP_WASM_DIR" \
  > "$SWEEP_CACHE/exited-zelligent-stale/session-layout.kdl"
printf 'layout {\n    pane { plugin location="file:%s/gone/zelligent-plugin.wasm" }\n}\n' "$FAKE_SWEEP_WASM_DIR" \
  > "$SWEEP_CACHE/alive-stale/session-layout.kdl"
printf 'layout {\n    pane { plugin location="file:%s/gone/zjstatus.wasm" }\n}\n' "$FAKE_SWEEP_WASM_DIR" \
  > "$SWEEP_CACHE/exited-thirdparty-stale/session-layout.kdl"

cat > "$MOCK_DR_SWEEP/zellij" <<'MOCK'
#!/bin/bash
if [ "$1" = "list-sessions" ]; then
  cat <<LIST
exited-zelligent-stale [Created 5s ago] (EXITED - attach to resurrect)
alive-stale [Created 5s ago]
exited-thirdparty-stale [Created 5s ago] (EXITED - attach to resurrect)
LIST
  exit 0
fi
if [ "$1" = "delete-session" ]; then exit 0; fi
exit 0
MOCK
chmod +x "$MOCK_DR_SWEEP/zellij"

out_sweep=$(HOME="$MOCK_DR_SWEEP_HOME" ZELLIGENT_PLUGIN_SRC="$FAKE_SWEEP_WASM" \
  PATH="$MOCK_DR_SWEEP:/usr/bin:/bin" "$SCRIPT" doctor 2>&1); code_sweep=$?
check "doctor sweep: exits 0" "0" "$code_sweep"
contains "doctor sweep: header printed" "Serialized sessions:" "$out_sweep"
contains "doctor sweep: auto-drops exited zelligent-stale session" "Dropped stale saved session 'exited-zelligent-stale'" "$out_sweep"
contains "doctor sweep: warns (does not silently fix) alive-but-stale session" "alive-stale (alive): stale" "$out_sweep"
contains "doctor sweep: alive-stale gets a fix command, not auto-deletion" "delete-session --force 'alive-stale'" "$out_sweep"
contains "doctor sweep: warns on exited session broken only by third-party plugin" "exited-thirdparty-stale (exited): stale" "$out_sweep"
excludes "doctor sweep: does not claim to have dropped the third-party-broken session" "Dropped stale saved session 'exited-thirdparty-stale'" "$out_sweep"
check "doctor sweep: alive-stale cache dir untouched" "true" \
  "$([ -d "$SWEEP_CACHE/alive-stale" ] && echo true || echo false)"
check "doctor sweep: exited-thirdparty-stale cache dir untouched" "true" \
  "$([ -d "$SWEEP_CACHE/exited-thirdparty-stale" ] && echo true || echo false)"
check "doctor sweep: exited-zelligent-stale cache dir removed" "false" \
  "$([ -d "$SWEEP_CACHE/exited-zelligent-stale" ] && echo true || echo false)"

rm -rf "$MOCK_DR_SWEEP" "$MOCK_DR_SWEEP_HOME" "$FAKE_SWEEP_WASM_DIR"

# doctor with existing keybinds block in config: preserves existing keybinds
MOCK_DR_BIN2=$(mktemp -d)
MOCK_DR_HOME2=$(mktemp -d)
FAKE_WASM_DIR2=$(mktemp -d)
FAKE_WASM2="$FAKE_WASM_DIR2/zelligent-plugin.wasm"
echo "fake-wasm" > "$FAKE_WASM2"
cat > "$MOCK_DR_BIN2/zellij" <<'MOCK'
#!/bin/bash
MOCK
chmod +x "$MOCK_DR_BIN2/zellij"
mkdir -p "$MOCK_DR_HOME2/.config/zellij"
cat > "$MOCK_DR_HOME2/.config/zellij/config.kdl" <<'KDL'
keybinds {
    shared_except "locked" {
        bind "Ctrl x" {
            Quit
        }
    }
}
KDL

out=$(HOME="$MOCK_DR_HOME2" ZELLIGENT_PLUGIN_SRC="$FAKE_WASM2" \
  PATH="$MOCK_DR_BIN2:$PATH" "$SCRIPT" doctor 2>&1); code=$?
check "doctor with existing keybinds exits 0" "0" "$code"
CONFIG_CONTENT2=$(cat "$MOCK_DR_HOME2/.config/zellij/config.kdl")
contains "doctor preserves existing keybinds" "Ctrl x" "$CONFIG_CONTENT2"
not_contains "doctor does not add new keybinding" "Ctrl y" "$CONFIG_CONTENT2"
# Alt-z must land INSIDE the existing keybinds block: zellij parses only the
# first top-level `keybinds` node, so a second appended block would be
# silently ignored.
contains "doctor adds Alt-z into existing keybinds block" 'bind "Alt z"' "$CONFIG_CONTENT2"
contains "doctor reports insertion into existing block" "keybinding: added Alt-z (focus sidebar) to keybinds block" "$out"
check "doctor keeps a single top-level keybinds block" "1" \
  "$(grep -c '^keybinds' "$MOCK_DR_HOME2/.config/zellij/config.kdl")"

# doctor idempotent with merged keybinds block: second run leaves it alone
CONFIG_BEFORE2=$(cat "$MOCK_DR_HOME2/.config/zellij/config.kdl")
out=$(HOME="$MOCK_DR_HOME2" ZELLIGENT_PLUGIN_SRC="$FAKE_WASM2" \
  PATH="$MOCK_DR_BIN2:$PATH" "$SCRIPT" doctor 2>&1); code=$?
check "doctor rerun with merged keybinds exits 0" "0" "$code"
contains "doctor rerun reports keybinding ok" "keybinding: ok (Alt z" "$out"
check "doctor rerun leaves merged config unchanged" "$CONFIG_BEFORE2" \
  "$(cat "$MOCK_DR_HOME2/.config/zellij/config.kdl")"

rm -rf "$MOCK_DR_BIN2" "$MOCK_DR_HOME2" "$FAKE_WASM_DIR2"

# doctor with a user-owned Alt-z binding: respects it, does not touch config
MOCK_DR_BIN_ALTZ=$(mktemp -d)
MOCK_DR_HOME_ALTZ=$(mktemp -d)
FAKE_WASM_DIR_ALTZ=$(mktemp -d)
FAKE_WASM_ALTZ="$FAKE_WASM_DIR_ALTZ/zelligent-plugin.wasm"
echo "fake-wasm" > "$FAKE_WASM_ALTZ"
cat > "$MOCK_DR_BIN_ALTZ/zellij" <<'MOCK'
#!/bin/bash
MOCK
chmod +x "$MOCK_DR_BIN_ALTZ/zellij"
mkdir -p "$MOCK_DR_HOME_ALTZ/.config/zellij"
cat > "$MOCK_DR_HOME_ALTZ/.config/zellij/config.kdl" <<'KDL'
keybinds {
    shared_except "locked" {
        bind "Alt z" {
            ToggleFloatingPanes
        }
    }
}
KDL
ALTZ_CONFIG_BEFORE=$(cat "$MOCK_DR_HOME_ALTZ/.config/zellij/config.kdl")

out=$(HOME="$MOCK_DR_HOME_ALTZ" ZELLIGENT_PLUGIN_SRC="$FAKE_WASM_ALTZ" \
  PATH="$MOCK_DR_BIN_ALTZ:$PATH" "$SCRIPT" doctor 2>&1); code=$?
check "doctor with user Alt-z exits 0" "0" "$code"
contains "doctor with user Alt-z reports skip" "keybinding: skipped (Alt z already bound" "$out"
not_contains "doctor with user Alt-z adds no zelligent-focus bind" "zelligent-focus" \
  "$(cat "$MOCK_DR_HOME_ALTZ/.config/zellij/config.kdl")"
check "doctor with user Alt-z leaves keybinds unchanged" "$ALTZ_CONFIG_BEFORE" \
  "$(grep -Ev 'serialization_interval|copy_command' "$MOCK_DR_HOME_ALTZ/.config/zellij/config.kdl")"

rm -rf "$MOCK_DR_BIN_ALTZ" "$MOCK_DR_HOME_ALTZ" "$FAKE_WASM_DIR_ALTZ"

# doctor with content sharing the keybinds opening line (`keybinds { normal {`
# is valid KDL): the displaced content must land on its own line, not get
# glued to our section's closing brace (which would corrupt a working config)
MOCK_DR_BIN_GLUE=$(mktemp -d)
MOCK_DR_HOME_GLUE=$(mktemp -d)
FAKE_WASM_DIR_GLUE=$(mktemp -d)
FAKE_WASM_GLUE="$FAKE_WASM_DIR_GLUE/zelligent-plugin.wasm"
echo "fake-wasm" > "$FAKE_WASM_GLUE"
cat > "$MOCK_DR_BIN_GLUE/zellij" <<'MOCK'
#!/bin/bash
MOCK
chmod +x "$MOCK_DR_BIN_GLUE/zellij"
mkdir -p "$MOCK_DR_HOME_GLUE/.config/zellij"
cat > "$MOCK_DR_HOME_GLUE/.config/zellij/config.kdl" <<'KDL'
keybinds { normal {
        bind "Ctrl x" { Quit; }
    }
}
KDL

out=$(HOME="$MOCK_DR_HOME_GLUE" ZELLIGENT_PLUGIN_SRC="$FAKE_WASM_GLUE" \
  PATH="$MOCK_DR_BIN_GLUE:$PATH" "$SCRIPT" doctor 2>&1); code=$?
check "doctor with inline keybinds content exits 0" "0" "$code"
GLUE_CONFIG=$(cat "$MOCK_DR_HOME_GLUE/.config/zellij/config.kdl")
contains "doctor inline: Alt-z inserted" 'bind "Alt z"' "$GLUE_CONFIG"
contains "doctor inline: reports added" "keybinding: added Alt-z (focus sidebar) to keybinds block" "$out"
contains "doctor inline: user mode block preserved" 'bind "Ctrl x" { Quit; }' "$GLUE_CONFIG"
# The displaced `normal {` must start its own line — a `}` glued before it
# is the corruption this fixture guards against.
check "doctor inline: displaced content on its own line" "1" \
  "$(grep -c '^normal {' "$MOCK_DR_HOME_GLUE/.config/zellij/config.kdl")"
not_contains "doctor inline: no glued brace before displaced content" '} normal {' "$GLUE_CONFIG"
check "doctor inline: single top-level keybinds block" "1" \
  "$(grep -c '^keybinds' "$MOCK_DR_HOME_GLUE/.config/zellij/config.kdl")"

rm -rf "$MOCK_DR_BIN_GLUE" "$MOCK_DR_HOME_GLUE" "$FAKE_WASM_DIR_GLUE"

# doctor with a multi-key user binding (`bind "Alt x" "Alt z"` — zellij's own
# default config uses this form): must be detected as a conflict and skipped
MOCK_DR_BIN_MK=$(mktemp -d)
MOCK_DR_HOME_MK=$(mktemp -d)
FAKE_WASM_DIR_MK=$(mktemp -d)
FAKE_WASM_MK="$FAKE_WASM_DIR_MK/zelligent-plugin.wasm"
echo "fake-wasm" > "$FAKE_WASM_MK"
cat > "$MOCK_DR_BIN_MK/zellij" <<'MOCK'
#!/bin/bash
MOCK
chmod +x "$MOCK_DR_BIN_MK/zellij"
mkdir -p "$MOCK_DR_HOME_MK/.config/zellij"
cat > "$MOCK_DR_HOME_MK/.config/zellij/config.kdl" <<'KDL'
keybinds {
    shared_except "locked" {
        bind "Alt x" "Alt z" { ToggleFloatingPanes; }
    }
}
KDL

out=$(HOME="$MOCK_DR_HOME_MK" ZELLIGENT_PLUGIN_SRC="$FAKE_WASM_MK" \
  PATH="$MOCK_DR_BIN_MK:$PATH" "$SCRIPT" doctor 2>&1); code=$?
check "doctor with multi-key Alt-z exits 0" "0" "$code"
contains "doctor multi-key Alt-z reports skip" "keybinding: skipped (Alt z already bound" "$out"
not_contains "doctor multi-key Alt-z adds no duplicate" "zelligent-focus" \
  "$(cat "$MOCK_DR_HOME_MK/.config/zellij/config.kdl")"

rm -rf "$MOCK_DR_BIN_MK" "$MOCK_DR_HOME_MK" "$FAKE_WASM_DIR_MK"

# doctor with a symlinked config.kdl (dotfiles setups): the edit must write
# through the symlink, not replace it with a regular file
MOCK_DR_BIN_SYM=$(mktemp -d)
MOCK_DR_HOME_SYM=$(mktemp -d)
FAKE_WASM_DIR_SYM=$(mktemp -d)
FAKE_WASM_SYM="$FAKE_WASM_DIR_SYM/zelligent-plugin.wasm"
echo "fake-wasm" > "$FAKE_WASM_SYM"
cat > "$MOCK_DR_BIN_SYM/zellij" <<'MOCK'
#!/bin/bash
MOCK
chmod +x "$MOCK_DR_BIN_SYM/zellij"
mkdir -p "$MOCK_DR_HOME_SYM/.config/zellij" "$MOCK_DR_HOME_SYM/dotfiles"
cat > "$MOCK_DR_HOME_SYM/dotfiles/config.kdl" <<'KDL'
keybinds {
    normal {
        bind "Ctrl x" { Quit; }
    }
}
KDL
ln -s "$MOCK_DR_HOME_SYM/dotfiles/config.kdl" "$MOCK_DR_HOME_SYM/.config/zellij/config.kdl"

out=$(HOME="$MOCK_DR_HOME_SYM" ZELLIGENT_PLUGIN_SRC="$FAKE_WASM_SYM" \
  PATH="$MOCK_DR_BIN_SYM:$PATH" "$SCRIPT" doctor 2>&1); code=$?
check "doctor with symlinked config exits 0" "0" "$code"
check "doctor preserves config symlink" "true" \
  "$([ -L "$MOCK_DR_HOME_SYM/.config/zellij/config.kdl" ] && echo true || echo false)"
contains "doctor writes Alt-z through the symlink" "zelligent-focus" \
  "$(cat "$MOCK_DR_HOME_SYM/dotfiles/config.kdl")"

rm -rf "$MOCK_DR_BIN_SYM" "$MOCK_DR_HOME_SYM" "$FAKE_WASM_DIR_SYM"

# doctor with plugin source not found: prints error
MOCK_DR_BIN3=$(mktemp -d)
MOCK_DR_HOME3=$(mktemp -d)
cat > "$MOCK_DR_BIN3/zellij" <<'MOCK'
#!/bin/bash
MOCK
chmod +x "$MOCK_DR_BIN3/zellij"

out=$(HOME="$MOCK_DR_HOME3" ZELLIGENT_PLUGIN_SRC="/nonexistent/path.wasm" \
  PATH="$MOCK_DR_BIN3:$PATH" "$SCRIPT" doctor 2>&1); code=$?
check "doctor with missing plugin source exits non-zero" "1" "$code"
contains "doctor with missing plugin source: prints error" "source not found" "$out"

rm -rf "$MOCK_DR_BIN3" "$MOCK_DR_HOME3"

# doctor respects XDG_CONFIG_HOME
MOCK_DR_BIN4=$(mktemp -d)
MOCK_DR_HOME4=$(mktemp -d)
MOCK_XDG=$(mktemp -d)
FAKE_WASM_DIR4=$(mktemp -d)
FAKE_WASM4="$FAKE_WASM_DIR4/zelligent-plugin.wasm"
echo "fake-wasm" > "$FAKE_WASM4"
cat > "$MOCK_DR_BIN4/zellij" <<'MOCK'
#!/bin/bash
MOCK
chmod +x "$MOCK_DR_BIN4/zellij"

out=$(HOME="$MOCK_DR_HOME4" XDG_CONFIG_HOME="$MOCK_XDG" ZELLIGENT_PLUGIN_SRC="$FAKE_WASM4" \
  PATH="$MOCK_DR_BIN4:$PATH" "$SCRIPT" doctor 2>&1); code=$?
check "doctor with XDG_CONFIG_HOME exits 0" "0" "$code"
check "doctor uses XDG_CONFIG_HOME for config" "true" \
  "$([ -f "$MOCK_XDG/zellij/config.kdl" ] && echo true || echo false)"
# Should NOT have created anything under ~/.config
check "doctor does not use ~/.config when XDG set" "false" \
  "$([ -d "$MOCK_DR_HOME4/.config/zellij" ] && echo true || echo false)"

rm -rf "$MOCK_DR_BIN4" "$MOCK_DR_HOME4" "$MOCK_XDG" "$FAKE_WASM_DIR4"

# doctor claude-plugin registration (mocked `claude`, the way `zellij` is
# mocked above). Contract (maintainer decision, #169): doctor NEVER
# introspects or mutates Claude Code's marketplace registrations —
# `marketplace add` is attempted idempotently and a failure that the
# subsequent install/update recovers from is tolerated (older Claude Code
# errored on a name collision for the healthy already-registered case);
# what matters is whether the plugin then installs/updates. Stale dev
# registrations are dev hygiene: `bash dev-install.sh --uninstall`.
# Claude Code writes progress to stdout and the reason for an outcome to
# stderr, so doctor must SURFACE that stderr rather than discard it (#212) —
# the old blanket `2>/dev/null` left users with a bare "Adding marketplace…"
# and a guess about stale state as the only diagnosis.
mock_claude_recording_argv() {
  # Writes a `claude` mock into $1 that appends every invocation's argv to
  # $2 (one line per call) and dispatches canned responses for the doctor
  # flow's plugin subcommands. Failures write a reason to stderr, the way
  # the real CLI does, so tests can assert doctor relays it.
  local bin_dir="$1" log_file="$2" list_output="$3" add_exit="$4" install_exit="${5:-0}"
  local update_exit="${6:-0}"
  cat > "$bin_dir/claude" <<EOF
#!/bin/bash
echo "\$*" >> "$log_file"
case "\$1 \$2 \$3" in
  "plugin marketplace add")
    [ $add_exit -eq 0 ] || echo "MOCK_ADD_FAILED_REASON" >&2
    exit $add_exit ;;
  "plugin marketplace remove") exit 0 ;;
esac
case "\$1 \$2" in
  "plugin list") echo "$list_output"; exit 0 ;;
  "plugin update")
    [ $update_exit -eq 0 ] || echo "MOCK_UPDATE_FAILED_REASON" >&2
    exit $update_exit ;;
  "plugin install")
    [ $install_exit -eq 0 ] || echo "MOCK_INSTALL_FAILED_REASON" >&2
    exit $install_exit ;;
esac
exit 0
EOF
  chmod +x "$bin_dir/claude"
}

# A marketplace source directory doctor will accept: `claude plugin
# marketplace add` requires the manifest, and doctor now pre-checks it.
make_fake_marketplace_dir() {
  local dir="$1"
  mkdir -p "$dir/.claude-plugin"
  echo '{"name":"zelligent","plugins":[]}' > "$dir/.claude-plugin/marketplace.json"
}

# Healthy steady state: add hits the name collision (already registered),
# plugin is installed — doctor must tolerate the collision, update, never
# call marketplace remove, and exit 0.
MOCK_DR_MP1=$(mktemp -d)
MOCK_DR_MP1_HOME=$(mktemp -d)
FAKE_WASM_MP1_DIR=$(mktemp -d)
FAKE_WASM_MP1="$FAKE_WASM_MP1_DIR/zelligent-plugin.wasm"
echo "fake-wasm" > "$FAKE_WASM_MP1"
FAKE_PLUGIN_DIR_MP1=$(mktemp -d)/claude-plugin
make_fake_marketplace_dir "$FAKE_PLUGIN_DIR_MP1"
cat > "$MOCK_DR_MP1/zellij" <<'MOCK'
#!/bin/bash
MOCK
chmod +x "$MOCK_DR_MP1/zellij"
CLAUDE_ARGV_LOG_MP1="$MOCK_DR_MP1_HOME/claude-argv.log"
mock_claude_recording_argv "$MOCK_DR_MP1" "$CLAUDE_ARGV_LOG_MP1" "zelligent@zelligent" 1
out=$(HOME="$MOCK_DR_MP1_HOME" ZELLIGENT_PLUGIN_SRC="$FAKE_WASM_MP1" ZELLIGENT_PLUGIN_DIR="$FAKE_PLUGIN_DIR_MP1" \
  ZELLIGENT_DEFAULT_LAYOUT_SRC="$ZELLIGENT_DEFAULT_LAYOUT_SRC" \
  PATH="$MOCK_DR_MP1:/usr/bin:/bin" "$SCRIPT" doctor 2>&1); code=$?
MP1_ARGV=$(cat "$CLAUDE_ARGV_LOG_MP1")
check "doctor collision: exits 0 (collision is not an error)" "0" "$code"
not_contains "doctor collision: never calls marketplace remove" "plugin marketplace remove" "$MP1_ARGV"
contains "doctor collision: still updates the plugin" "claude plugin: updated" "$out"
contains "doctor collision: never a bare green — prints the stale-registration note" \
  "using the previously registered 'zelligent' marketplace" "$out"
contains "doctor collision: relays why the add failed" "MOCK_ADD_FAILED_REASON" "$out"
contains "doctor collision: hints at restarting sessions" \
  "restart running Claude Code sessions to pick up hook changes" "$out"
rm -rf "$MOCK_DR_MP1" "$MOCK_DR_MP1_HOME" "$FAKE_WASM_MP1_DIR" "$(dirname "$FAKE_PLUGIN_DIR_MP1")"

# Fresh install: add succeeds, plugin not yet installed — doctor installs
# and hints at the restart.
MOCK_DR_MP2=$(mktemp -d)
MOCK_DR_MP2_HOME=$(mktemp -d)
FAKE_WASM_MP2_DIR=$(mktemp -d)
FAKE_WASM_MP2="$FAKE_WASM_MP2_DIR/zelligent-plugin.wasm"
echo "fake-wasm" > "$FAKE_WASM_MP2"
FAKE_PLUGIN_DIR_MP2=$(mktemp -d)/claude-plugin
make_fake_marketplace_dir "$FAKE_PLUGIN_DIR_MP2"
cat > "$MOCK_DR_MP2/zellij" <<'MOCK'
#!/bin/bash
MOCK
chmod +x "$MOCK_DR_MP2/zellij"
CLAUDE_ARGV_LOG_MP2="$MOCK_DR_MP2_HOME/claude-argv.log"
mock_claude_recording_argv "$MOCK_DR_MP2" "$CLAUDE_ARGV_LOG_MP2" "no plugins" 0
out=$(HOME="$MOCK_DR_MP2_HOME" ZELLIGENT_PLUGIN_SRC="$FAKE_WASM_MP2" ZELLIGENT_PLUGIN_DIR="$FAKE_PLUGIN_DIR_MP2" \
  ZELLIGENT_DEFAULT_LAYOUT_SRC="$ZELLIGENT_DEFAULT_LAYOUT_SRC" \
  PATH="$MOCK_DR_MP2:/usr/bin:/bin" "$SCRIPT" doctor 2>&1); code=$?
check "doctor fresh: exits 0" "0" "$code"
contains "doctor fresh: installs the plugin" "claude plugin: installed" "$out"
contains "doctor fresh: hints at restarting sessions" \
  "restart running Claude Code sessions to pick up hook changes" "$out"
rm -rf "$MOCK_DR_MP2" "$MOCK_DR_MP2_HOME" "$FAKE_WASM_MP2_DIR" "$(dirname "$FAKE_PLUGIN_DIR_MP2")"

# Genuine failure: add fails, plugin absent, install fails — doctor must lead
# with the registration failure (the install is only its symptom), relay both
# reasons from claude's stderr, and exit nonzero. It must NOT prescribe
# `marketplace remove`: on current Claude Code re-adding a name replaces the
# registration, so a stale entry is not what an add failure means (#212).
MOCK_DR_MP3=$(mktemp -d)
MOCK_DR_MP3_HOME=$(mktemp -d)
FAKE_WASM_MP3_DIR=$(mktemp -d)
FAKE_WASM_MP3="$FAKE_WASM_MP3_DIR/zelligent-plugin.wasm"
echo "fake-wasm" > "$FAKE_WASM_MP3"
FAKE_PLUGIN_DIR_MP3=$(mktemp -d)/claude-plugin
make_fake_marketplace_dir "$FAKE_PLUGIN_DIR_MP3"
cat > "$MOCK_DR_MP3/zellij" <<'MOCK'
#!/bin/bash
MOCK
chmod +x "$MOCK_DR_MP3/zellij"
CLAUDE_ARGV_LOG_MP3="$MOCK_DR_MP3_HOME/claude-argv.log"
mock_claude_recording_argv "$MOCK_DR_MP3" "$CLAUDE_ARGV_LOG_MP3" "no plugins" 1 1
out=$(HOME="$MOCK_DR_MP3_HOME" ZELLIGENT_PLUGIN_SRC="$FAKE_WASM_MP3" ZELLIGENT_PLUGIN_DIR="$FAKE_PLUGIN_DIR_MP3" \
  ZELLIGENT_DEFAULT_LAYOUT_SRC="$ZELLIGENT_DEFAULT_LAYOUT_SRC" \
  PATH="$MOCK_DR_MP3:/usr/bin:/bin" "$SCRIPT" doctor 2>&1); code=$?
contains "doctor install-failure: leads with the registration failure" \
  "claude plugin: failed to register marketplace" "$out"
contains "doctor install-failure: relays why the add failed" "MOCK_ADD_FAILED_REASON" "$out"
contains "doctor install-failure: relays why the install failed" "MOCK_INSTALL_FAILED_REASON" "$out"
not_contains "doctor install-failure: no longer blames a stale registration" \
  "claude plugin marketplace remove zelligent && zelligent doctor" "$out"
check "doctor install-failure: exits nonzero" "1" "$code"
rm -rf "$MOCK_DR_MP3" "$MOCK_DR_MP3_HOME" "$FAKE_WASM_MP3_DIR" "$(dirname "$FAKE_PLUGIN_DIR_MP3")"

# #212 regression: the bundled marketplace directory exists but has no
# .claude-plugin/marketplace.json (Homebrew upgrade from a release predating
# the bundled plugin). Doctor must diagnose the incomplete install by name,
# point at a reinstall, exit nonzero, and never reach `claude` at all — the
# old code let `marketplace add` fail, swallowed its stderr, and told the
# user to remove a marketplace, which cannot fix a missing manifest.
MOCK_DR_MP4=$(mktemp -d)
MOCK_DR_MP4_HOME=$(mktemp -d)
FAKE_WASM_MP4_DIR=$(mktemp -d)
FAKE_WASM_MP4="$FAKE_WASM_MP4_DIR/zelligent-plugin.wasm"
echo "fake-wasm" > "$FAKE_WASM_MP4"
FAKE_PLUGIN_DIR_MP4=$(mktemp -d)/claude-plugin
mkdir -p "$FAKE_PLUGIN_DIR_MP4/plugins"   # present, but no marketplace manifest
cat > "$MOCK_DR_MP4/zellij" <<'MOCK'
#!/bin/bash
MOCK
chmod +x "$MOCK_DR_MP4/zellij"
CLAUDE_ARGV_LOG_MP4="$MOCK_DR_MP4_HOME/claude-argv.log"
mock_claude_recording_argv "$MOCK_DR_MP4" "$CLAUDE_ARGV_LOG_MP4" "no plugins" 0
out=$(HOME="$MOCK_DR_MP4_HOME" ZELLIGENT_PLUGIN_SRC="$FAKE_WASM_MP4" ZELLIGENT_PLUGIN_DIR="$FAKE_PLUGIN_DIR_MP4" \
  ZELLIGENT_DEFAULT_LAYOUT_SRC="$ZELLIGENT_DEFAULT_LAYOUT_SRC" \
  PATH="$MOCK_DR_MP4:/usr/bin:/bin" "$SCRIPT" doctor 2>&1); code=$?
contains "doctor missing-manifest: names the incomplete install" \
  "claude plugin: incomplete install" "$out"
contains "doctor missing-manifest: names the missing manifest path" \
  "$FAKE_PLUGIN_DIR_MP4/.claude-plugin/marketplace.json" "$out"
contains "doctor missing-manifest: prescribes a reinstall" "brew reinstall zelligent" "$out"
not_contains "doctor missing-manifest: does not prescribe marketplace remove" \
  "marketplace remove" "$out"
check "doctor missing-manifest: never invokes claude" "false" \
  "$([ -s "$CLAUDE_ARGV_LOG_MP4" ] && echo true || echo false)"
check "doctor missing-manifest: exits nonzero" "1" "$code"
rm -rf "$MOCK_DR_MP4" "$MOCK_DR_MP4_HOME" "$FAKE_WASM_MP4_DIR" "$(dirname "$FAKE_PLUGIN_DIR_MP4")"

# A recovered `marketplace add` failure must never print as a bare green,
# on EITHER recovery path. The update-success path had this note; the
# install-success path did not, so a plugin installed through a marketplace
# that is not this install's path was reported as a clean "installed".
MOCK_DR_MP5=$(mktemp -d)
MOCK_DR_MP5_HOME=$(mktemp -d)
FAKE_WASM_MP5_DIR=$(mktemp -d)
FAKE_WASM_MP5="$FAKE_WASM_MP5_DIR/zelligent-plugin.wasm"
echo "fake-wasm" > "$FAKE_WASM_MP5"
FAKE_PLUGIN_DIR_MP5=$(mktemp -d)/claude-plugin
make_fake_marketplace_dir "$FAKE_PLUGIN_DIR_MP5"
cat > "$MOCK_DR_MP5/zellij" <<'MOCK'
#!/bin/bash
MOCK
chmod +x "$MOCK_DR_MP5/zellij"
CLAUDE_ARGV_LOG_MP5="$MOCK_DR_MP5_HOME/claude-argv.log"
mock_claude_recording_argv "$MOCK_DR_MP5" "$CLAUDE_ARGV_LOG_MP5" "no plugins" 1 0
out=$(HOME="$MOCK_DR_MP5_HOME" ZELLIGENT_PLUGIN_SRC="$FAKE_WASM_MP5" ZELLIGENT_PLUGIN_DIR="$FAKE_PLUGIN_DIR_MP5" \
  ZELLIGENT_DEFAULT_LAYOUT_SRC="$ZELLIGENT_DEFAULT_LAYOUT_SRC" \
  PATH="$MOCK_DR_MP5:/usr/bin:/bin" "$SCRIPT" doctor 2>&1); code=$?
check "doctor add-fail+install-ok: exits 0 (the plugin is usable)" "0" "$code"
contains "doctor add-fail+install-ok: still reports the install" "claude plugin: installed" "$out"
contains "doctor add-fail+install-ok: never a bare green — notes the stale registration" \
  "using the previously registered 'zelligent' marketplace" "$out"
contains "doctor add-fail+install-ok: relays why the add failed" "MOCK_ADD_FAILED_REASON" "$out"
rm -rf "$MOCK_DR_MP5" "$MOCK_DR_MP5_HOME" "$FAKE_WASM_MP5_DIR" "$(dirname "$FAKE_PLUGIN_DIR_MP5")"

# Add fails AND update fails: the installed plugin still works so this stays
# non-fatal, but nothing this run attempted took effect. Reporting a bare
# "ok" while discarding the add's reason would repeat the #212 failure of
# hiding the cause behind a reassuring word.
MOCK_DR_MP6=$(mktemp -d)
MOCK_DR_MP6_HOME=$(mktemp -d)
FAKE_WASM_MP6_DIR=$(mktemp -d)
FAKE_WASM_MP6="$FAKE_WASM_MP6_DIR/zelligent-plugin.wasm"
echo "fake-wasm" > "$FAKE_WASM_MP6"
FAKE_PLUGIN_DIR_MP6=$(mktemp -d)/claude-plugin
make_fake_marketplace_dir "$FAKE_PLUGIN_DIR_MP6"
cat > "$MOCK_DR_MP6/zellij" <<'MOCK'
#!/bin/bash
MOCK
chmod +x "$MOCK_DR_MP6/zellij"
CLAUDE_ARGV_LOG_MP6="$MOCK_DR_MP6_HOME/claude-argv.log"
mock_claude_recording_argv "$MOCK_DR_MP6" "$CLAUDE_ARGV_LOG_MP6" "zelligent@zelligent" 1 0 1
out=$(HOME="$MOCK_DR_MP6_HOME" ZELLIGENT_PLUGIN_SRC="$FAKE_WASM_MP6" ZELLIGENT_PLUGIN_DIR="$FAKE_PLUGIN_DIR_MP6" \
  ZELLIGENT_DEFAULT_LAYOUT_SRC="$ZELLIGENT_DEFAULT_LAYOUT_SRC" \
  PATH="$MOCK_DR_MP6:/usr/bin:/bin" "$SCRIPT" doctor 2>&1); code=$?
not_contains "doctor add-fail+update-fail: never reports a bare ok" \
  "claude plugin: ok (update check failed)" "$out"
contains "doctor add-fail+update-fail: says nothing was refreshed" \
  "not refreshed — marketplace registration and update both failed" "$out"
contains "doctor add-fail+update-fail: relays why the add failed" "MOCK_ADD_FAILED_REASON" "$out"
contains "doctor add-fail+update-fail: relays why the update failed" "MOCK_UPDATE_FAILED_REASON" "$out"
rm -rf "$MOCK_DR_MP6" "$MOCK_DR_MP6_HOME" "$FAKE_WASM_MP6_DIR" "$(dirname "$FAKE_PLUGIN_DIR_MP6")"

# The update-success path keeps relaying the update's own stderr when only
# the update fails (add fine) — the tolerated "ok" case must still not be silent.
MOCK_DR_MP7=$(mktemp -d)
MOCK_DR_MP7_HOME=$(mktemp -d)
FAKE_WASM_MP7_DIR=$(mktemp -d)
FAKE_WASM_MP7="$FAKE_WASM_MP7_DIR/zelligent-plugin.wasm"
echo "fake-wasm" > "$FAKE_WASM_MP7"
FAKE_PLUGIN_DIR_MP7=$(mktemp -d)/claude-plugin
make_fake_marketplace_dir "$FAKE_PLUGIN_DIR_MP7"
cat > "$MOCK_DR_MP7/zellij" <<'MOCK'
#!/bin/bash
MOCK
chmod +x "$MOCK_DR_MP7/zellij"
CLAUDE_ARGV_LOG_MP7="$MOCK_DR_MP7_HOME/claude-argv.log"
mock_claude_recording_argv "$MOCK_DR_MP7" "$CLAUDE_ARGV_LOG_MP7" "zelligent@zelligent" 0 0 1
out=$(HOME="$MOCK_DR_MP7_HOME" ZELLIGENT_PLUGIN_SRC="$FAKE_WASM_MP7" ZELLIGENT_PLUGIN_DIR="$FAKE_PLUGIN_DIR_MP7" \
  ZELLIGENT_DEFAULT_LAYOUT_SRC="$ZELLIGENT_DEFAULT_LAYOUT_SRC" \
  PATH="$MOCK_DR_MP7:/usr/bin:/bin" "$SCRIPT" doctor 2>&1); code=$?
check "doctor update-fail only: exits 0 (tolerated)" "0" "$code"
contains "doctor update-fail only: keeps the tolerated ok wording" \
  "claude plugin: ok (update check failed)" "$out"
contains "doctor update-fail only: relays why the update failed" "MOCK_UPDATE_FAILED_REASON" "$out"
not_contains "doctor update-fail only: no stale-registration note when the add succeeded" \
  "using the previously registered 'zelligent' marketplace" "$out"
rm -rf "$MOCK_DR_MP7" "$MOCK_DR_MP7_HOME" "$FAKE_WASM_MP7_DIR" "$(dirname "$FAKE_PLUGIN_DIR_MP7")"

# dev-install --uninstall removes the Claude plugin + marketplace and the
# dev artifacts (grep-level contract; the flag is the designated home for
# dev-environment hygiene that doctor deliberately does not do).
contains "dev-install --uninstall removes the plugin"      'claude plugin uninstall zelligent@zelligent' "$(cat "$SCRIPT_DIR/dev-install.sh")"
contains "dev-install --uninstall removes the marketplace" 'claude plugin marketplace remove zelligent'  "$(cat "$SCRIPT_DIR/dev-install.sh")"

# ── Install script contract ──────────────────────────────────────────────────
echo "Install script contract:"

DEV_INSTALL_CONTENT=$(cat "$SCRIPT_DIR/dev-install.sh")
contains "dev-install copies default layout asset" 'default-layout.kdl' "$DEV_INSTALL_CONTENT"
contains "dev-install creates user layout if missing" 'USER_LAYOUT_DST' "$DEV_INSTALL_CONTENT"
contains "dev-install preserves existing user layout" 'Preserved existing user layout' "$DEV_INSTALL_CONTENT"
contains "default layout asset exists in repo" '{{zelligent_sidebar}}' "$(cat "$REPO_ROOT/share/default-layout.kdl")"
contains "default layout asset contains children placeholder" '{{zelligent_children}}' "$(cat "$REPO_ROOT/share/default-layout.kdl")"

# ── Claude Code plugin bundling ─────────────────────────────────────────────
echo "Claude Code plugin bundling:"

PLUGIN_JSON_CONTENT=$(cat "$SCRIPT_DIR/claude-plugin/plugins/zelligent/.claude-plugin/plugin.json")
RELEASE_YML_CONTENT=$(cat "$SCRIPT_DIR/.github/workflows/release.yml")
HOOKS_JSON_CONTENT=$(cat "$SCRIPT_DIR/claude-plugin/plugins/zelligent/hooks/hooks.json")
LIB_RS_CONTENT=$(cat "$SCRIPT_DIR/plugin/src/lib.rs")

# plugin.json ships a placeholder version, and release.yml's sed stamp targets
# that exact placeholder — cross-grepped so neither can drift alone (a bump to
# one without the other silently breaks version-based update detection; see
# docs/design-docs and the plugin.json version-skip caveat).
contains "plugin.json ships the 0.0.0-dev placeholder" '"version": "0.0.0-dev"' "$PLUGIN_JSON_CONTENT"
contains "release.yml stamps the matching placeholder pattern" '\"version\": \"0.0.0-dev\"' "$RELEASE_YML_CONTENT"

# release.yml staging step must bundle claude-plugin/ into the tarball, or
# doctor's Homebrew-path probe finds nothing to install ("not bundled").
contains "release.yml stages claude-plugin into the tarball" 'cp -R claude-plugin release-staging/' "$RELEASE_YML_CONTENT"
contains "release.yml verifies the plugin.json stamp" 'Failed to stamp' "$RELEASE_YML_CONTENT"

# dev-install.sh must stamp the COPY (not the source tree) with a version
# that's unique per install, so `claude plugin update` never same-version-
# skips a dev refresh.
# Pin the actual uniqueness ingredients (sha + timestamp + pid), not just
# the variable's existence.
contains "dev-install stamps the claude-plugin copy with a unique dev version" 'DEV_PLUGIN_VERSION="${VERSION}-dev.${SHA}.$(date -u +%Y%m%d%H%M%S).$$"' "$DEV_INSTALL_CONTENT"
# Pin that the sed's target is the installed copy: the sed must write to
# $DEV_PLUGIN_JSON, and $DEV_PLUGIN_JSON must be derived from $PLUGIN_DST.
# (A grep for PLUGIN_DST anywhere would pass even if the sed hit the source.)
DEV_STAMP_SED_LINE=$(printf '%s\n' "$DEV_INSTALL_CONTENT" | grep 'DEV_PLUGIN_VERSION' | grep 'sed' | head -1)
DEV_JSON_DEF_LINE=$(printf '%s\n' "$DEV_INSTALL_CONTENT" | grep '^DEV_PLUGIN_JSON=' | head -1)
if [ "${DEV_STAMP_SED_LINE#*\$DEV_PLUGIN_JSON}" != "$DEV_STAMP_SED_LINE" ] \
   && [ "${DEV_JSON_DEF_LINE#*\$PLUGIN_DST}" != "$DEV_JSON_DEF_LINE" ]; then
  pass "dev-install stamp sed targets the installed copy path"
else
  fail "dev-install stamp sed targets the installed copy path (sed: $DEV_STAMP_SED_LINE | def: $DEV_JSON_DEF_LINE)"
fi

# hooks.json's pipe name and event args are one half of a wire protocol whose
# other half is plugin/src/lib.rs's pipe parser — they must never drift
# independently of each other.
contains "hooks.json uses the zelligent-status pipe name" 'zellij pipe --name zelligent-status' "$HOOKS_JSON_CONTENT"
contains "hooks.json sends event=Start" 'event=Start' "$HOOKS_JSON_CONTENT"
contains "hooks.json sends event=Stop" 'event=Stop' "$HOOKS_JSON_CONTENT"
contains "hooks.json sends event=PermissionRequest" 'event=PermissionRequest' "$HOOKS_JSON_CONTENT"
contains "plugin/src/lib.rs parses the zelligent-status pipe name" '"zelligent-status"' "$LIB_RS_CONTENT"
contains "plugin/src/lib.rs matches on \"Start\"" 'Some("Start")' "$LIB_RS_CONTENT"
contains "plugin/src/lib.rs matches on \"Stop\"" 'Some("Stop")' "$LIB_RS_CONTENT"
contains "plugin/src/lib.rs matches on \"PermissionRequest\"" 'Some("PermissionRequest")' "$LIB_RS_CONTENT"

# ── Query subcommands ────────────────────────────────────────────────────────
echo "Query subcommands:"

# show-repo
out=$("$SCRIPT" show-repo 2>&1); code=$?
check "show-repo exits 0" "0" "$code"
contains "show-repo outputs repo_root" "repo_root=" "$out"
contains "show-repo outputs repo_name" "repo_name=" "$out"
# Verify repo_name matches the basename of the repo
EXPECTED_NAME=$(basename "$(echo "$out" | grep '^repo_root=' | cut -d= -f2-)")
ACTUAL_NAME=$(echo "$out" | grep '^repo_name=' | cut -d= -f2-)
check "show-repo name matches root basename" "$EXPECTED_NAME" "$ACTUAL_NAME"

# show-repo from non-git dir
NONGIT2=$(mktemp -d)
out=$(cd "$NONGIT2" && "$SCRIPT" show-repo 2>&1); code=$?
check "show-repo non-git dir exits non-zero" "1" "$code"
rm -rf "$NONGIT2"

# list-worktrees (no managed worktrees exist for this test)
out=$("$SCRIPT" list-worktrees 2>&1); code=$?
check "list-worktrees exits 0" "0" "$code"

# list-worktrees with mismatched dir/branch name
# Create a worktree where the directory name differs from the branch
TEST_WT_BRANCH="test-mismatched-branch-$$"
TEST_WT_DIR="$HOME/.zelligent/worktrees/$REPO_NAME/different-dirname-$$"
mkdir -p "$HOME/.zelligent/worktrees/$REPO_NAME"
register_cleanup_worktree "$TEST_WT_DIR" "$TEST_WT_BRANCH"
git -C "$REPO_ROOT" worktree add -b "$TEST_WT_BRANCH" "$TEST_WT_DIR" HEAD &>/dev/null

out=$("$SCRIPT" list-worktrees 2>&1); code=$?
check "list-worktrees mismatched exits 0" "0" "$code"
contains "list-worktrees mismatched: outputs dir" "different-dirname-$$" "$out"
contains "list-worktrees mismatched: outputs branch" "$TEST_WT_BRANCH" "$out"
# Verify tab-separated format: dir<TAB>branch
contains "list-worktrees mismatched: tab-separated format" "$(printf 'different-dirname-%s\t%s' $$ "$TEST_WT_BRANCH")" "$out"

# remove with mismatched dir/branch name — should resolve path from git metadata
out=$("$SCRIPT" remove "$TEST_WT_BRANCH" 2>&1); code=$?
check "remove mismatched exits 0" "0" "$code"
contains "remove mismatched: prints success" "Removed" "$out"
check "remove mismatched: worktree dir deleted" "false" \
  "$([ -d "$TEST_WT_DIR" ] && echo true || echo false)"

# Cleanup branch
git -C "$REPO_ROOT" branch -D "$TEST_WT_BRANCH" &>/dev/null || true

# remove with nonexistent branch — should fail gracefully
out=$("$SCRIPT" remove "no-such-branch-$$" 2>&1); code=$?
check "remove nonexistent branch exits non-zero" "1" "$code"
contains "remove nonexistent branch: prints error" "no worktree found" "$out"

# remove inside Zellij: closes the worktree's tab so the sidebar doesn't show
# it as an orphaned "user tab"
TEST_WT_INSIDE_BRANCH="test-inside-$$"
TEST_WT_INSIDE_SESSION="${TEST_WT_INSIDE_BRANCH//\//-}"
TEST_WT_INSIDE_DIR="$HOME/.zelligent/worktrees/$REPO_NAME/$TEST_WT_INSIDE_BRANCH"
register_cleanup_worktree "$TEST_WT_INSIDE_DIR" "$TEST_WT_INSIDE_BRANCH"
git -C "$REPO_ROOT" worktree add -b "$TEST_WT_INSIDE_BRANCH" "$TEST_WT_INSIDE_DIR" HEAD &>/dev/null
# Mock zellij records its full argv to a log so we can verify the close
MOCK_BIN_REMOVE=$(mktemp -d)
REMOVE_LOG=$(mktemp)
cat > "$MOCK_BIN_REMOVE/zellij" <<MOCK
#!/bin/bash
echo "zellij \$*" >> "$REMOVE_LOG"
if [ "\$1" = "action" ] && [ "\$2" = "current-tab-info" ]; then
  echo "name: origin-tab"
  echo "id: 1"
fi
MOCK
chmod +x "$MOCK_BIN_REMOVE/zellij"
out=$(ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN_REMOVE:$PATH" "$SCRIPT" remove "$TEST_WT_INSIDE_BRANCH" 2>&1); code=$?
check "remove inside zellij exits 0" "0" "$code"
contains "remove inside zellij: prints success" "Removed" "$out"
ACTIONS=$(cat "$REMOVE_LOG")
contains "remove inside zellij: queries current tab"  "action current-tab-info"                     "$ACTIONS"
contains "remove inside zellij: switches to target"   "action go-to-tab-name $TEST_WT_INSIDE_SESSION" "$ACTIONS"
contains "remove inside zellij: closes the tab"       "action close-tab"                              "$ACTIONS"
contains "remove inside zellij: returns to origin"    "action go-to-tab-name origin-tab"             "$ACTIONS"
excludes "remove inside zellij: no manual-close hint" "Close the '$TEST_WT_INSIDE_SESSION' tab manually" "$out"
git -C "$REPO_ROOT" branch -D "$TEST_WT_INSIDE_BRANCH" &>/dev/null || true
rm -rf "$MOCK_BIN_REMOVE" "$REMOVE_LOG"

# remove inside Zellij with --plugin-driven flag: CLI must SKIP its own
# tab-close action sequence and defer to the plugin (which will emit
# Action::CloseTabAndRefresh). Issue #121.
TEST_WT_PLUGINDRIVEN_BRANCH="test-plugindriven-$$"
TEST_WT_PLUGINDRIVEN_DIR="$HOME/.zelligent/worktrees/$REPO_NAME/$TEST_WT_PLUGINDRIVEN_BRANCH"
register_cleanup_worktree "$TEST_WT_PLUGINDRIVEN_DIR" "$TEST_WT_PLUGINDRIVEN_BRANCH"
git -C "$REPO_ROOT" worktree add -b "$TEST_WT_PLUGINDRIVEN_BRANCH" "$TEST_WT_PLUGINDRIVEN_DIR" HEAD &>/dev/null
MOCK_BIN_PLUGINDRIVEN=$(mktemp -d)
PLUGINDRIVEN_LOG=$(mktemp)
cat > "$MOCK_BIN_PLUGINDRIVEN/zellij" <<MOCK
#!/bin/bash
echo "zellij \$*" >> "$PLUGINDRIVEN_LOG"
exit 0
MOCK
chmod +x "$MOCK_BIN_PLUGINDRIVEN/zellij"
out=$(ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN_PLUGINDRIVEN:$PATH" "$SCRIPT" remove --plugin-driven "$TEST_WT_PLUGINDRIVEN_BRANCH" 2>&1); code=$?
check "remove plugin-driven exits 0" "0" "$code"
contains "remove plugin-driven: prints success" "Removed" "$out"
ACTIONS=$(cat "$PLUGINDRIVEN_LOG")
excludes "remove plugin-driven: skips current-tab-info" "action current-tab-info" "$ACTIONS"
excludes "remove plugin-driven: skips go-to-tab-name"   "action go-to-tab-name"   "$ACTIONS"
excludes "remove plugin-driven: skips close-tab"        "action close-tab"        "$ACTIONS"
excludes "remove plugin-driven: no manual-close hint"   "Close the '$TEST_WT_PLUGINDRIVEN_BRANCH' tab manually" "$out"
git -C "$REPO_ROOT" branch -D "$TEST_WT_PLUGINDRIVEN_BRANCH" &>/dev/null || true
rm -rf "$MOCK_BIN_PLUGINDRIVEN" "$PLUGINDRIVEN_LOG"

# remove with a stray env var ZELLIGENT_PLUGIN_DRIVEN=1 must NOT skip the
# auto-close (the env var is meaningless to the CLI; only the explicit
# --plugin-driven flag matters). Guards against a user accidentally
# exporting the var in their shell. Issue #121 / PR #122 review.
TEST_WT_ENVNOOP_BRANCH="test-envnoop-$$"
TEST_WT_ENVNOOP_DIR="$HOME/.zelligent/worktrees/$REPO_NAME/$TEST_WT_ENVNOOP_BRANCH"
register_cleanup_worktree "$TEST_WT_ENVNOOP_DIR" "$TEST_WT_ENVNOOP_BRANCH"
git -C "$REPO_ROOT" worktree add -b "$TEST_WT_ENVNOOP_BRANCH" "$TEST_WT_ENVNOOP_DIR" HEAD &>/dev/null
MOCK_BIN_ENVNOOP=$(mktemp -d)
ENVNOOP_LOG=$(mktemp)
cat > "$MOCK_BIN_ENVNOOP/zellij" <<MOCK
#!/bin/bash
echo "zellij \$*" >> "$ENVNOOP_LOG"
if [ "\$1" = "action" ] && [ "\$2" = "current-tab-info" ]; then
  echo "name: origin-tab"
fi
exit 0
MOCK
chmod +x "$MOCK_BIN_ENVNOOP/zellij"
out=$(ZELLIJ=1 ZELLIJ_SESSION_NAME=fake ZELLIGENT_PLUGIN_DRIVEN=1 PATH="$MOCK_BIN_ENVNOOP:$PATH" "$SCRIPT" remove "$TEST_WT_ENVNOOP_BRANCH" 2>&1); code=$?
check "remove with stray env var: exits 0" "0" "$code"
ACTIONS=$(cat "$ENVNOOP_LOG")
contains "remove with stray env var: still closes the tab" "action close-tab" "$ACTIONS"
git -C "$REPO_ROOT" branch -D "$TEST_WT_ENVNOOP_BRANCH" &>/dev/null || true
rm -rf "$MOCK_BIN_ENVNOOP" "$ENVNOOP_LOG"

# remove inside Zellij: when go-to-tab-name fails (tab already gone), the
# script should still exit 0 and skip the close-tab call instead of erroring
TEST_WT_TABGONE_BRANCH="test-tabgone-$$"
TEST_WT_TABGONE_DIR="$HOME/.zelligent/worktrees/$REPO_NAME/$TEST_WT_TABGONE_BRANCH"
register_cleanup_worktree "$TEST_WT_TABGONE_DIR" "$TEST_WT_TABGONE_BRANCH"
git -C "$REPO_ROOT" worktree add -b "$TEST_WT_TABGONE_BRANCH" "$TEST_WT_TABGONE_DIR" HEAD &>/dev/null
MOCK_BIN_TABGONE=$(mktemp -d)
TABGONE_LOG=$(mktemp)
cat > "$MOCK_BIN_TABGONE/zellij" <<MOCK
#!/bin/bash
echo "zellij \$*" >> "$TABGONE_LOG"
if [ "\$1" = "action" ] && [ "\$2" = "current-tab-info" ]; then
  echo "name: origin-tab"
  echo "id: 1"
  exit 0
fi
# Simulate the worktree's tab having already been closed externally:
# go-to-tab-name fails when the target tab no longer exists.
if [ "\$1" = "action" ] && [ "\$2" = "go-to-tab-name" ] && [ "\$3" = "$TEST_WT_TABGONE_BRANCH" ]; then
  exit 1
fi
exit 0
MOCK
chmod +x "$MOCK_BIN_TABGONE/zellij"
out=$(ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN_TABGONE:$PATH" "$SCRIPT" remove "$TEST_WT_TABGONE_BRANCH" 2>&1); code=$?
check "remove inside zellij (tab already gone): still exits 0" "0" "$code"
contains "remove inside zellij (tab already gone): prints success" "Removed" "$out"
ACTIONS=$(cat "$TABGONE_LOG")
excludes "remove inside zellij (tab already gone): skips close-tab" "action close-tab" "$ACTIONS"
git -C "$REPO_ROOT" branch -D "$TEST_WT_TABGONE_BRANCH" &>/dev/null || true
rm -rf "$MOCK_BIN_TABGONE" "$TABGONE_LOG"

# remove refuses to act on non-zelligent-managed worktree (safety check)
TEST_WT_UNMANAGED_BRANCH="test-unmanaged-$$"
TEST_WT_UNMANAGED_DIR=$(mktemp -d)
register_cleanup_worktree "$TEST_WT_UNMANAGED_DIR" "$TEST_WT_UNMANAGED_BRANCH"
git -C "$REPO_ROOT" worktree add -b "$TEST_WT_UNMANAGED_BRANCH" "$TEST_WT_UNMANAGED_DIR" HEAD &>/dev/null
out=$("$SCRIPT" remove "$TEST_WT_UNMANAGED_BRANCH" 2>&1); code=$?
check "remove unmanaged worktree exits non-zero" "1" "$code"
contains "remove unmanaged worktree: prints error" "not managed by zelligent" "$out"
check "remove unmanaged worktree: dir still exists" "true" \
  "$([ -d "$TEST_WT_UNMANAGED_DIR" ] && echo true || echo false)"
git -C "$REPO_ROOT" worktree remove --force "$TEST_WT_UNMANAGED_DIR" &>/dev/null || true
git -C "$REPO_ROOT" branch -D "$TEST_WT_UNMANAGED_BRANCH" &>/dev/null || true

# list-branches
out=$("$SCRIPT" list-branches 2>&1); code=$?
check "list-branches exits 0" "0" "$code"
contains "list-branches includes main or master" "main" "$out"

# ── Launch mode selection ─────────────────────────────────────────────────────
echo "Launch mode:"

# Mock zellij + lazygit; cats any file arg so we can inspect the layout
MOCK_BIN=$(mktemp -d)
cat > "$MOCK_BIN/zellij" <<'MOCK'
#!/bin/bash
echo "zellij $*"
for arg in "$@"; do
  if [ -f "$arg" ]; then cat "$arg"; fi
done
MOCK
cat > "$MOCK_BIN/lazygit" <<'MOCK'
#!/bin/bash
MOCK
chmod +x "$MOCK_BIN/zellij" "$MOCK_BIN/lazygit"

# Shared cleanup for worktrees created during launch-mode tests
cleanup_test_branch() {
  git -C "$REPO_ROOT" worktree remove --force \
    "$HOME/.zelligent/worktrees/$REPO_NAME/some-branch" &>/dev/null || true
  git -C "$REPO_ROOT" branch -D some-branch &>/dev/null || true
}

# Inside Zellij: new-tab, no tab wrapper in layout
register_managed_cleanup "$HOME" some-branch
out=$(ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN:$PATH" "$SCRIPT" spawn some-branch 2>&1)
cleanup_test_branch
contains "inside zellij: prints tab message"        "Opening tab"       "$out"
contains "inside zellij: calls action new-tab"      "action new-tab"    "$out"
excludes "inside zellij: layout has no tab wrapper" 'tab name='         "$out"
# Bare-shell agent: pane should be named after the tab/session, not literal "shell"
contains "inside zellij: shell agent pane uses session name" 'pane name="some-branch"' "$out"
excludes "inside zellij: shell agent pane is not 'shell'"    'pane name="shell"'       "$out"

# #167: the invalidate pipe is fire-and-forget. `zellij pipe` blocks until a
# plugin consumes the message (~1s with sidebars, forever with none), so
# spawn must NOT wait on it. Mock zellij's pipe subcommand to hang for 30s
# and assert spawn still returns promptly, then that the pipe was actually
# fired (its argv lands in the log asynchronously).
MOCK_BIN_HANGPIPE=$(mktemp -d)
HANGPIPE_LOG=$(mktemp)
cat > "$MOCK_BIN_HANGPIPE/zellij" <<MOCK
#!/bin/bash
echo "zellij \$*" >> "$HANGPIPE_LOG"
if [ "\$1" = "pipe" ]; then sleep 30; fi
exit 0
MOCK
cp "$MOCK_BIN/lazygit" "$MOCK_BIN_HANGPIPE/lazygit"
chmod +x "$MOCK_BIN_HANGPIPE/zellij"
register_managed_cleanup "$HOME" some-branch
SPAWN_START=$(date +%s)
out=$(ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN_HANGPIPE:$PATH" "$SCRIPT" spawn some-branch 2>&1); code=$?
SPAWN_ELAPSED=$(( $(date +%s) - SPAWN_START ))
cleanup_test_branch
check "async pipe: spawn exits 0 despite hanging pipe" "0" "$code"
if [ "$SPAWN_ELAPSED" -lt 10 ]; then
  pass "async pipe: spawn returns promptly (${SPAWN_ELAPSED}s) while the pipe hangs"
else
  fail "async pipe: spawn returns promptly while the pipe hangs" "<10s" "${SPAWN_ELAPSED}s"
fi
PIPE_SEEN=""
for _ in 1 2 3 4 5 6 7 8 9 10; do
  if grep -q "pipe --name zelligent-invalidate" "$HANGPIPE_LOG" 2>/dev/null; then PIPE_SEEN=1; break; fi
  sleep 0.2
done
if [ -n "$PIPE_SEEN" ]; then
  pass "async pipe: invalidate pipe was still fired (async)"
else
  fail "async pipe: invalidate pipe was still fired (async)" "pipe --name zelligent-invalidate in mock log" "absent after 2s"
fi
rm -rf "$MOCK_BIN_HANGPIPE" "$HANGPIPE_LOG"

# Regression: a non-shell agent command must still drive the pane name
# (the `*) echo "$base" ;;` arm in pane_name_for_agent_cmd). With agent_cmd
# `claude`, the pane should be named "claude", not "some-branch" or "shell".
register_managed_cleanup "$HOME" some-branch
out_claude=$(ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN:$PATH" "$SCRIPT" spawn some-branch claude 2>&1)
cleanup_test_branch
contains "inside zellij: claude agent pane uses 'claude'"          'pane name="claude"'       "$out_claude"
excludes "inside zellij: claude agent pane is not the session"     'pane name="some-branch"'  "$out_claude"
excludes "inside zellij: claude agent pane is not 'shell'"         'pane name="shell"'        "$out_claude"

# pane_name_for_agent_cmd unit tests: source the function from zelligent.sh
# and exercise the empty-agent-cmd branches directly. Codex review flagged
# that the end-to-end tests can't reach the `[ -z "$agent_cmd" ]` arm,
# since `spawn` defaults agent_cmd to `$SHELL`.
PANE_NAME_FN=$(
  awk '/^pane_name_for_agent_cmd\(\) \{/,/^\}$/' "$SCRIPT"
)
result=$(bash -c "$PANE_NAME_FN; pane_name_for_agent_cmd '' ''" 2>&1)
check "pane_name fn: empty agent+empty session falls back to 'shell'" "shell" "$result"
result=$(bash -c "$PANE_NAME_FN; pane_name_for_agent_cmd '' 'my-branch'" 2>&1)
check "pane_name fn: empty agent+session falls back to session name"  "my-branch" "$result"
result=$(bash -c "$PANE_NAME_FN; pane_name_for_agent_cmd 'bash' ''" 2>&1)
check "pane_name fn: bash agent+empty session falls back to 'shell'"  "shell" "$result"
result=$(bash -c "$PANE_NAME_FN; pane_name_for_agent_cmd 'bash' 'my-branch'" 2>&1)
check "pane_name fn: bash agent+session falls back to session name"   "my-branch" "$result"
result=$(bash -c "$PANE_NAME_FN; pane_name_for_agent_cmd 'claude --foo' 'my-branch'" 2>&1)
check "pane_name fn: non-shell agent always wins over session name"   "claude" "$result"

# Outside Zellij, no existing repo session: create session named after repo
register_managed_cleanup "$HOME" some-branch
out=$(ZELLIJ="" ZELLIJ_SESSION_NAME="" PATH="$MOCK_BIN:$PATH" "$SCRIPT" spawn some-branch 2>&1)
cleanup_test_branch
contains "outside zellij (new): prints session message"          "Creating Zellij session"            "$out"
contains "outside zellij (new): session named after repo"        "$REPO_NAME"                         "$out"
contains "outside zellij (new): calls --new-session-with-layout" "zellij --new-session-with-layout"   "$out"
contains "outside zellij (new): sets default tab template"       "default_tab_template"               "$out"
contains "outside zellij (new): layout has tab wrapper"          'tab name="some-branch"'             "$out"
contains "outside zellij (new): layout names sidebar pane"       'pane name="zelligent"'             "$out"
# #163: the explicit `tab { }` body must be content-only — a third sidebar
# pane there gets merged INTO the template's children slot and renders a
# nested duplicate sidebar. Exactly two: default_tab_template + new_tab_template.
count_equals "outside zellij (new): exactly two sidebar panes (templates only, none in tab body)" 'pane name="zelligent"' 2 "$out"
# #139: `default_tab_template`'s {{zelligent_children}} ("children" keyword)
# is only filled in by Zellij when merging an EXPLICIT tab body into the
# template — which is what makes the `tab name="some-branch"` block above
# render correctly. A tab created later via `zellij action new-tab --name X`
# with no --layout has no explicit body to merge, and Zellij's fill for that
# path does not recurse into nested panes to find the marker, so it silently
# resolves to nothing (sidebar only, no shell pane). `new_tab_template` is a
# separate KDL node — parsed like a literal tab, no children-marker merge —
# that Zellij prefers over `default_tab_template` for exactly that case, so
# give it real sidebar+shell+lazygit content instead of a "children" marker.
contains "outside zellij (new): sets new tab template"           "new_tab_template"                    "$out"
contains "outside zellij (new): new tab template has sidebar"    'plugin location="file:'              "$out"
contains "outside zellij (new): new tab template has shell pane" 'pane name="shell"'                   "$out"
contains "outside zellij (new): new tab template has lazygit"    'command="lazygit"'                   "$out"

cat > "$TEST_REPO_LAYOUT" <<'KDL'
// leading comment before outer layout
pane split_direction="Vertical" {
    pane size="33%" {
        {{zelligent_sidebar}}
    }
    {{zelligent_children}}
}
// trailing comment after outer layout
KDL
register_managed_cleanup "$HOME" some-branch
out=$(ZELLIJ="" ZELLIJ_SESSION_NAME="" PATH="$MOCK_BIN:$PATH" "$SCRIPT" spawn some-branch 2>&1)
cleanup_test_branch
contains "outside zellij (new): commented layout still parses" "zellij --new-session-with-layout" "$out"
contains "outside zellij (new): commented layout keeps tab wrapper" 'tab name="some-branch"' "$out"
# 3, not 2: initial tab's rendered fragment + default_tab_template +
# new_tab_template (#139) each carry their own copy of the custom-width
# sidebar pane.
count_equals "outside zellij (new): custom sidebar width reaches default template and new tab template (initial tab inherits it, #163)" 'size="33%"' 2 "$out"
cp "$ZELLIGENT_DEFAULT_LAYOUT_SRC" "$TEST_REPO_LAYOUT"

# Outside Zellij, repo session already exists: add tab and attach
MOCK_BIN2=$(mktemp -d)
cat > "$MOCK_BIN2/zellij" <<MOCK2
#!/bin/bash
if [ "\$1" = "list-sessions" ]; then echo "$REPO_NAME"; fi
echo "zellij \$*"
for arg in "\$@"; do
  if [ -f "\$arg" ] && [[ "\$arg" == *.kdl ]]; then cat "\$arg"; fi
done
MOCK2
cat > "$MOCK_BIN2/lazygit" <<'MOCK'
#!/bin/bash
MOCK
chmod +x "$MOCK_BIN2/zellij" "$MOCK_BIN2/lazygit"

register_managed_cleanup "$HOME" some-branch
out=$(ZELLIJ="" ZELLIJ_SESSION_NAME="" PATH="$MOCK_BIN2:$PATH" "$SCRIPT" spawn some-branch 2>&1)
cleanup_test_branch
contains "outside zellij (existing): attaches to repo session" "Attaching to session '$REPO_NAME'" "$out"
contains "outside zellij (existing): calls action new-tab"     "action new-tab"                   "$out"
contains "outside zellij (existing): calls attach"             "zellij attach $REPO_NAME"         "$out"
# new-tab --layout expects a fragment (panes at root), not a full session
# layout. Feeding it a session layout grafted the sidebar pane into the
# existing tab (visible as duplicated sidebars in the UI).
excludes "outside zellij (existing): layout is a fragment, no default_tab_template" "default_tab_template" "$out"
excludes "outside zellij (existing): layout is a fragment, no tab wrapper"          'tab name='           "$out"

# TTY guard: outside Zellij, with TTY check enabled, spawn should refuse and
# exit nonzero with a friendly error rather than panicking inside zellij.
out=$(ZELLIJ="" ZELLIJ_SESSION_NAME="" ZELLIGENT_SKIP_TTY_CHECK="" PATH="$MOCK_BIN2:$PATH" "$SCRIPT" spawn some-branch 2>&1); code=$?
cleanup_test_branch
check    "tty guard: refuses spawn outside zellij without a tty" "1" "$code"
contains "tty guard: prints friendly error" "must run from a TTY" "$out"

rm -rf "$MOCK_BIN" "$MOCK_BIN2"

# ── Integration: layout loading via background session ────────────────────────
echo "Integration (requires Zellij):"

if ! command -v zellij &>/dev/null; then
  echo "  ⚠️  Zellij not found, skipping integration tests"
else
  TEST_SESSION="zelligent-test-$$"
  # A leftover serialized session from an aborted prior run (same PID, or a
  # PID reused since) can resurrect here instead of creating fresh — belt
  # and braces determinism fix noted in the #155 design doc (Phase 3).
  zellij delete-session --force "$TEST_SESSION" 2>/dev/null || true
  zellij attach --create-background "$TEST_SESSION" 2>/dev/null

  # Mock lazygit so the script can pass the dependency check
  MOCK_BIN_INT=$(mktemp -d)
  cat > "$MOCK_BIN_INT/lazygit" <<'MOCK'
#!/bin/bash
MOCK
  chmod +x "$MOCK_BIN_INT/lazygit"

  # Real Zellij (unlike the unit tests' mock) actually loads the sidebar
  # plugin, so the top-of-file ZELLIGENT_PLUGIN_SRC=$SCRIPT fallback — a shell
  # script — fails wasm validation ("magic header not detected") and makes
  # `zellij action new-tab` exit 2. Point at a real wasm when one is available.
  INT_PLUGIN_WASM="$REPO_ROOT/plugin/target/wasm32-wasip1/release/zelligent-plugin.wasm"
  if [ ! -f "$INT_PLUGIN_WASM" ] && command -v brew &>/dev/null; then
    INT_PLUGIN_WASM="$(brew --prefix 2>/dev/null)/share/zelligent/zelligent-plugin.wasm"
  fi
  if [ -f "$INT_PLUGIN_WASM" ]; then
    INT_PLUGIN_SRC="$INT_PLUGIN_WASM"
  else
    # No wasm anywhere: keep the script fallback so the tab/layout checks
    # below still run; the plugin-load failure makes new-tab exit 2, so the
    # exit-code assertion is skipped rather than asserted wrong.
    INT_PLUGIN_SRC="$ZELLIGENT_PLUGIN_SRC"
  fi

  # Call the script in inside-Zellij mode; it will call `zellij action new-tab`
  # targeting the background test session via ZELLIJ_SESSION_NAME
  register_managed_cleanup "$HOME" integration-test-branch
  int_out=$(ZELLIGENT_PLUGIN_SRC="$INT_PLUGIN_SRC" \
    ZELLIJ=1 ZELLIJ_SESSION_NAME="$TEST_SESSION" PATH="$MOCK_BIN_INT:$PATH" \
    "$SCRIPT" spawn integration-test-branch 2>&1)
  int_code=$?
  if [ -f "$INT_PLUGIN_WASM" ]; then
    check "script exits 0 (integration)" "0" "$int_code"
  else
    echo "  ⚠️  No plugin wasm found, skipping exit-code check (build one: cd plugin && cargo build --release)"
  fi

  git -C "$REPO_ROOT" worktree remove --force \
    "$HOME/.zelligent/worktrees/$REPO_NAME/integration-test-branch" &>/dev/null || true
  git -C "$REPO_ROOT" branch -D integration-test-branch &>/dev/null || true
  rm -rf "$MOCK_BIN_INT"

  DUMP=$(ZELLIJ_SESSION_NAME="$TEST_SESSION" zellij action dump-layout 2>/dev/null)
  contains "tab appears in session layout" 'tab name="integration-test-branch"' "$DUMP"
  contains "tab has sidebar plugin" 'plugin location="file:' "$DUMP"
  contains "tab has status-bar" 'plugin location="zellij:status-bar"' "$DUMP"
  # L1: sidebar must be a vertical (left/right) split — "vertical" means side-by-side,
  # "horizontal" means top/bottom. Getting this wrong renders the sidebar as a top bar.
  # Note: dump-layout normalizes split_direction values to lowercase.
  contains "L1: dump-layout shows vertical split (sidebar on left)" 'split_direction="vertical"' "$DUMP"
  not_contains "L1: dump-layout has no horizontal split (no top bar)" 'split_direction="horizontal"' "$DUMP"

  zellij kill-session "$TEST_SESSION" 2>/dev/null
fi

# ── Doc index completeness ─────────────────────────────────────────────────
echo "Doc index completeness:"

INDEX_CONTENT=$(cat "$REPO_ROOT/docs/design-docs/index.md")
for doc in "$REPO_ROOT"/docs/design-docs/*.md; do
  docname=$(basename "$doc")
  [ "$docname" = "index.md" ] && continue
  contains "index references $docname" "$docname" "$INDEX_CONTENT"
done

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "Results: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
