# Architecture

Zelligent has two components that work together:

## CLI (`zelligent.sh`)

A Bash script installed as `zelligent`. Handles:

- **Session management** — creates/attaches Zellij sessions named after the git repo (`basename` of repo root)
- **Worktree lifecycle** — `spawn` creates git worktrees under `~/.zelligent/worktrees/<repo>/`, `remove` cleans them up
- **Layout generation** — builds KDL layout files for Zellij tabs (agent pane + lazygit pane + chrome)
- **Doctor** — `zelligent doctor` sets up Zellij config, keybindings, plugin permissions, and Claude Code plugin
- **Nuke** — `zelligent nuke` force-deletes the session, its server processes, and resurrection cache

The CLI resolves the main repo root even when run from a worktree (`git rev-parse --git-common-dir`).

## Zellij WASM Plugin (`plugin/`)

A Rust plugin compiled to `wasm32-wasip1`. Provides a floating UI inside Zellij (launched via Ctrl-Y keybinding). Handles:

- **Worktree browsing** — lists worktrees, lets you switch tabs or spawn/remove worktrees
- **Branch selection** — browse existing branches or type a new branch name
- **Tab management** — switches to worktree tabs, closes tabs (name-based, not index-based)
- **Agent status** — reads CLI pipe messages to show agent status (idle/working/needs-input/done)
- **Session awareness** — gets session name via `SessionUpdate` events (not env vars, WASM sandbox blocks those)

### Key file paths

| File | Purpose |
|------|---------|
| `zelligent.sh` | CLI entry point |
| `plugin/src/lib.rs` | Plugin state machine, event handling, command dispatch |
| `plugin/src/ui.rs` | Plugin UI rendering |
| `plugin/tests/render_snapshots.rs` | Insta snapshot tests for UI |
| `dev-install.sh` | Build + install CLI and WASM plugin locally |
| `test.sh` | Full test suite runner |

### How they interact

```
User runs `zelligent spawn feature/foo claude`
  -> CLI creates git worktree at ~/.zelligent/worktrees/<repo>/feature/foo
  -> CLI generates KDL layout (agent pane + lazygit pane)
  -> CLI calls `zellij action new-tab --layout <file> --name feature-foo`

User presses Ctrl-Y inside Zellij
  -> Zellij launches the WASM plugin as a floating pane
  -> Plugin calls `zelligent list-worktrees` and `zelligent list-branches` via RunCommand
  -> Plugin calls `zelligent spawn/remove` via RunCommand when user selects an action
```

### WASM sandbox constraints

See [references/zellij-plugin-api.md](references/zellij-plugin-api.md) for details. The key constraint: WASM plugins cannot access host environment variables or the filesystem directly. All external operations go through Zellij's plugin API (`RunCommand`, events, pipes).
