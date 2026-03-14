---
fixture: setup-with-worktrees.sh
---

# Persistent Sidebar Behavior Test

Verifies the destructive and non-destructive sidebar behaviors that snapshots cannot prove in a live session.

## Test 1: Start zelligent and seed managed tabs
- Action: Start the repo-under-test `zelligent.sh` in the `view` window, dismiss the built-in Zellij tip pane if needed, launch the zelligent plugin directly via `zellij action launch-or-focus-plugin`, then use the `ctrl` window to spawn `feature-a` and `feature-b` into the live session
- Expected: The persistent sidebar is visible and contains rows for the repo tab, `feature-a`, and `feature-b`

## Test 2: Add a user tab
- Action: In the `ctrl` window, create a plain Zellij tab named `notes` in the live session
- Expected: The sidebar shows a `notes` row that is distinct from managed worktree rows

## Test 3: User-tab delete is a no-op
- Action: Navigate to `notes`, press `d`, then capture the terminal
- Expected: No confirm prompt appears, the tab stays open, and the sidebar remains in browse mode

## Test 4: Main-tab delete is a no-op
- Action: Navigate to the repo tab (`main` or the current repo branch), press `d`, then capture the terminal
- Expected: No confirm prompt appears and the sidebar remains in browse mode

## Test 5: Managed worktree delete enters confirm mode
- Action: Navigate to `feature-b` and press `d`
- Expected: The sidebar shows the confirm prompt for removing worktree `feature-b`

## Test 6: Cancel delete returns to browse mode
- Action: Press `n`
- Expected: The confirm prompt disappears and the sidebar returns to the normal browse view with `feature-b` still present

## Test 7: Browse-mode quit keys are no-ops
- Action: Press `q`, then `Esc`
- Expected: The sidebar remains visible after both keys
