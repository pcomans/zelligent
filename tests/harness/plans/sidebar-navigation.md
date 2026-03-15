---
fixture: setup-with-worktrees.sh
launch: ZELLIGENT_PLUGIN_SRC="$HOME/.local/share/zelligent/zelligent-plugin.wasm" ./zelligent.sh
session_name: zelligent-test-repo
---

# Sidebar Navigation Test

Verifies keyboard navigation, browse-mode transitions, and mixed tab identity in
the persistent sidebar. The fixture creates three managed worktrees:
`feature-a`, `feature-b`, and `feature-c`.

## Test 1: Startup shows the seeded sidebar rows
- Action: Wait for the `launch: zelligent` command to finish rendering
- Expected: The sidebar shows the local row plus rows for `feature-a`, `feature-b`, and `feature-c`

## Test 2: `j` moves selection down
- Action: Press `j`
- Expected: The sidebar selection moves from the local row to the next visible item

## Test 3: `j` again reaches the next managed worktree
- Action: Press `j` again
- Expected: The selected row advances again without closing or hiding the sidebar

## Test 4: `k` moves selection back up
- Action: Press `k`
- Expected: The selected row moves back to the previous item

## Test 5: `Enter` switches to the selected row
- Action: With a managed worktree row selected, press `Enter`
- Expected: The active tab switches to that row's tab and the persistent sidebar remains visible

## Test 6: `n` opens branch-picker mode
- Action: Press `n`
- Expected: The sidebar switches to branch selection mode and shows a branch list

## Test 7: `Esc` exits branch-picker mode
- Action: Press `Esc`
- Expected: The sidebar returns to browse mode with the worktree list visible

## Test 8: `i` opens input mode
- Action: Press `i`
- Expected: The sidebar shows the new-branch input prompt

## Test 9: Typing updates the input buffer
- Action: Type `test-input`
- Expected: The input prompt displays `test-input`

## Test 10: `Esc` exits input mode and clears the prompt
- Action: Press `Esc`
- Expected: The sidebar returns to browse mode instead of keeping the typed prompt on screen

## Test 11: Manual tabs are labeled as user tabs
- Action: In the control window, run `ZELLIJ=1 ZELLIJ_SESSION_NAME=zelligent-test-repo zellij action new-tab --name notes`
- Expected: The sidebar gains a `notes` row that is shown as a user tab rather than a managed worktree

## Test 12: `Enter` opens the selected manual tab
- Action: Select the `notes` row and press `Enter`
- Expected: Focus switches to the `notes` tab and the sidebar remains visible

## Test 13: `q` and browse-mode `Esc` do not close the sidebar
- Action: Return to browse mode if needed, then press `q` and `Esc`
- Expected: The sidebar stays visible and usable after both keys
