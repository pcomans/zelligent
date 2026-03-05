# Zelligent Agent Notification Design

## Context

Zelligent runs as a Zellij plugin. Goal: detect agent status (working/waiting/done) and notify the user. Support any agent that has a hook system.

## Architecture

```
Agent hook → zellij pipe --name zelligent-status --args "event=Stop,tab=$ZELLIGENT_TAB_NAME"
  → Plugin fn pipe() → handle_pipe() (pure) → Action::Notify
  → execute() → run_command(osascript/afplay)
```

### Agent → Plugin Communication via CLI Pipes

Agent hook scripts run `zellij pipe` to notify the plugin. The plugin receives this in `fn pipe()`, filters by `name == "zelligent-status"`, and updates internal state.

**Events:**
- `Start` / `UserPromptSubmit` → `AgentStatus::Working`
- `PermissionRequest` → `AgentStatus::NeedsInput`
- `Stop` → `AgentStatus::Done`

**How the hook knows the tab name:** `zelligent spawn` injects `ZELLIGENT_TAB_NAME` as an env var into the agent pane command. This is inherited by the agent process and all its children (including hooks).

### Zellij Plugin API — Key Findings

**CLI Pipes:**
- Pipes arrive via `fn pipe(&mut self, msg: PipeMessage)` — a separate WASM export, NOT an event subscription. No `subscribe()` call needed.
- `zellij pipe` (no --plugin) broadcasts to ALL running plugins (`is_private = false`)
- Permission: `PermissionType::ReadCliPipes`

**PipeMessage struct:**
```rust
PipeMessage {
    source: PipeSource::Cli(pipe_id),
    name: String,               // from --name flag
    payload: Option<String>,    // data chunk; None = EOF
    args: BTreeMap<String, String>, // from --args key=value
    is_private: bool,
}
```

**Background delivery:** Plugins in non-active tabs receive ALL events and pipe messages unconditionally. No tab-visibility filtering exists.

**OS Notifications:** No built-in API. Use `run_command` with `osascript` (macOS) / `notify-send` (Linux).

### Status Model

```rust
enum AgentStatus {
    Idle,       // "  " (two spaces, preserves alignment)
    Working,    // green ●
    NeedsInput, // yellow ●
    Done,       // green ✓
}
```

### Notification Suppression

Active-tab notification suppression is **not yet implemented**. Currently, OS notifications are emitted regardless of which tab is active. The status map is always updated. A future enhancement could skip the OS notification when the user is already viewing the agent's tab.

### Platform Support

OS notifications currently use `osascript` (macOS only). Linux support (`notify-send`) is not yet implemented.

## Implementation

### Plugin (`plugin/src/lib.rs`)
- `AgentStatus` enum + `agent_statuses: BTreeMap<String, AgentStatus>` in `State`
- `Action::Notify { tab_name, status }` variant
- `handle_pipe()` — pure method, fully testable
- `pipe()` wiring in `ZellijPlugin` impl
- `execute()` fires `osascript` notification + `afplay` Glass.aiff sound for `NeedsInput`

### UI (`plugin/src/ui.rs`)
- `status_indicator()` helper returns ANSI-colored indicator string
- `render_worktree_list` accepts `&BTreeMap<String, AgentStatus>`, maps branch → tab name via `sanitize_tab_name()` which replaces `/` with `-` and strips characters outside `[A-Za-z0-9_-]` (matching `zelligent.sh`)

### Shell (`zelligent.sh`)
- `ZELLIGENT_TAB_NAME` env var injected into agent pane command (both setup.sh and direct paths)
- Doctor: `ReadCliPipes` added to permissions block
- Doctor: Claude Code hooks auto-installed into `~/.claude/settings.json` (Stop, UserPromptSubmit, Notification/permission_prompt)

## Resolved Design Decisions

- **Pipes over PaneRenderReport:** Structured data, no fragile text parsing, works with any agent that has hooks
- **Broadcast pipes (no --plugin):** Simpler hook scripts — no need to know plugin URL at install time
- **Tab name as key:** Already the established pattern in the codebase (see CLAUDE.md tab indexing notes)
- **`zelligent doctor` installs hooks:** Rather than a separate `zelligent hooks install` command, hooks are set up as part of the doctor flow
