---
fixture: setup-with-worktrees.sh
---

# Managed Tab Navigation Test

Verifies managed worktree tabs can be opened and navigated in the persistent sidebar.
The fixture creates worktrees for `feature-a`, `feature-b`, and `feature-c`.

## Test 1: Start zelligent
- Action: In the `view` window, start the repo-under-test `zelligent.sh`, dismiss the built-in Zellij tip pane if needed, then launch the zelligent plugin directly via `zellij action launch-or-focus-plugin`
- Expected: Zellij starts with the persistent left sidebar visible

## Test 2: Open managed worktree tabs
- Action: In the `ctrl` window, run the repo-under-test `zelligent.sh spawn feature-a` and `zelligent.sh spawn feature-b` targeting the live session
- Expected: The sidebar shows rows for `feature-a` and `feature-b` in addition to the repo tab

## Test 3: Navigate managed rows
- Action: Send `j` and `k` in the `view` window until `feature-b` is selected, then move back once
- Expected: The highlight moves predictably between managed rows and never disappears

## Test 4: Open selected row
- Action: Press `Enter` on a managed worktree row
- Expected: Zellij switches to that tab and the persistent sidebar remains visible

## Test 5: Browse-mode quit keys are no-ops
- Action: Press `q`, then `Esc`
- Expected: The sidebar stays visible and the current tab remains open

## Test 6: Footer/version still render
- Action: Capture the terminal buffer
- Expected: The sidebar footer still shows the key hints and a semantic version string
