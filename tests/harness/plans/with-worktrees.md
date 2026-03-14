---
fixture: setup-with-worktrees.sh
launch: ZELLIGENT_PLUGIN_SRC="$HOME/.local/share/zelligent/zelligent-plugin.wasm" ./zelligent.sh
session_name: zelligent-test-repo
---

# Sidebar Worktree Listing Test

Verifies the embedded sidebar stays stable in a repo that already has seeded
worktrees. The fixture creates three worktrees: `feature-a`, `feature-b`, and
`feature-c`.

## Test 1: Session starts with the embedded sidebar visible
- Action: Wait for the `launch: zelligent` command to finish
- Expected: The left sidebar is visible immediately, without pressing `Ctrl-y`

## Test 2: Fixture seeded the repo with managed worktrees
- Action: In the control window, run `git worktree list`
- Expected: The output includes `feature-a`, `feature-b`, and `feature-c`

## Test 3: Refresh keeps the sidebar stable
- Action: Press `r`
- Expected: The sidebar remains visible and responsive after refresh

## Test 4: Spawned tabs keep the sidebar frame
- Action: In the control window, run `ZELLIJ=1 ZELLIJ_SESSION_NAME=zelligent-test-repo ZELLIGENT_PLUGIN_SRC="$HOME/.local/share/zelligent/zelligent-plugin.wasm" ./zelligent.sh spawn feature-d bash`
- Expected: The new `feature-d` tab appears in the live session and still renders the left sidebar

## Test 5: Version is displayed
- Action: Read the terminal buffer
- Expected: A semantic version string (e.g., "0.1.14") appears in the plugin UI
