# Product Sense

## What zelligent is

Zelligent spawns AI coding agents into isolated git worktrees, each in its own Zellij tab with a side-by-side lazygit pane. It manages the full lifecycle: creating worktrees, opening tabs, and cleaning up.

## UX principles

- **One repo = one Zellij session.** Session is named after the repo directory.
- **One branch = one worktree = one tab.** Each agent gets an isolated copy of the code.
- **Tabs are named after branches.** `feature/my-thing` becomes tab `feature-my-thing` (slashes replaced with dashes).
- **Agent + lazygit side by side.** Default layout: 70% agent pane (left), 30% lazygit (right), with tab-bar and status-bar chrome.
- **Ctrl-Y for the plugin.** Floating UI to browse worktrees, spawn new ones, or switch tabs without leaving Zellij.
- **Minimal setup.** `zelligent doctor` configures everything. `zelligent` with no args creates or attaches to the session.

## Conventions

### Session name format

Branch names are sanitized for Zellij session/tab names:
- Replace `/` with `-`
- Strip characters outside `[a-zA-Z0-9_-]`
- Example: `feature/my-branch` -> tab named `feature-my-branch`

### Layout format

The default layout (overridable via `.zelligent/layout.kdl`) must:
- Include `plugin location="zellij:tab-bar"` as first pane
- Include `plugin location="zellij:status-bar"` as last pane
- NOT wrap content in a `tab { }` block (that's for session layouts, not `new-tab`)
- Use `{{cwd}}` and `{{agent_cmd}}` as template variables in custom layouts

See [references/zellij-kdl-layout.md](references/zellij-kdl-layout.md) for layout format details.

### Worktree storage

All worktrees live under `~/.zelligent/worktrees/<repo-name>/<branch-name>`. This keeps them out of the main repo directory.

### Hooks

Repos can provide `.zelligent/setup.sh` and `.zelligent/teardown.sh` scripts that run during spawn/remove. `zelligent init` creates stubs.
