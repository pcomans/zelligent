# Architecture

Zelligent has two components that work together:

## CLI (`zelligent.sh`)

A Bash script installed as `zelligent`. Handles:

- **Session management** — creates/attaches Zellij sessions named after the git repo (`basename` of repo root)
- **Worktree lifecycle** — `spawn` creates git worktrees under `~/.zelligent/worktrees/<repo>/`, `remove` cleans them up
- **Layout generation** — builds KDL layout files for spawned tabs (persistent sidebar plugin + agent/lazygit split + status-bar)
- **Doctor** — `zelligent doctor` sets up Zellij config, plugin permissions, and Claude Code plugin
- **Nuke** — `zelligent nuke` force-deletes the session, its server processes, and resurrection cache

The CLI resolves the main repo root even when run from a worktree (`git rev-parse --git-common-dir`).

## Zellij WASM Plugin (`plugin/`)

A Rust plugin compiled to `wasm32-wasip1`. It runs as a persistent sidebar in zelligent-managed spawned tabs. Handles:

- **Session tab browsing** — lists all open session tabs and enriches rows when a tab maps to a zelligent-managed worktree
- **Branch selection** — browse existing branches or type a new branch name
- **Tab management** — switches tabs and removes matched worktrees (name-based, not index-based)
- **Agent status** — reads CLI pipe messages to show agent status (idle/working/needs-input/done) and sends OS notifications. See [design-docs/agent-notifications.md](design-docs/agent-notifications.md).
- **Session awareness** — gets session name via `std::env::var("ZELLIJ_SESSION_NAME")` (WASI inherits host env vars)

### Key file paths

| File | Purpose |
|------|---------|
| `zelligent.sh` | CLI entry point |
| `plugin/src/lib.rs` | Plugin state machine, event handling, command dispatch |
| `plugin/src/ui.rs` | Plugin UI rendering |
| `plugin/tests/render_snapshots.rs` | Insta snapshot tests for UI |
| `dev-install.sh` | Build + install CLI and WASM plugin locally |
| `test.sh` | Full test suite runner |
| `claude-plugin/` | Claude Code plugin (hooks for agent status notifications) |

### How they interact

```
User runs `zelligent spawn feature/foo claude`
  -> CLI creates git worktree at ~/.zelligent/worktrees/<repo>/feature/foo
  -> CLI generates KDL layout (sidebar plugin + agent pane + lazygit pane + status-bar)
  -> CLI calls `zellij action new-tab --layout <file> --name feature-foo`

Plugin starts as an embedded sidebar
  -> Plugin calls `zelligent list-worktrees` and `zelligent list-branches` via RunCommand
  -> Plugin calls `zelligent spawn/remove` via RunCommand when user selects an action
```

### WASM plugin environment

See [references/zellij-plugin-api.md](references/zellij-plugin-api.md) for API details. Plugins run in a WASI sandbox but inherit the host environment (`std::env::var()` works). External operations go through Zellij's plugin API (`RunCommand`, events, pipes).

## Claude Code Plugin (`claude-plugin/`)

A Claude Code plugin installed by `zelligent doctor`. Provides hooks that send agent status events to the Zellij plugin via `zellij pipe`. See [design-docs/agent-notifications.md](design-docs/agent-notifications.md) for the full pipeline.
