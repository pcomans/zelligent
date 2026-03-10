---
fixture: setup-with-worktrees.sh
---

# Worktree Navigation Test

Verifies the sidebar plugin correctly lists and navigates worktrees.
The fixture creates 3 worktrees: feature-a, feature-b, feature-c.

## Test 1: Sidebar shows worktrees on start
- Action: Start a zelligent session
- Expected: The sidebar on the left shows a list containing "feature-a", "feature-b", "feature-c" as two-line rows (name + branch subtitle). The first item is highlighted.

## Test 2: Navigate down with j
- Action: Click the sidebar to focus it, then press j twice
- Expected: The selection moves to "feature-c" (third item)

## Test 3: Navigate up with k
- Action: Press k once
- Expected: The selection moves back to "feature-b" (second item)

## Test 4: Refresh with r
- Action: Press r
- Expected: The worktree list refreshes. All three worktrees still visible.

## Test 5: Mouse click selects item
- Action: Click on "feature-a" in the sidebar
- Expected: The selection moves to "feature-a" (first item)

## Test 6: Enter switches tab
- Action: Press Enter on the selected item
- Expected: The active tab switches to the selected worktree's tab

## Test 7: Version is displayed
- Action: Read the terminal buffer
- Expected: A semantic version string (e.g., "0.1.14") appears in the sidebar footer
