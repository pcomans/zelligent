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
- Expected: The sidebar shows the startup row `zelligent-test-repo` plus rows for `feature-a`, `feature-b`, and `feature-c`

## Test 2: Wheel-down moves selection to the next row
- Action: Send a wheel-down mouse event to the sidebar pane. Scroll events are pane-wide, so any in-sidebar coordinate is fine; for example, column 10, row 5
- Expected: The selected row moves from the startup row `zelligent-test-repo` to `feature-a`

## Test 3: Wheel-up moves selection back
- Action: Send a wheel-up mouse event inside the sidebar content area
- Expected: The selected row returns to the startup row `zelligent-test-repo`

## Test 4: First click selects a detached worktree row
- Action: Left-click on the `feature-b` title line in the sidebar. In a standard 220x60 harness capture this is around column 10, row 9; otherwise, use the current capture to target the `feature-b` row directly
- Expected: `feature-b` becomes the selected row, but the active tab remains `zelligent-test-repo`

## Test 5: Second click on the selected row opens it
- Action: Left-click the same `feature-b` row again
- Expected: The session opens a `feature-b` tab and the persistent sidebar remains visible. If `feature-b` was still detached, this click spawns it; if it was already open, this click switches to it

## Test 6: Clicking a non-item line does nothing
- Action: Left-click a blank sidebar line below the visible rows or in the footer area. In a standard 220x60 harness capture, column 10, row 12 is one such blank line
- Expected: The current selection and active tab stay unchanged
