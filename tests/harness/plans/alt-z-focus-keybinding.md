---
fixture: setup-with-worktrees.sh
launch: zelligent  # INSTALLED CLI — never the fixture clone's ./zelligent.sh (old main; see README "CLI under test")
session_name: zelligent-test-repo
---

# Alt-z Focus-Sidebar Keybinding Test

Verifies the doctor-installed `Alt z` keybinding: a URL-less `MessagePlugin`
pipe (`zelligent-focus`) that only the visible sidebar instance answers by
focusing itself. See decision 50 in
`docs/design-docs/sidebar-layout-decisions.md`.

Setup note (BEFORE evaluating any test): zellij reads keybinds at session
start, so the binding must exist in the config before `launch` runs. From the
control window run `zelligent doctor` and check the `keybinding:` line. If it
prints `added` (not `ok`), the session that `launch` created predates the
binding — kill the session (`zellij kill-session zelligent-test-repo`) and
re-run the `launch` command in the view window before continuing.

Focus assertions use the layout dump from the control window, never colors:
`ZELLIJ_SESSION_NAME=zelligent-test-repo zellij action dump-layout`
— the focused pane carries `focus=true`. The sidebar pane is
`pane name="zelligent"`. Send Alt-z to the view window as
`tmux -L zt-driver-test send-keys -t zt-driver:view M-z`. Allow ~1s after
each keypress before dumping.

## Test 1: Binding is installed

- Action: From the control window, run `zelligent doctor` and read the
  `keybinding:` output line
- Expected: `keybinding: ok (Alt z in …config.kdl)` — the binding is present
  with pipe name `zelligent-focus` (verify with
  `grep -A3 'bind "Alt z"' <that config.kdl>`)

## Test 2: Alt-z focuses the sidebar in the startup tab

- Action: Confirm via dump-layout that focus starts on a body pane (the
  shell or lazygit pane has `focus=true`, not `name="zelligent"`). Send
  `M-z` to the view window, wait ~1s, dump-layout again
- Expected: The pane with `focus=true` is now `pane name="zelligent"`; the
  focused tab is unchanged

## Test 3: Focused sidebar accepts keyboard navigation

- Action: With the sidebar focused from Test 2, capture the view window with
  ANSI codes (`capture-pane -e -J`), note the highlighted row, press `j`,
  capture again
- Expected: The selection highlight moves down one row (fixture seeds
  `feature-a`, `feature-b`, `feature-c`, so there are rows to move between)

## Test 4: Alt-z targets the current tab's own sidebar

- Action: Switch to a worktree tab (press Enter on a highlighted worktree
  row, or `zellij action go-to-tab-name feature-a` from the control window),
  wait ~1s. Confirm via dump-layout that the `feature-a` tab has
  `focus=true` and its focused pane is a body pane. Send `M-z`, wait ~1s,
  dump-layout
- Expected: The `feature-a` tab still has `focus=true` (no tab switch), and
  within it the `focus=true` pane is `name="zelligent"` — the hidden sidebar
  instances in other tabs did not steal focus

## Test 5: Alt-z is idempotent when the sidebar is already focused

- Action: With the sidebar focused from Test 4, send `M-z` again, wait ~1s,
  dump-layout
- Expected: Same tab still focused, sidebar pane still the `focus=true`
  pane — no error banner in the sidebar, no tab or pane change
