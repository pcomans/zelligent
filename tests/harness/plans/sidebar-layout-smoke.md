---
fixture: setup-empty-repo.sh
launch: zelligent  # INSTALLED CLI — never the fixture clone's ./zelligent.sh (old main; see README "CLI under test")
session_name: zelligent-test-repo
---

# Sidebar Layout Smoke Test

Verifies the PR 83 layout behavior started through `zelligent`.

## Test 1: Session startup renders the sidebar
- Action: Wait for the `launch: zelligent` command to create the session
- Expected: The session opens with a persistent left sidebar

## Test 2: Initial repo tab uses the shared layout
- Action: Read the terminal buffer
- Expected: The initial tab shows the sidebar plus the standard shell and lazygit body

## Test 3: Spawn from inside the session keeps the same sidebar pane
- Action: Spawn from the sidebar UI itself (never from the control window — see README "CLI under test"): focus the sidebar in the view window, press `i`, type `feature-a`, press Enter
- Expected: A new tab opens in the live session and still shows the persistent left sidebar

## Test 4: Manual tabs inherit the sidebar
- Action: Press `Ctrl-t`, then `n` (creates an unnamed tab immediately — Zellij has no inline rename prompt here; do NOT type a name, it would land in the new tab's shell). To name it, use `zellij action rename-tab scratch` from the control window
- Expected: The new manual tab keeps the borderless left sidebar pane inherited from `default_tab_template` (36 cols, plugin's in-pane ` <repo> ` header, no Zellij frame title above it)
