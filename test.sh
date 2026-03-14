#!/bin/bash

# Test suite for zelligent.sh
# Unit tests run anywhere. Integration tests require Zellij to be installed.

PASS=0
FAIL=0
CLEANUP_WORKTREE_PATHS=()
CLEANUP_WORKTREE_BRANCHES=()
SCRIPT="$(cd "$(dirname "$0")" && pwd)/zelligent.sh"
GIT_COMMON_DIR="$(git -C "$(dirname "$0")" rev-parse --path-format=absolute --git-common-dir)"
REPO_ROOT="${GIT_COMMON_DIR%/.git}"
REPO_NAME="$(basename "$REPO_ROOT")"
# Spawn now requires a resolvable plugin path and layout asset.
export ZELLIGENT_PLUGIN_SRC="${ZELLIGENT_PLUGIN_SRC:-$SCRIPT}"
export ZELLIGENT_DEFAULT_LAYOUT_SRC="${ZELLIGENT_DEFAULT_LAYOUT_SRC:-$REPO_ROOT/share/default-layout.kdl}"

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
Replaces 'exec claude' with 'claude' so the mock binary can return.
"""
import re, subprocess, sys

layout_file = sys.argv[1]
with open(layout_file) as f:
    for line in f:
        stripped = line.strip()
        if stripped.startswith("args "):
            tokens = re.findall(r'"((?:[^"\\]|\\.)*)"', stripped)
            args = [t.replace('\\"', '"').replace('\\\\', '\\') for t in tokens]
            args = [a.replace('exec claude', 'claude') for a in args]
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
out=$(HOME="$MOCK_NOARGS_HOME_NONE" ZELLIGENT_PLUGIN_SRC="" PATH="$MOCK_NOARGS_BIN_NONE:/usr/bin:/bin" "$SCRIPT" 2>&1); code=$?
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
contains "no args with plugin: layout has status-bar" 'plugin location="zellij:status-bar"' "$out"
count_equals "no args with plugin: session layout has one shared sidebar split" 'split_direction="Vertical"' 1 "$out"
contains "no args with plugin: startup honors SHELL" 'agent_cmd "/bin/zsh"' "$out"
contains "no args with plugin: startup shell reaches layout args" 'exec /bin/zsh' "$out"

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

# nuke from non-git dir: exits non-zero
NONGIT_NUKE=$(mktemp -d)
out=$(cd "$NONGIT_NUKE" && "$SCRIPT" nuke 2>&1); code=$?
check "nuke non-git dir exits non-zero" "1" "$code"
rm -rf "$NONGIT_NUKE"

# --help lists nuke
out=$("$SCRIPT" --help 2>&1)
contains "--help lists nuke" "nuke" "$out"

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
contains "doctor idempotent: keybinding skipped" "keybinding: skipped (persistent sidebar only)" "$out2"
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

rm -rf "$MOCK_DR_BIN2" "$MOCK_DR_HOME2" "$FAKE_WASM_DIR2"

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

# ── Install script contract ──────────────────────────────────────────────────
echo "Install script contract:"

DEV_INSTALL_CONTENT=$(cat "$REPO_ROOT/dev-install.sh")
contains "dev-install copies default layout asset" 'default-layout.kdl' "$DEV_INSTALL_CONTENT"
contains "default layout asset exists in repo" '{{zelligent_sidebar}}' "$(cat "$REPO_ROOT/share/default-layout.kdl")"
contains "default layout asset contains children placeholder" '{{zelligent_children}}' "$(cat "$REPO_ROOT/share/default-layout.kdl")"

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

# Outside Zellij, no existing repo session: create session named after repo
register_managed_cleanup "$HOME" some-branch
out=$(ZELLIJ="" ZELLIJ_SESSION_NAME="" PATH="$MOCK_BIN:$PATH" "$SCRIPT" spawn some-branch 2>&1)
cleanup_test_branch
contains "outside zellij (new): prints session message"          "Creating Zellij session"            "$out"
contains "outside zellij (new): session named after repo"        "$REPO_NAME"                         "$out"
contains "outside zellij (new): calls --new-session-with-layout" "zellij --new-session-with-layout"   "$out"
contains "outside zellij (new): sets default tab template"       "default_tab_template"               "$out"
contains "outside zellij (new): layout has tab wrapper"          'tab name="some-branch"'             "$out"

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
count_equals "outside zellij (new): custom sidebar width reaches initial tab and default template" 'size="33%"' 2 "$out"
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

rm -rf "$MOCK_BIN" "$MOCK_BIN2"

# ── Integration: layout loading via background session ────────────────────────
echo "Integration (requires Zellij):"

if ! command -v zellij &>/dev/null; then
  echo "  ⚠️  Zellij not found, skipping integration tests"
else
  TEST_SESSION="zelligent-test-$$"
  zellij attach --create-background "$TEST_SESSION" 2>/dev/null

  # Mock lazygit so the script can pass the dependency check
  MOCK_BIN_INT=$(mktemp -d)
  cat > "$MOCK_BIN_INT/lazygit" <<'MOCK'
#!/bin/bash
MOCK
  chmod +x "$MOCK_BIN_INT/lazygit"

  # Call the script in inside-Zellij mode; it will call `zellij action new-tab`
  # targeting the background test session via ZELLIJ_SESSION_NAME
  register_managed_cleanup "$HOME" integration-test-branch
  int_out=$(ZELLIJ=1 ZELLIJ_SESSION_NAME="$TEST_SESSION" PATH="$MOCK_BIN_INT:$PATH" \
    "$SCRIPT" spawn integration-test-branch 2>&1)
  int_code=$?
  check "script exits 0 (integration)" "0" "$int_code"

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
