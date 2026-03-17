---
fixture: setup-with-worktrees.sh
launch: ZELLIGENT_PLUGIN_SRC="$HOME/.local/share/zelligent/zelligent-plugin.wasm" ./zelligent.sh
session_name: zelligent-test-repo
---

# Sidebar Mouse Interaction Test

Verifies wheel navigation and click behavior in the persistent sidebar.

Use ANSI-aware capture for this plan so selected rows remain distinguishable.
Use tmux SGR mouse sequences, as documented in `.claude/skills/tmux/SKILL.md`,
to send wheel and left-click events into the `view` window.

## Test 1: Startup shows the seeded sidebar rows
- Action: Wait for the `launch: zelligent` command to finish rendering
- Expected: The sidebar shows the local row plus rows for `feature-a`, `feature-b`, and `feature-c`

## Test 2: Wheel-down moves selection to the next row
- Action: Send a wheel-down mouse event inside the sidebar content area, for example at column 10, row 5
- Expected: The selected row moves from the local row to `feature-a`

## Test 3: Wheel-up moves selection back
- Action: Send a wheel-up mouse event inside the sidebar content area
- Expected: The selected row returns to the local row

## Test 4: First click selects a detached worktree row
- Action: Left-click on the `feature-b` row in the sidebar
- Expected: `feature-b` becomes the selected row, but the active tab does not change yet

## Test 5: Second click on the selected row opens it
- Action: Left-click the same `feature-b` row again
- Expected: The session switches to a `feature-b` tab and the persistent sidebar remains visible

## Test 6: Clicking a non-item line does nothing
- Action: Left-click a blank sidebar line below the visible rows or in the footer area
- Expected: The current selection and active tab stay unchanged
