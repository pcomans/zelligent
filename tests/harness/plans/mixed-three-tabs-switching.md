---
fixture: setup-with-worktrees.sh
---

# Mixed Three-Tab Switching Regression Plan

Validates sidebar behavior when a session contains exactly 3 relevant tabs:
- 2 zelligent-managed worktree tabs
- 1 manual user-created tab (non-worktree)

This plan is focused on switching correctness and mixed tab identity.

## Test 1: Create two worktree tabs
- Action: Run `zelligent spawn feature-a bash`, then `zelligent spawn feature-b bash`.
- Expected: Two sidebar rows are present for worktree tabs, each with second-line `branch: ...` subtitles.

## Test 2: Create one manual tab
- Action: Create a non-worktree tab using Zellij directly (eg. `zellij action new-tab --name manual-notes`).
- Expected: Sidebar shows a `manual-notes` row with second-line subtitle `user tab` (not `branch:`).

## Test 3: Confirm mixed identity is stable
- Action: Refresh once with `r`.
- Expected: Sidebar still distinguishes rows as:
  - `feature-a` -> `branch: feature-a`
  - `feature-b` -> `branch: feature-b`
  - `manual-notes` -> `user tab`

## Test 4: Keyboard switching across all three tabs
- Action: Starting from first row, use `j`/`k` to select each of the other two rows and press `Enter` after each selection.
- Expected: Focus switches to the selected tab every time and sidebar remains visible after each switch.

## Test 5: Repeat switching in reverse order
- Action: Navigate back through the same three rows in reverse order with `k` + `Enter`.
- Expected: Reverse-direction switching is correct with no off-by-one/tab-index mismatch.

## Test 6: Manual-tab delete guardrail
- Action: Select `manual-notes` and press `d`.
- Expected: No destructive action is run; UI shows an error/status message indicating only worktree tabs are removable.

## Test 7: Worktree delete behavior still works
- Action: Select `feature-b`, press `d`, confirm removal.
- Expected: Worktree removal completes, associated tab closes, sidebar refreshes, and remaining rows stay correctly classified.

## Test 8: Click behavior consistency
- Action: On a non-active row, click once (select) then click again (activate).
- Expected: First click only changes selection; second click switches tabs.

## Test 9: Selection stability after tab updates
- Action: Create another temporary manual tab, then close it.
- Expected: Sidebar recomputes without losing selection consistency or mislabeling existing rows.
