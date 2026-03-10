---
fixture: setup-empty-repo.sh
---

# Empty Repo Smoke Test

Verifies the sidebar plugin works correctly in a git repo with no worktrees.

## Test 1: Sidebar is visible on session start
- Action: Start a zelligent session for an empty repo
- Expected: The sidebar pane appears on the left (~24% width) showing the Zelligent logo (empty state) and navigation hints (n: pick branch, i: type new branch name)

## Test 2: Version is displayed
- Action: Read the terminal buffer
- Expected: A semantic version string (e.g., "0.1.14") appears in the sidebar footer

## Test 3: Branch picker opens with n
- Action: Click the sidebar pane to focus it, then press n
- Expected: The sidebar switches to branch selection mode showing "Select a branch:" header

## Test 4: Escape returns to browse
- Action: Press Escape
- Expected: The sidebar returns to the empty browse state with the Zelligent logo

## Test 5: Input mode opens with i
- Action: Press i
- Expected: The sidebar switches to input mode showing "New branch name:" prompt with a cursor
