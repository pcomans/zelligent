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

**`zellij pipe` BLOCKS the calling process until a plugin consumes the
message** (#167): with sidebars loaded that can take up to zellij's ~1s
CliPipe dispatch timeout (the `Action CliPipe did not complete within 1s
timeout` log line), and in a session with NO consuming plugin it blocks
indefinitely. Never call it synchronously on a latency-sensitive or
unconditional path — background it under a timeout (`run_with_timeout N
zellij pipe … &`, see `pipe_invalidate` in zelligent.sh, and the
backgrounded hook commands in claude-plugin). Inside the plugin,
`run_command`-launched pipes don't block the plugin itself (the host
runs them async), only the spawned process.

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
  "Successful" is generation-gated (`invalidate_generation`, bumped per
  invalidation and stamped into the refresh's `run_command` context) so a
  refresh already in flight when a newer invalidation lands cannot clear
  that invalidation's bit — see [ARCHITECTURE.md](../ARCHITECTURE.md).

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
