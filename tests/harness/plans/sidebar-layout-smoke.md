---
fixture: setup-empty-repo.sh
launch: ZELLIGENT_PLUGIN_SRC="$HOME/.local/share/zelligent/zelligent-plugin.wasm" ./zelligent.sh
session_name: zelligent-test-repo
---

# Sidebar Layout Smoke Test

Verifies the PR 83 layout behavior started through `zelligent`.

## Test 1: Session startup renders the sidebar
- Action: Wait for the `launch: zelligent` command to create the session
- Expected: The session opens with a persistent left sidebar already embedded in the layout

## Test 2: Initial repo tab uses the shared layout
- Action: Read the terminal buffer
- Expected: The initial tab shows the sidebar plus the standard shell and lazygit body

## Test 3: Spawn from inside the session keeps the same frame
- Action: In the control window, run `ZELLIJ=1 ZELLIJ_SESSION_NAME=zelligent-test-repo ZELLIGENT_PLUGIN_SRC="$HOME/.local/share/zelligent/zelligent-plugin.wasm" ./zelligent.sh spawn feature-a bash`
- Expected: A new tab opens in the live session and still shows the persistent left sidebar

## Test 4: Manual tabs inherit the sidebar
- Action: Press `Ctrl-t`, then `n`, type `scratch`, and press Enter
- Expected: The new manual tab keeps the left sidebar frame inherited from `default_tab_template`
