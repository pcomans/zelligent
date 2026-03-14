---
fixture: setup-empty-repo.sh
---

# Empty Repo Sidebar Smoke Test

Verifies zelligent starts cleanly in a repo with no extra worktrees and still shows the persistent sidebar.

## Test 1: Start zelligent
- Action: In the `view` window, start the repo-under-test `zelligent.sh`, dismiss the built-in Zellij tip pane if needed, then launch the zelligent plugin directly via `zellij action launch-or-focus-plugin`
- Expected: Zellij starts and the persistent left sidebar is visible immediately

## Test 2: Browse-mode quit keys are harmless
- Action: Press `q`, then `Esc`
- Expected: The sidebar remains visible and the session stays open

## Test 3: Version is displayed
- Action: Read the terminal buffer
- Expected: A semantic version string (e.g., "0.1.14") appears somewhere in the sidebar footer

## Test 4: Clean terminal state
- Action: Read the terminal buffer again after a short wait
- Expected: No visible crash output or shell error text appears in the session
