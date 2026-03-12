---
fixture: setup-with-worktrees.sh
---

# Sidebar Navigation Test

Verifies keyboard navigation, branch picker, input mode, and tab switching in the sidebar.
The fixture creates 3 worktrees: feature-a, feature-b, feature-c.

## Prerequisites
- Start a zelligent session: `cd /tmp/zelligent-test-repo && zelligent`
- Then spawn worktrees: `zelligent spawn feature-a`, `zelligent spawn feature-b`, `zelligent spawn feature-c`

## Test 1: Sidebar shows worktrees with two-line rows
- Action: Take a snapshot of the sidebar pane
- Expected: The sidebar shows three items. Each item has two lines: the tab name on line 1 and "branch: <name>" on line 2. The first item is highlighted (inverse video).

## Test 2: Navigate down with j
- Action: Focus the sidebar pane (click on it), then press j twice
- Expected: The selection moves to the third item ("feature-c"). Use inverse video cell detection to verify.

## Test 3: Navigate up with k
- Action: Press k once
- Expected: The selection moves to "feature-b" (second item).

## Test 4: Navigate wraps around
- Action: Press k twice (from feature-b → feature-a → feature-c via wrap)
- Expected: The selection wraps to "feature-c" (last item).

## Test 5: Enter switches to selected tab
- Action: Navigate to "feature-b" (press k), then press Enter
- Expected: The active tab changes to "feature-b". The main pane shows the feature-b worktree directory.

## Test 6: Branch picker opens with n
- Action: Press n
- Expected: The sidebar switches to branch selection mode showing "Select a branch:" header and a list of branches.

## Test 7: Escape exits branch picker
- Action: Press Escape
- Expected: The sidebar returns to browse mode showing the worktree list.

## Test 8: Input mode opens with i
- Action: Press i
- Expected: The sidebar shows "New branch name:" prompt with a cursor.

## Test 9: Typing in input mode
- Action: Type "test-input"
- Expected: The input field shows "test-input" with a cursor after it.

## Test 10: Escape exits input mode
- Action: Press Escape
- Expected: The sidebar returns to browse mode. The input buffer is cleared.

## Test 11: Refresh with r
- Action: Press r
- Expected: The sidebar shows "Refreshed" status message briefly. All three worktrees still visible.

## Test 12: q is no-op (sidebar stays)
- Action: Press q
- Expected: Nothing happens. The sidebar remains visible and functional. The sidebar is NOT closed.

## Test 13: Esc is no-op in browse mode
- Action: Press Escape while in browse mode
- Expected: Nothing happens. The sidebar remains visible and functional.
