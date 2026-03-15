---
fixture: setup-with-worktrees.sh
launch: ZELLIGENT_PLUGIN_SRC="$HOME/.local/share/zelligent/zelligent-plugin.wasm" ./zelligent.sh
session_name: zelligent-test-repo
---

# Sidebar Agent Status Test

Verifies that visible sidebar status indicators update in response to pipe
messages for managed tabs.

## Test 1: Open managed tabs for the seeded worktrees
- Action: In the control window, run `zellij --session zelligent-test-repo action new-tab --name feature-a`
- Action: In the control window, run `zellij --session zelligent-test-repo action new-tab --name feature-b`
- Action: In the control window, run `zellij --session zelligent-test-repo action new-tab --name feature-c`
- Expected: The sidebar shows `feature-a`, `feature-b`, and `feature-c` as tabs without active status indicators yet

## Test 2: Working status shows on `feature-a`
- Action: In the control window, run `zellij --session zelligent-test-repo pipe --name zelligent-status --args "event=Start,tab=feature-a"`
- Expected: The `feature-a` row shows the working indicator in the sidebar

## Test 3: Needs-input status shows on `feature-b`
- Action: In the control window, run `zellij --session zelligent-test-repo pipe --name zelligent-status --args "event=PermissionRequest,tab=feature-b"`
- Expected: The `feature-b` row shows the needs-input indicator in the sidebar

## Test 4: Done status shows on `feature-c`
- Action: In the control window, run `zellij --session zelligent-test-repo pipe --name zelligent-status --args "event=Stop,tab=feature-c"`
- Expected: The `feature-c` row shows the done indicator in the sidebar

## Test 5: Later updates replace earlier status
- Action: In the control window, run `zellij --session zelligent-test-repo pipe --name zelligent-status --args "event=Stop,tab=feature-a"`
- Expected: The `feature-a` row changes from working to done instead of keeping the old indicator
