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

**UI harness tests (require tmux skill):** visual layout, sidebar rendering, keyboard navigation. Plans live in `tests/harness/plans/`. Run by asking Claude to execute a plan; it delegates to the `test-driver` subagent which wraps Zellij in tmux and reads terminal content via `capture-pane`. For manual proofs or ad hoc interaction, use [.claude/skills/tmux/SKILL.md](.claude/skills/tmux/SKILL.md). See [tests/harness/README.md](tests/harness/README.md).

**Manual testing required:** visual appearance, agent command launching, end-to-end spawn/remove from a live Zellij session. See [docs/design-docs/zellij-behaviors.md](docs/design-docs/zellij-behaviors.md) for testing patterns.

## Push gate

A `.claude/hooks/pre-push-block.sh` hook blocks `git push` unless prefixed with `DOCS_VERIFIED=1`. Before pushing, confirm that `docs/` and `AGENTS.md` have been updated or don't need updates, then run `DOCS_VERIFIED=1 git push ...`.

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

Design docs are implementation history and decision context. They may be superseded by newer code or reference docs, so treat them as historical unless they clearly describe the current contract.

## Key constraints

- **Run `bash test.sh` before and after every change.** This is the single test command for the repo.
- **Push gate:** `git push` is blocked unless prefixed with `DOCS_VERIFIED=1`. Confirm `docs/` and `AGENTS.md` are up to date before pushing.
- **Plugin builds require a PATH workaround** for Homebrew Rust. See [docs/BUILD.md](docs/BUILD.md) for the exact incantation.
- **Tab position != tab index** in the Zellij plugin API. Always use name-based tab operations. See [docs/design-docs/tab-management.md](docs/design-docs/tab-management.md).
- **`CLAUDE.md` is a symlink to `AGENTS.md`.** Keep the symlink intact when making changes.

## Skills

A skill is a set of local instructions to follow that is stored in a `SKILL.md` file. Below is the list of skills that can be used. Each entry includes a name, description, and file path so you can open the source for full instructions when using a specific skill.

### Available skills

- pptx: Use this skill any time a .pptx file is involved in any way — as input, output, or both. This includes: creating slide decks, pitch decks, or presentations; reading, parsing, or extracting text from any .pptx file (even if the extracted content will be used elsewhere, like in an email or summary); editing, modifying, or updating existing presentations; combining or splitting slide files; working with templates, layouts, speaker notes, or comments. Trigger whenever the user mentions "deck," "slides," "presentation," or references a .pptx filename, regardless of what they plan to do with the content afterward. If a .pptx file needs to be opened, created, or touched, use this skill. (file: /Users/philipp/.codex/skills/pptx/SKILL.md)
- skill-creator: Guide for creating effective skills. This skill should be used when users want to create a new skill (or update an existing skill) that extends Codex's capabilities with specialized knowledge, workflows, or tool integrations. (file: /Users/philipp/.codex/skills/.system/skill-creator/SKILL.md)
- skill-installer: Install Codex skills into $CODEX_HOME/skills from a curated list or a GitHub repo path. Use when a user asks to list installable skills, install a curated skill, or install a skill from another repo (including private repos). (file: /Users/philipp/.codex/skills/.system/skill-installer/SKILL.md)

### How to use skills

- Discovery: The list above is the skills available in this session (name + description + file path). Skill bodies live on disk at the listed paths.
- Trigger rules: If the user names a skill (with `$SkillName` or plain text) OR the task clearly matches a skill's description shown above, you must use that skill for that turn. Multiple mentions mean use them all. Do not carry skills across turns unless re-mentioned.
- Missing/blocked: If a named skill isn't in the list or the path can't be read, say so briefly and continue with the best fallback.
- How to use a skill (progressive disclosure):
  1) After deciding to use a skill, open its `SKILL.md`. Read only enough to follow the workflow.
  2) When `SKILL.md` references relative paths (e.g., `scripts/foo.py`), resolve them relative to the skill directory listed above first, and only consider other paths if needed.
  3) If `SKILL.md` points to extra folders such as `references/`, load only the specific files needed for the request; don't bulk-load everything.
  4) If `scripts/` exist, prefer running or patching them instead of retyping large code blocks.
  5) If `assets/` or templates exist, reuse them instead of recreating from scratch.
- Coordination and sequencing:
  - If multiple skills apply, choose the minimal set that covers the request and state the order you'll use them.
  - Announce which skill(s) you're using and why (one short line). If you skip an obvious skill, say why.
- Context hygiene:
  - Keep context small: summarize long sections instead of pasting them; only load extra files when needed.
  - Avoid deep reference-chasing: prefer opening only files directly linked from `SKILL.md` unless you're blocked.
  - When variants exist (frameworks, providers, domains), pick only the relevant reference file(s) and note that choice.
- Safety and fallback: If a skill can't be applied cleanly (missing files, unclear instructions), state the issue, pick the next-best approach, and continue.
