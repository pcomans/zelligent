# Agent Instructions

Zelligent spawns AI coding agents into isolated git worktrees, each in its own Zellij tab.
CLI (`zelligent.sh`) + Zellij WASM plugin (`plugin/`).

## Where to look

| Topic | Doc |
|-------|-----|
| Build, test & push instructions | [docs/BUILD.md](docs/BUILD.md) |
| Architecture & file map | [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) |
| Product & UX conventions | [docs/PRODUCT_SENSE.md](docs/PRODUCT_SENSE.md) |
| Code conventions & Zellij gotchas | [docs/CONVENTIONS.md](docs/CONVENTIONS.md) |
| Quality scoring | [docs/QUALITY_SCORE.md](docs/QUALITY_SCORE.md) |
| Worktree spawn/remove lifecycle | [docs/design-docs/worktree-lifecycle.md](docs/design-docs/worktree-lifecycle.md) |
| Tab creation & index vs position bug | [docs/design-docs/tab-management.md](docs/design-docs/tab-management.md) |
| Agent status notifications | [docs/design-docs/agent-notifications.md](docs/design-docs/agent-notifications.md) |
| Session resurrection & cwd bug | [docs/design-docs/session-resurrection.md](docs/design-docs/session-resurrection.md) |
| Zellij CLI behaviors (critical) | [docs/design-docs/zellij-behaviors.md](docs/design-docs/zellij-behaviors.md) |
| WASM plugin API & sandbox | [docs/references/zellij-plugin-api.md](docs/references/zellij-plugin-api.md) |
| KDL layout format | [docs/references/zellij-kdl-layout.md](docs/references/zellij-kdl-layout.md) |
| All design docs | [docs/design-docs/index.md](docs/design-docs/index.md) |

## Key constraints

- **Run `bash test.sh` before and after every change.** This is the single test command for the repo.
- **Push gate:** `git push` is blocked unless prefixed with `DOCS_VERIFIED=1`. Confirm `docs/` and `AGENTS.md` are up to date before pushing.
- **Plugin builds require a PATH workaround** for Homebrew Rust. See [docs/BUILD.md](docs/BUILD.md) for the exact incantation.
- **Tab position != tab index** in the Zellij plugin API. Always use name-based tab operations. See [docs/design-docs/tab-management.md](docs/design-docs/tab-management.md).
- **`CLAUDE.md` is a copy of `AGENTS.md`.** Keep both files identical when making changes.
