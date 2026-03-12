# Product Sense

## What zelligent is

Zelligent runs AI coding agents in isolated git worktrees, each in its own Zellij tab. A zelligent-managed session always has a persistent sidebar on the left for browsing, spawning, and removing worktrees.

## UX principles

- **One repo = one Zellij session.** Session name is derived from the repo directory.
- **One branch = one worktree = one tab.** Each agent gets its own isolated checkout.
- **Sidebar is mandatory.** If the sidebar plugin cannot be loaded, zelligent fails instead of silently dropping to plain Zellij.
- **Layouts come from files, not script heredocs.** Runtime layout precedence is repo `.zelligent/layout.kdl`, then `~/.zelligent/layout.kdl`, then hard failure.
- **Repo layout owns the body, zelligent owns the sidebar.** Custom layouts can change the rest of the tab, but must include `{{zelligent_sidebar}}` exactly once.
- **No hidden launch modes.** Plain `zelligent` and `zelligent spawn` share the same layout rendering pipeline; only placeholder values differ by context.
- **Manual tabs keep the frame.** New tabs created directly in Zellij inside a zelligent-managed session must inherit the sidebar frame. Full body parity is best-effort if Zellij limits `default_tab_template`.
- **Minimal global setup.** `zelligent doctor` installs permissions, config defaults, and the default layout. It does not add a plugin keybinding.

## Conventions

### Session and tab names

Branch names are sanitized for Zellij tab names:
- Replace `/` with `-`
- Strip characters outside `[a-zA-Z0-9_-]`
- Example: `feature/my-branch` -> `feature-my-branch`

### Layout contract

`.zelligent/layout.kdl` is a full layout file with three supported placeholders:
- `{{zelligent_sidebar}}` required exactly once
- `{{cwd}}` optional
- `{{agent_cmd}}` optional

`{{cwd}}` resolves to:
- repo root for plain `zelligent`
- spawned worktree path for `zelligent spawn`

`{{agent_cmd}}` resolves to a shell command fragment intended for `bash -c`, not just a raw binary name.

### Worktree storage

All spawned worktrees live under `~/.zelligent/worktrees/<repo-name>/<branch-name>`.

### Hooks

Repos can provide `.zelligent/setup.sh` and `.zelligent/teardown.sh`. `zelligent init` creates stub versions of those scripts.
