---
fixture: setup-with-worktrees.sh
---

# Worktree Navigation Test

Verifies the plugin correctly lists and navigates worktrees.
The fixture creates 3 worktrees: feature-a, feature-b, feature-c.

## Test 1: Plugin opens and shows worktrees
- Action: Run `zelligent spawn smoke-test bash`
- Expected: A new tab opens with the persistent left sidebar showing session tabs including "feature-a", "feature-b", "feature-c", and "smoke-test".

## Test 2: Navigate down with j
- Action: Press j twice
- Expected: The selection moves to "feature-c" (third item)

## Test 3: Navigate up with k
- Action: Press k once
- Expected: The selection moves back to "feature-b" (second item)

## Test 4: Refresh with r
- Action: Press r
- Expected: The worktree list refreshes. All three worktrees still visible.

## Test 5: Switch tab from sidebar and keep sidebar visible
- Action: Select another row and press Enter
- Expected: Focus switches to that tab and the left sidebar remains visible (it does not close).

## Test 6: Version is displayed
- Action: Read the terminal buffer
- Expected: A semantic version string (e.g., "0.1.14") appears in the plugin UI

## Test 7: Sidebar is persistent
- Action: Press q
- Expected: Plugin pane closes only in the current tab context (if closable), but tab switching via Enter never closes the sidebar automatically.
