# Zellij Plugin API Reference

For general plugin API docs, see the official Zellij documentation:
- [Plugin API overview](https://zellij.dev/documentation/plugin-api.html)
- [Events](https://zellij.dev/documentation/plugin-api-events)
- [Commands](https://zellij.dev/documentation/plugin-api-commands.html)
- [Permissions](https://zellij.dev/documentation/plugin-api-permissions)
- [Developing a Rust plugin](https://zellij.dev/tutorials/developing-a-rust-plugin/)

This doc covers **zelligent-specific gotchas** not found in official docs.

## WASM environment

Zellij's WASI context calls `builder.inherit_env()` (plugin_loader.rs:451), so **`std::env::var()` works inside plugins** — they inherit the full host environment. The plugin uses `std::env::var("ZELLIJ_SESSION_NAME")` to get the session name (lib.rs:670).

Zellij also provides a `get_session_environment_variables` plugin command for accessing `session_env_vars` without relying on WASI inheritance, but the plugin doesn't use it.

## Event delivery and hidden panes

Zellij delivers Events (`TabUpdate`, `Key`, `Mouse`, etc.) only to plugin
instances whose pane is in the **visible** tab. A plugin instance in a
hidden tab receives no Events at all — its view of the world freezes at
the moment its tab lost focus, and it catches up via the `TabUpdate` it
receives when its tab becomes active again. Pipes (`zellij pipe`) are the
exception: they broadcast to all instances, hidden or not. Consequences:
per-tab plugin instances (like the sidebar) cannot observe anything that
happens while hidden, and any `run_command` result racing a tab switch may
never reach a now-hidden instance. Self-heal logic must therefore hang off
the catch-up `TabUpdate` (see the tab-set-change refresh trigger in
`handle_tab_update`, [ARCHITECTURE.md](../ARCHITECTURE.md)). Established by
the 2026-07 live verification of issues #138/#140.

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
