---
fixture: setup-with-worktrees.sh
launch: ZELLIGENT_PLUGIN_SRC="$HOME/.local/share/zelligent/zelligent-plugin.wasm" ./zelligent.sh
session_name: zelligent-test-repo
---

# Sidebar Delete Confirmation Test

Verifies the remove flow and the user-tab delete guardrail in the persistent
sidebar.

## Test 1: `d` opens confirm mode for a managed worktree
- Action: Select `feature-b`, then press `d`
- Expected: The sidebar switches to a confirmation prompt asking to remove `feature-b`

## Test 2: `Esc` cancels deletion
- Action: Press `Esc`
- Expected: The sidebar returns to browse mode and `feature-b` is still present

## Test 3: `n` also cancels deletion
- Action: Press `d` again on `feature-b`, then press `n`
- Expected: The sidebar returns to browse mode and `feature-b` is still present

## Test 4: Manual tabs cannot enter confirm mode
- Action: In the control window, run `ZELLIJ=1 ZELLIJ_SESSION_NAME=zelligent-test-repo zellij action new-tab --name notes`, then select `notes` and press `d`
- Expected: The sidebar stays in browse mode and shows an error indicating only worktree tabs can be removed

## Test 5: Confirmed deletion removes the worktree
- Action: Re-select `feature-b`, press `d`, then press `y`
- Expected: The sidebar refreshes without `feature-b`

## Test 6: The worktree is gone from git state too
- Action: In the control window, run `git worktree list`
- Expected: The output still includes `feature-a` and `feature-c` but no longer includes `feature-b`
