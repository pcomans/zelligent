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
receives when its tab becomes active again. CLI pipes (`zellij pipe`
without a `--plugin` target) are the exception, and the **only channel
that reaches hidden instances**: they broadcast to every plugin instance,
hidden or not. (The plugin-side `pipe_message_to_plugin` /
`MessageToPlugin` API is not a broadcast — it targets one destination
plugin by id/url and may launch a new instance on a miss — so
cross-instance broadcast goes through `run_command` invoking the host
`zellij pipe` instead.)

Consequences: per-tab plugin instances (like the sidebar) cannot observe
anything that happens while hidden, and any `run_command` result racing a
tab switch may never reach a now-hidden instance. Self-heal logic
therefore combines two patterns (see `handle_tab_update` / `handle_pipe`
and [ARCHITECTURE.md](../ARCHITECTURE.md)):

- **Catch-up TabUpdate**: diff the woken instance's stale state against
  the first `TabUpdate` it receives on becoming visible (the
  tab-set-change refresh trigger). Blind to changes that net out to zero
  drift.
- **Dirty bit over pipes**: writers broadcast an invalidation pipe
  (`zelligent-invalidate`); every instance durably marks its cache dirty
  and attempts a refresh immediately. Hidden instances lose the refresh
  *result*, so the bit is retried on every `TabUpdate` until a successful
  refresh clears it — the retry that succeeds is the one fired while
  visible. This is the only pattern that catches a spawn+remove
  round-trip completed entirely inside an instance's blind window.

Established by the 2026-07 live verification and instrumentation of issues
#138/#140 (hidden instances logged zero events across the entire window;
a visible instance's `run_command` refresh completed in 8ms).

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
