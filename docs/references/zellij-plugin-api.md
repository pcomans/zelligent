# Zellij Plugin API Reference

For general plugin API docs, see the official Zellij documentation:
- [Plugin API overview](https://zellij.dev/documentation/plugin-api.html)
- [Events](https://zellij.dev/documentation/plugin-api-events)
- [Commands](https://zellij.dev/documentation/plugin-api-commands.html)
- [Permissions](https://zellij.dev/documentation/plugin-api-permissions)
- [Developing a Rust plugin](https://zellij.dev/tutorials/developing-a-rust-plugin/)

This doc covers **zelligent-specific gotchas** not found in official docs.

## WASM sandbox gotchas

- **No environment variables.** `std::env::var()` returns empty/error. The WASM sandbox does not inherit the host process environment. This means `ZELLIJ_SESSION_NAME` (set for CLI subprocesses) is not available inside plugins.
- **Getting the session name:** subscribe to `EventType::SessionUpdate` and find the current session:

  ```rust
  Event::SessionUpdate(sessions, _resurrectable) => {
      if let Some(info) = sessions.iter().find(|s| s.is_current_session) {
          self.session_name = info.name.clone();
      }
  }
  ```

  `SessionUpdate` signature: `Event::SessionUpdate(Vec<SessionInfo>, Vec<(String, Duration)>)` — first arg is active sessions, second is resurrectable sessions. Requires `PermissionType::ReadApplicationState`.

## Tab index vs position bug

See [design-docs/tab-management.md](../design-docs/tab-management.md) for the full writeup and workaround. Summary: `TabInfo.position` != tab index, so never use position with `close_tab_with_index` or `rename_tab`.

## `kill_sessions` is terminal

`kill_sessions(&[&name])` terminates the plugin's own process. Nothing after this call runs. Use it only as the final action (e.g., session nuke).

## Permissions used by zelligent

Granted automatically by `zelligent doctor`:

| Permission | Used for |
|-----------|----------|
| `ChangeApplicationState` | Tab switching, closing tabs |
| `ReadApplicationState` | Reading tab list, session info |
| `RunCommands` | Executing zelligent CLI commands |
| `ReadCliPipes` | Receiving agent status notifications |
