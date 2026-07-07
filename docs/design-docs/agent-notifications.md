# Agent Status Notifications

## Overview

Zelligent detects agent status (working/waiting/done) and notifies the user. The system works with any agent that has a hook system (currently Claude Code).

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
| `Notification` | `permission_prompt` | `PermissionRequest` | `NeedsInput` |

Each hook runs: `zellij pipe --name zelligent-status --args "event=<event>,tab=$ZELLIGENT_TAB_NAME"`

### 3. Plugin receives via `fn pipe()`

Pipes arrive via `fn pipe(&mut self, msg: PipeMessage)` -- a separate WASM export, not an event subscription. No `subscribe()` call needed. Requires `PermissionType::ReadCliPipes`.

`zellij pipe` (without `--plugin`) broadcasts to ALL running plugins. The plugin filters by `msg.name == "zelligent-status"`.

### Buffering events for not-yet-known tabs (#141)

No automatic `event=Start` pipe is fired during spawn — neither `zelligent.sh` nor the plugin sends one. The only pipes come from external `zelligent-status` senders (the Claude Code plugin's hooks, see above), and those can race the `TabUpdate` that registers a brand-new tab with a given sidebar instance — so `handle_pipe` sometimes sees an event for a tab that isn't in `self.tabs` yet. Rather than dropping it, the event is stashed in `pending_statuses: BTreeMap<String, PendingStatus>` (latest event per tab wins and resets its age, capped at 16 entries to bound memory) and drained into `agent_statuses` by `handle_tab_update` once that tab appears in a snapshot. An entry that never matches is aged out after `PENDING_STATUS_MAX_TAB_UPDATES` `TabUpdate`s so a stale or mistyped `tab=` value can't get misapplied to an unrelated tab created later with that name. Buffered events never produce a `Notify` — replaying a `NeedsInput`/`Done` notification at `TabUpdate` time would fire it from the wrong context, and the buffered case is overwhelmingly `Start`/`Working`, which never notifies anyway.

### Replaying statuses to late-created instances (#140 part B / Z-6)

`agent_statuses` is per-instance state (see "Status model" below). A sidebar instance created *after* a `zelligent-status` pipe was sent never received it — `zellij pipe` only reaches instances alive at send time — so a tab created after a status event shows no glyph while older tabs still do. Fixed by a request/replay handshake, entirely over the same CLI-pipe channel documented in [zellij-plugin-api.md](../references/zellij-plugin-api.md) ("Event delivery and hidden panes"), since that's the only channel proven to reach hidden instances and the plugin-side `pipe_message_to_plugin`/`MessageToPlugin` API (zellij-tile 0.43) targets one destination plugin, not a broadcast.

1. **Request on permission grant.** The `Event::PermissionRequestResult(Granted)` branch of `update()` calls `fire_status_request()`, which runs `zellij --session <session> pipe --name zelligent-status-request` (no args) — the same `run_command`-via-host-`zellij` pattern as `fire_invalidate_broadcast`. This reaches every sidebar instance in the session, including the sender itself. It must NOT fire from `load()`: the `RunCommands` grant is asynchronous even when `permissions.kdl` already pre-approves the plugin, so a `run_command` issued during `load()` is deterministically denied (`permission 'RunCommands' denied` in zellij.log) and the broadcast silently never happens — found by live verification of the first cut of this fix.
2. **Replay on request.** `handle_pipe` answers a `zelligent-status-request` with `Action::ReplayStatuses(payload)` only if `self.agent_statuses` or `self.pending_statuses` is non-empty (an instance with nothing to offer stays silent — otherwise every load would broadcast noise; an instance holding only buffered early events still has real knowledge and must reply). `payload` is built by `serialize_statuses()`: `agent_statuses` entries plus buffered `pending_statuses` entries (#141), so a late instance catches up on both. The action is executed as a `zelligent-status-replay` pipe carrying one arg, `statuses`, e.g. `statuses=feat-a:Working;feat-b:Done`.
3. **Wire format.** Entries are `tab:code` pairs joined by `;` (`code` is `Idle`/`Working`/`NeedsInput`/`Done`). Worktree tab names are sanitized branch names restricted to `[a-zA-Z0-9_-]` (see the `tr -cd` in `zelligent.sh`), but `zelligent-status` accepts arbitrary `tab=` values, so `serialize_statuses()` skips any name containing `:` or `;` rather than emitting a payload that would fragment into a spurious entry on every receiver. It also caps the payload at `STATUS_REPLAY_MAX_LEN` (4096 bytes), dropping only whole trailing entries so nothing is corrupted mid-entry — a defensive bound, not expected to bite any real session.
4. **Monotone merge on receipt.** `handle_pipe`'s `zelligent-status-replay` branch never overwrites a tab it already has an opinion about: an entry is applied only if `agent_statuses` has no key for that tab. If the tab is known (in `self.tabs`) it goes straight into `agent_statuses`; if not, it's routed through the exact same `pending_statuses` buffer semantics as a live event (#141) — capped at 16, and only if there isn't already a buffered entry for that tab. This produces no `Notify` and no `status_message` under any circumstance; replay is a silent catch-up, never a user-visible event.
5. **Loop safety.** Handling a request returns at most one `ReplayStatuses` action (never another request). Handling a replay always returns `Action::None` (never a request or another replay). Because the merge is monotone, an instance receiving its own broadcast — replies included — is a no-op, and multiple instances answering the same request is harmless.

See `plugin/src/lib.rs` (`fire_status_request`, `fire_status_replay`, `serialize_statuses`, `parse_statuses`, and the `zelligent-status-request`/`zelligent-status-replay` branches in `handle_pipe`) for the implementation, and its `#[cfg(test)]` module (tests named `status_request_*` / `status_replay_*` / `serialize_parse_statuses_round_trip`) for the request/replay/merge/loop-safety/cap coverage.

### 4. Plugin sends OS notifications

For `NeedsInput` and `Done` statuses, the plugin runs `osascript` to show a macOS notification. For `NeedsInput`, it also plays `Glass.aiff` via `afplay`.

Linux support (`notify-send`) is not yet implemented.

## Status model

```rust
enum AgentStatus {
    Idle,       // default, no indicator
    Working,    // green dot
    NeedsInput, // yellow dot
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
