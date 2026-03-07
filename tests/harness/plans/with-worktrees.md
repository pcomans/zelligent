---
fixture: setup-with-worktrees.sh
---

# Worktree Navigation Test

Verifies the plugin correctly lists and navigates worktrees.
The fixture creates 3 worktrees: feature-a, feature-b, feature-c.

## Test 1: Plugin opens and shows worktrees
- Action: Press Ctrl+Y
- Expected: The plugin shows a list containing "feature-a", "feature-b", "feature-c". The first item ("feature-a") is highlighted.

## Test 2: Navigate down with j
- Action: Press j twice
- Expected: The selection moves to "feature-c" (third item)

## Test 3: Navigate up with k
- Action: Press k once
- Expected: The selection moves back to "feature-b" (second item)

## Test 4: Refresh with r
- Action: Press r
- Expected: The worktree list refreshes. All three worktrees still visible.

## Test 5: Close and reopen preserves list
- Action: Press q, then Ctrl+Y
- Expected: Plugin closes then reopens showing the same 3 worktrees

## Test 6: Version is displayed
- Action: Read the terminal buffer
- Expected: The version string "0.1.14" appears in the plugin UI

## Test 7: Clean close
- Action: Press q
- Expected: Back to shell prompt, no errors
