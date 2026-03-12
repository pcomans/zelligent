---
fixture: setup-with-worktrees.sh
launch: ZELLIGENT_PLUGIN_SRC="$HOME/.local/share/zelligent/zelligent-plugin.wasm" ./zelligent.sh
session_name: zelligent-test-repo
---

# Sidebar Worktree Listing Test

Verifies the embedded sidebar renders seeded worktrees inside a `zelligent`
session. The fixture creates three worktrees: `feature-a`, `feature-b`, and
`feature-c`.

## Test 1: Session starts with the embedded sidebar visible
- Action: Wait for the `launch: zelligent` command to finish
- Expected: The left sidebar is visible immediately, without pressing `Ctrl-y`

## Test 2: Sidebar shows seeded worktrees
- Action: Read the sidebar buffer
- Expected: The sidebar list includes `feature-a`, `feature-b`, and `feature-c`

## Test 3: Refresh keeps the list visible
- Action: Press `r`
- Expected: The sidebar refreshes and the same three worktrees remain visible

## Test 4: Spawned tabs keep the sidebar frame
- Action: In the control window, run `ZELLIJ=1 ZELLIJ_SESSION_NAME=zelligent-test-repo ZELLIGENT_PLUGIN_SRC="$HOME/.local/share/zelligent/zelligent-plugin.wasm" ./zelligent.sh spawn feature-d bash`
- Expected: The new tab appears in the live session and still renders the left sidebar

## Test 5: Version is displayed
- Action: Read the terminal buffer
- Expected: A semantic version string (e.g., "0.1.14") appears in the plugin UI
