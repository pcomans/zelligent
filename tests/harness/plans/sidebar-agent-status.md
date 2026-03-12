---
fixture: setup-with-worktrees.sh
---

# Sidebar Agent Status Test

Verifies that agent status indicators display correctly in the sidebar.
The fixture creates 3 worktrees: feature-a, feature-b, feature-c.

## Prerequisites
- Start a zelligent session with spawned worktrees
- Each worktree tab should have the sidebar visible

## Test 1: All worktrees show idle status by default
- Action: Read the sidebar pane content
- Expected: All three items show with a 2-space prefix (no colored dot). No status indicators visible.

## Test 2: Working status shows green dot
- Action: Send a pipe message: `zellij pipe --name zelligent-status --args "event=Start,tab=feature-a"`
- Expected: The "feature-a" row in the sidebar now shows a green dot "●" indicator.

## Test 3: Awaiting status shows yellow dot
- Action: Send a pipe message: `zellij pipe --name zelligent-status --args "event=PermissionRequest,tab=feature-b"`
- Expected: The "feature-b" row shows a yellow dot "●" indicator.

## Test 4: Done status shows green checkmark
- Action: Send a pipe message: `zellij pipe --name zelligent-status --args "event=Stop,tab=feature-c"`
- Expected: The "feature-c" row shows a green checkmark "✓" indicator.

## Test 5: Status updates replace previous status
- Action: Send: `zellij pipe --name zelligent-status --args "event=Stop,tab=feature-a"`
- Expected: The "feature-a" row now shows a green checkmark instead of the green dot.
