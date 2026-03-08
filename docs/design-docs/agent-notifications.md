# Agent Status Notifications

## Overview

Zelligent detects agent status (idle/working/awaiting/done) and notifies the user. The system works with any agent that has a hook system (currently Claude Code).

## Pipeline

```
Claude Code hook (Stop/UserPromptSubmit/Notification)
  -> zellij pipe --name zelligent-status --args "event=Stop,tab=$ZELLIGENT_TAB_NAME"
  -> Plugin fn pipe() -> handle_pipe() -> Action::Notify
  -> execute() -> run_command(osascript/afplay)
```

### 1. CLI injects `ZELLIGENT_TAB_NAME`

`zelligent spawn` sets `ZELLIGENT_TAB_NAME` as an env var in the agent pane command (zelligent.sh:497,501). This is inherited by the agent process and all its children, including hooks.

### 2. Claude Code hooks send pipe messages

The Claude Code plugin (`claude-plugin/plugins/zelligent/hooks/hooks.json`) defines hooks for three events:

| Claude Code hook event | Matcher | Pipe event value | Agent status |
|---|---|---|---|
| `Stop` | (none) | `Stop` | `Done` |
| `UserPromptSubmit` | (none) | `Start` | `Working` |
| `Notification` | `permission_prompt` | `PermissionRequest` | `Awaiting` |

Each hook runs: `zellij pipe --name zelligent-status --args "event=<event>,tab=$ZELLIGENT_TAB_NAME"`

### 3. Plugin receives via `fn pipe()`

Pipes arrive via `fn pipe(&mut self, msg: PipeMessage)` -- a separate WASM export, not an event subscription. No `subscribe()` call needed. Requires `PermissionType::ReadCliPipes`.

`zellij pipe` (without `--plugin`) broadcasts to ALL running plugins. The plugin filters by `msg.name == "zelligent-status"` and ignores messages for unknown tabs.

### 4. Plugin sends OS notifications

For `Awaiting` and `Done` statuses, the plugin runs `osascript` to show a macOS notification. For `Awaiting`, it also plays `Glass.aiff` via `afplay`.

Linux support (`notify-send`) is not yet implemented.

## Status model

```rust
enum AgentStatus {
    Idle,       // default, no indicator
    Working,    // green dot
    Awaiting,   // yellow dot
    Done,       // green checkmark
}
```

Statuses are stored in `agent_statuses: BTreeMap<String, AgentStatus>` keyed by tab name. The UI renders a colored indicator next to each worktree in the browse list.

## Installation

`zelligent doctor` handles everything:
- Grants `ReadCliPipes` permission to the Zellij plugin
- Installs the Claude Code plugin via `claude plugin marketplace add` + `claude plugin install zelligent@zelligent`

## PipeMessage format

```rust
PipeMessage {
    source: PipeSource::Cli(pipe_id),
    name: "zelligent-status",
    payload: None,
    args: { "event": "Stop", "tab": "feature-my-branch" },
    is_private: false,
}
```

Plugins in non-active tabs receive all pipe messages unconditionally.
