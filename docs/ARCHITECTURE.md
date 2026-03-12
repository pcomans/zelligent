# Architecture

Zelligent has two main components:

## CLI (`zelligent.sh`)

The Bash CLI is responsible for:

- **Session management**: create or attach to the repo session
- **Worktree lifecycle**: `spawn` and `remove`
- **Layout resolution and rendering**: choose repo or user layout, validate the sidebar placeholder, substitute runtime values, and render the final KDL
- **Session startup wrapping**: create new sessions with a `default_tab_template` so manual tabs inherit the sidebar frame
- **Doctor**: install permissions, config defaults, and the canonical user layout
- **Nuke**: remove a session and its resurrection state

The CLI resolves the main repo root even when invoked from a worktree by using `git rev-parse --path-format=absolute --git-common-dir`.

## Plugin (`plugin/`)

The Rust plugin is compiled to `wasm32-wasip1` and rendered as a persistent sidebar pane inside zelligent-managed sessions.

It handles:

- **Worktree browsing**: list and select worktrees
- **Branch selection/input**: spawn from an existing branch or typed name
- **Tab switching**: jump to already-open tabs by name
- **Worktree removal**: trigger `zelligent remove` and close the matching tab when needed
- **Agent status**: show working / needs-input / done indicators and forward notifications
- **Recovery tools**: dump layout or nuke the session when the plugin starts in a bad cwd

The sidebar must not close itself during normal use. In a persistent-pane product model, “close the plugin” is the wrong behavior.

## Shared assets

| File | Purpose |
|------|---------|
| `zelligent.sh` | CLI entry point |
| `default-layout.kdl` | Shipped canonical layout template |
| `plugin/src/lib.rs` | Plugin state machine and command dispatch |
| `plugin/src/ui.rs` | Sidebar rendering |
| `test.sh` | Shell integration/unit test suite |
| `dev-install.sh` | Local install script for CLI, plugin, and default layout |
| `claude-plugin/` | Claude Code plugin used for agent status notifications |

## Layout flow

Zelligent uses one layout pipeline for both plain `zelligent` and `zelligent spawn`:

1. Resolve the active template:
   - repo `.zelligent/layout.kdl`
   - else `~/.zelligent/layout.kdl`
   - else fail
2. Validate `{{zelligent_sidebar}}` appears exactly once
3. Substitute placeholders:
   - `{{zelligent_sidebar}}`
   - `{{cwd}}`
   - `{{agent_cmd}}`
4. Hand the rendered layout to Zellij

Context-specific values:

| Command | `{{cwd}}` | `{{agent_cmd}}` |
|---|---|---|
| `zelligent` | repo root | default shell fragment |
| `zelligent spawn` | worktree path | requested agent command fragment |

## Session startup vs new-tab

There are two Zellij mechanisms involved:

- **`zellij action new-tab --layout ...`** opens a tab right now
- **`default_tab_template`** influences future manually created tabs

So new sessions need both:
- the rendered layout for the first tab
- a session wrapper that injects the persistent sidebar frame into future manual tabs

That split is why session startup and `spawn` cannot use the exact same final KDL file, even though they share the same template source and substitution logic.

## Interaction flow

```text
User runs `zelligent`
  -> CLI resolves and renders the active layout
  -> CLI creates a new session with the first tab rendered from that layout
  -> CLI also installs a `default_tab_template` so future manual tabs inherit the sidebar frame

User runs `zelligent spawn feature/foo claude`
  -> CLI creates or reuses the worktree
  -> CLI resolves and renders the same active layout with worktree-specific placeholders
  -> CLI opens a new tab in the current or repo session

Sidebar action inside Zellij
  -> Plugin runs `zelligent list-worktrees`, `list-branches`, `spawn`, or `remove`
  -> CLI performs the git/Zellij work
  -> Plugin refreshes its state
```
