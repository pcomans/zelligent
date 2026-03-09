---
fixture: setup-empty-repo.sh
---

# Empty Repo Smoke Test

Verifies the persistent sidebar plugin works correctly when starting from a repo with no managed worktrees.

## Test 1: Spawn creates a sidebar tab
- Action: Run `zelligent spawn smoke-empty bash`
- Expected: A new tab opens with the zelligent sidebar visible on the left.

## Test 2: Sidebar shows required two-line rows
- Action: Read the sidebar list.
- Expected: Each row renders as two lines (title + subtitle).

## Test 3: Enter switches tabs without hiding sidebar
- Action: Create/switch to another tab row using Enter from the sidebar.
- Expected: Tab focus changes and the sidebar remains visible.

## Test 4: Version is displayed
- Action: Read the terminal buffer
- Expected: A semantic version string (e.g., "0.1.14") appears somewhere in the plugin UI

## Test 5: Key hints are visible
- Action: Inspect the bottom hint row of the sidebar.
- Expected: Navigation/action keys are shown in the footer.
