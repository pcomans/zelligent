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
  -> CLI generates KDL layout (agent pane + lazygit pane)
  -> CLI calls `zellij action new-tab --layout <file> --name feature-foo`

User presses Ctrl-Y inside Zellij
  -> Zellij launches the WASM plugin as a floating pane
  -> Plugin calls `zelligent list-worktrees` and `zelligent list-branches` via RunCommand
  -> Plugin calls `zelligent spawn/remove` via RunCommand when user selects an action
```

### WASM plugin environment

See [references/zellij-plugin-api.md](references/zellij-plugin-api.md) for API details. Plugins run in a WASI sandbox but inherit the host environment (`std::env::var()` works). External operations go through Zellij's plugin API (`RunCommand`, events, pipes).

## Glossary

- **Harness tests** (`tests/harness/`): UI acceptance tests driven by a tmux session + Chrome DevTools MCP. Run manually via the `test-driver` agent, not in CI.
- **Test harness** (general): the overall scaffolding that lets agents validate their own work — `test.sh`, CI jobs, lints, snapshot tests.
- **Agent** (zelligent context): a Claude Code instance running in an isolated git worktree tab, not the zelligent plugin itself.

## Claude Code Plugin (`claude-plugin/`)

A Claude Code plugin installed by `zelligent doctor`. Provides hooks that send agent status events to the Zellij plugin via `zellij pipe`. See [design-docs/agent-notifications.md](design-docs/agent-notifications.md) for the full pipeline.
