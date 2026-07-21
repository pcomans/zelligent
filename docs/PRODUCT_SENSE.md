# Product Sense

## What zelligent is

Zelligent spawns AI coding agents into isolated git worktrees, each in its own Zellij tab with a side-by-side lazygit pane. It manages the full lifecycle: creating worktrees, opening tabs, and cleaning up.

## UX principles

- **One repo = one Zellij session.** Session is named after the repo directory.
- **One branch = one worktree = one tab.** Each agent gets an isolated copy of the code.
- **Tabs are named after branches.** `feature/my-thing` becomes tab `feature-my-thing` (slashes replaced with dashes).
- **Persistent left sidebar.** Every zelligent-managed tab includes the sidebar plugin as an always-visible left pane.
- **Main tab body stays task-focused.** The default body is 70% agent and 30% lazygit, to the right of the sidebar.
- **Minimal setup.** `zelligent doctor` configures everything. `zelligent` with no args creates or attaches to the session.

## Sidebar interaction contract

Ground truth: `plugin/src/lib.rs` unit tests for `handle_mouse_browse` / `handle_key_browse` (search `browse_mouse_` / `browse_key_`). This is the single normative statement — test plans and other docs must defer to it, not restate it.

**Mouse:**
- A single left click on a sidebar item's title line OR its subtitle line selects **and** activates that item in one step (#137) — activation switches to the item's tab if one exists, or spawns it if detached.
- Clicking the already-selected item still activates (idempotent: no duplicate spawn, no tab churn).
- Clicks on the header, the blank separator, the footer, past the last item, or anywhere while the empty state is showing are no-ops: no selection change, no action.
- Wheel scroll (up/down) moves the `▌` selection cursor one row at a time and wraps at both ends; it never activates.

**Keyboard (browse mode):**
- `j` / `Down` and `k` / `Up` move the selection one row, wrapping at both ends — selection only, no activation.
- `Enter` activates the currently-selected item (switch to its tab, or spawn if detached) — a separate step from selection, unlike the mouse's combined click.
- `n` opens branch-select mode (choose an existing branch to spawn); `i` opens input-branch mode (type a new branch name); `d` opens the remove-confirmation mode for the selected worktree tab; `r` refreshes; `q`/`Esc` are no-ops in browse mode.
- The branch picker keeps its own cursor (it always opens on row 0); leaving the picker — Esc or Enter — never moves the browse selection, and after Enter the cursor follows the active-tab re-sync once the switch/spawn lands (#184, #151).

Not part of this contract but a real driving hazard: Zellij's click-to-focus eats exactly one click when the sidebar pane isn't already focused (the "focus-claim click"), recurring after every cross-tab landing. Tracked as #189.

The footer status message is tiered (#186): info self-clears after ~8s, but an error persists until a newer status replaces it or the next Key/Mouse interaction with the sidebar clears it — every interaction described above counts, whatever mode it's in. A remove's completion cue reaches every sidebar instance in the session, not just the one that initiated it (#194), since it rides the same cross-instance invalidate broadcast that heals stale rows — covering both the tab you land on after removing your own worktree's tab, and CLI-side removals run outside the plugin entirely.

## Conventions

### Session name format

Branch names are sanitized for Zellij session/tab names:
- Replace `/` with `-`
- Strip characters outside `[a-zA-Z0-9_-]`
- Example: `feature/my-branch` -> tab named `feature-my-branch`

### Layout format

Default layout: a fragment with a left sidebar pane, a main body, and the status bar. `.zelligent/layout.kdl` is fragment-based and must contain `{{zelligent_sidebar}}` and `{{zelligent_children}}` exactly once. `{{cwd}}` and `{{agent_cmd}}` are optional runtime placeholders. See [references/zellij-kdl-layout.md](references/zellij-kdl-layout.md) for format rules and gotchas.

### Agent command

`zelligent spawn <branch> [agent-cmd]` defaults to `$SHELL` if no agent command is given.

### Worktree storage

All worktrees live under `~/.zelligent/worktrees/<repo-name>/<branch-name>`. This keeps them out of the main repo directory.

### Hooks

Repos can provide `.zelligent/setup.sh` and `.zelligent/teardown.sh` scripts that run during spawn/remove. `zelligent init` creates stubs.
