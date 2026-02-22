# Agent Instructions

## Testing

Run the test suite before and after changes:

```bash
bash test.sh
```

The suite has two levels:

### Unit tests (always run)
- Session name sanitization (`/` → `-`)
- Layout file content (tab-bar, status-bar, agent command, cwd, lazygit)
- Argument validation (no args, remove without branch)
- Dependency checks (missing zellij, non-git directory)

### Integration tests (require Zellij installed)
Uses `zellij attach --create-background SESSION` to create a headless session,
then runs `ZELLIJ_SESSION_NAME=SESSION zellij action ...` to test against it,
then verifies state with `zellij action dump-layout`.

This lets you test layout loading and tab creation without a live terminal.

```bash
# The integration test does this automatically, but you can also run manually:
SESSION=spawn-agent-test-manual
zellij attach --create-background "$SESSION"
ZELLIJ_SESSION_NAME="$SESSION" zellij action new-tab --layout my-layout.kdl --name test
ZELLIJ_SESSION_NAME="$SESSION" zellij action dump-layout   # inspect the result
zellij kill-session "$SESSION"
```

### What still requires manual testing
- Visual appearance of the tab (pane sizes, chrome rendering)
- Agent command actually launching (claude, lazygit)
- End-to-end: running spawn-agent.sh from inside a live Zellij session

For end-to-end testing, run from inside an existing Zellij session:
```bash
./spawn-agent.sh feature/test-branch claude
# Expected: new tab opens with left pane (claude) + right pane (lazygit)
# Expected: tab-bar at top, status-bar at bottom
# Expected: tab named "feature-test-branch"

./spawn-agent.sh remove feature/test-branch
# Expected: worktree removed, branch preserved, tab note printed
```

## Session name format

Branch names are sanitized by replacing `/` with `-`:
- `feature/my-branch` → tab named `feature-my-branch`
- `main` → tab named `main`

## Layout format

The default layout (`.spawn-agent/layout.kdl` override supported) must:
- Include `plugin location="zellij:tab-bar"` as first pane
- Include `plugin location="zellij:status-bar"` as last pane
- NOT wrap content in a `tab { }` block (that's for session layouts, not `new-tab`)
- Use `{{cwd}}` and `{{agent_cmd}}` as template variables when providing a custom layout

## Key Zellij behaviors discovered

### Session creation
- `zellij --session NAME --layout FILE` — ALWAYS tries to add layout as a new tab to session NAME, whether inside Zellij or not. Fails with "Session not found" if NAME doesn't exist. Never creates a new session.
- `zellij --new-session-with-layout FILE --session NAME` — ALWAYS creates a new named session. Use this when outside Zellij to create a session with a specific name and layout.
- `zellij --new-session-with-layout FILE` inside an existing session — creates a nested Zellij (wrong).
- `session { name "..." }` block in layout KDL — not supported in Zellij 0.43.1.

### Tab creation (inside Zellij)
- `zellij action new-tab --layout FILE --name NAME` — correct way to open a new tab in the current session.
- `new-tab --layout` does NOT inherit `default_tab_template`; tab-bar and status-bar must be included explicitly in the layout file.
- A `tab { }` wrapper in the layout file passed to `new-tab` causes session-level replacement (breaks existing tabs and chrome). Layout for `new-tab` must contain only panes at the top level.

### Testing without a live terminal
- `zellij attach --create-background SESSION` — creates a headless background session (no terminal needed).
- `ZELLIJ_SESSION_NAME=SESSION zellij action ...` — runs actions against a specific session from outside it.
- `zellij action dump-layout` — inspect the full layout state of a session (use for test assertions).
- Mock zellij with a fake binary on PATH to test which command the script invokes without blocking.

### Session name constraints
- Session names must be `[a-zA-Z0-9_-]` only — no spaces, no slashes.
- The "less than 0 characters" error from Zellij is a display bug masking a name validation failure.
- Sanitize branch names by replacing `/` with `-`.

### Tab index vs position (plugin API)
- Each tab has a stable **index** (assigned at creation, never changes) and a **position** (visual slot in the tab bar, shifts when earlier tabs are closed).
- `TabInfo.position` from `TabUpdate` events gives the **position**, not the index.
- `close_tab_with_index` and `rename_tab` expect the internal **index**, not the position. Passing a position will target the wrong tab whenever a tab has been closed earlier in the session. ([zellij-org/zellij#3535](https://github.com/zellij-org/zellij/issues/3535))
- **Workaround:** Use `go_to_tab_name(name)` + `close_focused_tab()` instead of `close_tab_with_index`. Name-based lookup is immune to index/position mismatch.
- When closing a tab this way, save the active tab name first and call `go_to_tab_name` again afterward to return focus.
- Always guard with `self.tabs.iter().any(|t| t.name == tab_name)` before proceeding — if the tab doesn't exist, `go_to_tab_name` is a no-op and `close_focused_tab` would close whatever tab is currently focused.
- There is no plugin API to obtain a tab's internal index. The only reliable identifier is the tab name.

### $ZELLIJ environment variable
- Set by Zellij when running inside a session. Use `[ -n "$ZELLIJ" ]` to detect this.
- Unset it (`ZELLIJ=""`) when testing the outside-Zellij code path.
