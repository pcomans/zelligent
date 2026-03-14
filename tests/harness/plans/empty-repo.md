---
fixture: setup-empty-repo.sh
launch: ZELLIGENT_PLUGIN_SRC="$HOME/.local/share/zelligent/zelligent-plugin.wasm" ./zelligent.sh
session_name: zelligent-test-repo
---

# Empty Repo Sidebar Smoke Test

Verifies PR 83 startup behavior in a git repo with no managed worktrees.

## Test 1: `zelligent` starts a sidebar session
- Action: Wait for the `launch: zelligent` command to finish rendering
- Expected: A Zellij session appears with a persistent left sidebar, not a floating pane

## Test 2: Initial tab uses the default two-pane body
- Action: Read the terminal buffer
- Expected: The main area shows the repo tab body with a shell pane and a lazygit pane to the right of the sidebar

## Test 3: Empty-state sidebar is visible
- Action: Read the sidebar buffer
- Expected: The sidebar renders the zelligent UI without requiring `Ctrl-y`

## Test 4: Version is displayed
- Action: Read the terminal buffer
- Expected: A semantic version string (e.g., "0.1.14") appears somewhere in the plugin UI

## Test 5: Manual tabs inherit the sidebar frame
- Action: Press `Ctrl-t`, then `n`, type `scratch`, and press Enter
- Expected: The new manual tab still shows the persistent left sidebar pane
