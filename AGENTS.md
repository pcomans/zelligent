# Agent Instructions

Zelligent spawns AI coding agents into isolated git worktrees, each in its own Zellij tab. See [docs/PRODUCT_SENSE.md](docs/PRODUCT_SENSE.md) for UX principles and conventions.

> **Note:** `CLAUDE.md` is a symlink to this file. They are the same document.

## Architecture

CLI (`zelligent.sh`) + Zellij WASM plugin (`plugin/`). See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full map.

## Build & Test

Run the test suite before and after changes:

```bash
bash test.sh
```

### Plugin tests and builds

The plugin's `.cargo/config.toml` defaults to `wasm32-wasip1`. Use the PATH workaround for Homebrew Rust:

```bash
PATH="$HOME/.rustup/toolchains/stable-$(rustc -vV | grep host | cut -d' ' -f2)/bin:$PATH"
```

Run plugin unit tests (must specify host target):

```bash
cd plugin && cargo test --target "$(rustc -vV | awk '/^host:/ {print $2}')"
```

Build and install locally (CLI + WASM plugin):

```bash
PATH="$HOME/.rustup/toolchains/stable-$(rustc -vV | grep host | cut -d' ' -f2)/bin:$PATH" bash dev-install.sh
```

### Render snapshot tests

The plugin uses [insta](https://insta.rs) for render snapshot tests in `plugin/tests/render_snapshots.rs`. When adding a new UI mode or changing render output, add or update snapshot tests:

```bash
cd plugin && INSTA_UPDATE=always cargo test --target "$(rustc -vV | awk '/^host:/ {print $2}')"
```

Review the generated `.snap` files in `plugin/tests/snapshots/` before committing.

### Test levels

**Unit tests (always run):** session name sanitization, layout content, argument validation, dependency checks.

**Integration tests (require Zellij):** headless session creation via `zellij attach --create-background`, layout verification via `dump-layout`.

**Manual testing required:** visual appearance, agent command launching, end-to-end spawn/remove from a live Zellij session. See [docs/design-docs/zellij-behaviors.md](docs/design-docs/zellij-behaviors.md) for testing patterns.

## Push gate

A `.claude/hooks/pre-push-block.sh` hook blocks `git push` unless prefixed with `DOCS_VERIFIED=1`. Before pushing, confirm that `docs/` and `AGENTS.md` have been updated or don't need updates, then run `DOCS_VERIFIED=1 git push ...`.

## Where to look

| Topic | Doc |
|-------|-----|
| Architecture & file map | [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) |
| Product & UX conventions | [docs/PRODUCT_SENSE.md](docs/PRODUCT_SENSE.md) |
| Worktree spawn/remove lifecycle | [docs/design-docs/worktree-lifecycle.md](docs/design-docs/worktree-lifecycle.md) |
| Tab creation & index vs position bug | [docs/design-docs/tab-management.md](docs/design-docs/tab-management.md) |
| Agent status notifications | [docs/design-docs/agent-notifications.md](docs/design-docs/agent-notifications.md) |
| Session resurrection & cwd bug | [docs/design-docs/session-resurrection.md](docs/design-docs/session-resurrection.md) |
| Zellij CLI behaviors (critical) | [docs/design-docs/zellij-behaviors.md](docs/design-docs/zellij-behaviors.md) |
| WASM plugin API & sandbox | [docs/references/zellij-plugin-api.md](docs/references/zellij-plugin-api.md) |
| KDL layout format | [docs/references/zellij-kdl-layout.md](docs/references/zellij-kdl-layout.md) |
| All design docs | [docs/design-docs/index.md](docs/design-docs/index.md) |
