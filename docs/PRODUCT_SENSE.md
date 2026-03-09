# Product Sense

## What zelligent is

Zelligent spawns AI coding agents into isolated git worktrees, each in its own Zellij tab with a side-by-side lazygit pane. It manages the full lifecycle: creating worktrees, opening tabs, and cleaning up.

## UX principles

- **One repo = one Zellij session.** Session is named after the repo directory.
- **One branch = one worktree = one tab.** Each agent gets an isolated copy of the code.
- **Tabs are named after branches.** `feature/my-thing` becomes tab `feature-my-thing` (slashes replaced with dashes).
- **Persistent sidebar navigator.** Default spawned layout: 24% sidebar (left) + 70/30 agent/lazygit split (right) + status-bar chrome.
- **Single navigation surface.** The sidebar plugin is persistent in zelligent-managed tabs and acts as the canonical tab navigator.
- **Minimal setup.** `zelligent doctor` configures everything. `zelligent` with no args creates or attaches to the session.

## Conventions

### Session name format

Branch names are sanitized for Zellij session/tab names:
- Replace `/` with `-`
- Strip characters outside `[a-zA-Z0-9_-]`
- Example: `feature/my-branch` -> tab named `feature-my-branch`

### Layout format

Default layout: persistent sidebar pane (24%) + agent pane (70%) + lazygit pane (30%) + status-bar. Overridable via `.zelligent/layout.kdl` with `{{cwd}}` and `{{agent_cmd}}` template variables. See [references/zellij-kdl-layout.md](references/zellij-kdl-layout.md) for format rules and gotchas.

### Agent command

`zelligent spawn <branch> [agent-cmd]` defaults to `$SHELL` if no agent command is given.

### Worktree storage

All worktrees live under `~/.zelligent/worktrees/<repo-name>/<branch-name>`. This keeps them out of the main repo directory.

### Hooks

Repos can provide `.zelligent/setup.sh` and `.zelligent/teardown.sh` scripts that run during spawn/remove. `zelligent init` creates stubs.
