#!/bin/bash

# Test suite for zelligent.sh
# Unit tests run anywhere. Integration tests require Zellij to be installed.

PASS=0
FAIL=0
SCRIPT="$(cd "$(dirname "$0")" && pwd)/zelligent.sh"
GIT_COMMON_DIR="$(git -C "$(dirname "$0")" rev-parse --path-format=absolute --git-common-dir)"
REPO_ROOT="${GIT_COMMON_DIR%/.git}"
REPO_NAME="$(basename "$REPO_ROOT")"

pass() { echo "  ✅ $1"; ((PASS++)); }
fail() { echo "  ❌ $1"; ((FAIL++)); }

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
  if echo "$haystack" | grep -qF "$needle"; then
    pass "$desc"
  else
    fail "$desc (expected to contain: '$needle')"
  fi
}

excludes() {
  local desc="$1" needle="$2" haystack="$3"
  if echo "$haystack" | grep -qF "$needle"; then
    fail "$desc (must not contain: '$needle')"
  else
    pass "$desc"
  fi
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
contains "layout contains tab-bar"        'zellij:tab-bar'            "$out"
contains "layout contains status-bar"     'zellij:status-bar'         "$out"
excludes "inside zellij layout: no tab{} wrapper" 'tab name='        "$out"
contains "new worktree: setup.sh runs as preamble" 'setup.sh'        "$out"
contains "new worktree: agent starts via exec"     'exec claude'     "$out"

# Test: existing worktree should NOT include setup.sh preamble
# Re-create the worktree so it already exists, then run the script again
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
trap restore_setup EXIT INT TERM
mv "$SETUP_SH" "$SETUP_SH_BAK"
out_no_setup=$(ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN_LAYOUT:$PATH" \
  "$SCRIPT" spawn test-no-setup-branch claude 2>&1)
restore_setup
git -C "$REPO_ROOT" worktree remove --force \
  "$HOME/.zelligent/worktrees/$REPO_NAME/test-no-setup-branch" &>/dev/null || true
git -C "$REPO_ROOT" branch -D test-no-setup-branch &>/dev/null || true

contains "no setup.sh: uses direct command"  'exec claude' "$out_no_setup"
excludes "no setup.sh: no setup preamble"    'setup.sh'         "$out_no_setup"

# Test: multi-word AGENT_CMD with arguments
out_multi=$(ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN_LAYOUT:$PATH" \
  "$SCRIPT" spawn test-multi-cmd-branch 'claude "pls fix the bug" --model claude-sonnet-4-6' 2>&1)
git -C "$REPO_ROOT" worktree remove --force \
  "$HOME/.zelligent/worktrees/$REPO_NAME/test-multi-cmd-branch" &>/dev/null || true
git -C "$REPO_ROOT" branch -D test-multi-cmd-branch &>/dev/null || true

contains "multi-word cmd: contains full command" 'claude \"pls fix the bug\" --model claude-sonnet-4-6' "$out_multi"
contains "multi-word cmd: contains exec"         'exec claude'   "$out_multi"
contains "multi-word cmd: contains model flag"   'claude-sonnet-4-6' "$out_multi"

rm -rf "$MOCK_BIN_LAYOUT"

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

out=$(ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN_QUOTE:$PATH" \
  "$SCRIPT" spawn test-quoted-branch 'claude -p "Sag Hallo auf Deutsch"' 2>&1)
git -C "$REPO_ROOT" worktree remove --force \
  "$HOME/.zelligent/worktrees/$REPO_NAME/test-quoted-branch" &>/dev/null || true
git -C "$REPO_ROOT" branch -D test-quoted-branch &>/dev/null || true

contains "quoted cmd: quotes are escaped" 'exec claude -p \"Sag Hallo auf Deutsch\"' "$out"

rm -rf "$MOCK_BIN_QUOTE"

# ── Argument validation ────────────────────────────────────────────────────────
echo "Argument validation:"

out=$("$SCRIPT" 2>&1); code=$?
check "no args exits non-zero" "1" "$code"
contains "no args prints usage" "Usage:" "$out"

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
out=$(ZELLIJ=1 ZELLIJ_SESSION_NAME=fake PATH="$MOCK_BIN:$PATH" "$SCRIPT" spawn some-branch 2>&1)
cleanup_test_branch
contains "inside zellij: prints tab message"        "Opening tab"       "$out"
contains "inside zellij: calls action new-tab"      "action new-tab"    "$out"
excludes "inside zellij: layout has no tab wrapper" 'tab name='         "$out"

# Outside Zellij, no existing repo session: create session named after repo
out=$(ZELLIJ="" ZELLIJ_SESSION_NAME="" PATH="$MOCK_BIN:$PATH" "$SCRIPT" spawn some-branch 2>&1)
cleanup_test_branch
contains "outside zellij (new): prints session message"          "Creating Zellij session"            "$out"
contains "outside zellij (new): session named after repo"        "$REPO_NAME"                         "$out"
contains "outside zellij (new): calls --new-session-with-layout" "zellij --new-session-with-layout"   "$out"
contains "outside zellij (new): layout has tab wrapper"          'tab name="some-branch"'             "$out"

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
  contains "tab has tab-bar"    'plugin location="zellij:tab-bar"'    "$DUMP"
  contains "tab has status-bar" 'plugin location="zellij:status-bar"' "$DUMP"

  zellij kill-session "$TEST_SESSION" 2>/dev/null
fi

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "Results: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
