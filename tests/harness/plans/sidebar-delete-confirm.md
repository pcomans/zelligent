---
fixture: setup-with-worktrees.sh
---

# Sidebar Delete Confirmation Test

Verifies the delete confirmation flow in the sidebar.
The fixture creates 3 worktrees: feature-a, feature-b, feature-c.

## Prerequisites
- Start a zelligent session with spawned worktrees

## Test 1: Press d to enter confirm mode
- Action: Navigate to "feature-b" (press j once), then press d
- Expected: The sidebar shows "Remove worktree for 'feature-b'?" with "y confirm  n/Esc cancel" options.

## Test 2: Escape cancels deletion
- Action: Press Escape
- Expected: The sidebar returns to browse mode. All three worktrees are still listed.

## Test 3: Press d again then n cancels
- Action: Press d (on feature-b), then press n
- Expected: The sidebar returns to browse mode. All three worktrees still present.

## Test 4: Confirm deletion with y
- Action: Press d (on feature-b), then press y
- Expected: The worktree "feature-b" is removed. The sidebar list refreshes and shows only "feature-a" and "feature-c".
