---
fixture: setup-empty-repo.sh
---

# Empty Repo Smoke Test

Verifies the plugin works correctly in a git repo with no worktrees.

## Test 1: Plugin opens on Ctrl+Y
- Action: Press Ctrl+Y
- Expected: A floating pane appears showing the zelligent plugin with the Zelligent logo (empty state) and navigation hints at the bottom

## Test 2: Plugin closes on q
- Action: Press q
- Expected: The floating plugin pane closes, back to shell prompt

## Test 3: Plugin reopens
- Action: Press Ctrl+Y again
- Expected: The plugin opens again showing the same empty state

## Test 4: Version is displayed
- Action: Read the terminal buffer
- Expected: The version string "0.1.14" appears somewhere in the plugin UI

## Test 5: Clean close
- Action: Press q to close the plugin
- Expected: Back to shell prompt, no errors visible
