# Session Resurrection

## How Zellij resurrection works

Zellij periodically serializes the session layout to its cache directory. When a session is killed (Ctrl-Q) and later re-attached, Zellij restores the layout from the cached `session-layout.kdl`.

The `serialization_interval` config controls how frequently Zellij snapshots the layout. `zelligent doctor` sets this to 5 seconds (default is 60s, which means quick exits lose the layout).

### Cache paths

- macOS: `~/Library/Caches/org.Zellij-Contributors.Zellij/<version>/session_info/<session>/`
- Linux: `~/.cache/zellij/<version>/session_info/<session>/`

## Known bugs

### Bug 1: Doctor grep matches comments (fixed in PR #62)

**Problem:** `zelligent doctor` used `grep -qF 'serialization_interval'` which matched KDL comments like `// serialization_interval 10000`. So `serialization_interval 5` was never added to config, sessions used the 60s default, and quick Ctrl-Q exits had no saved layout to resurrect from.

**Fix:** `grep -v '^\s*//' "$CONFIG" | grep -qF ...` to skip comment lines before checking.

### Bug 2: Plugin gets wrong cwd on resurrection (confirmed, open)

**Problem:** After session resurrection, the WASM plugin gets `$HOME` as its `initial_cwd` instead of the repo directory. This causes a `NotGitRepo` error.

**Root cause chain:**

1. Plugin launched from layout without explicit `cwd` has `PluginConfig.initial_cwd = None`
2. `PluginConfig.initial_cwd` = `None`
3. At runtime, `FillPluginCwd` resolves cwd from the focused pane (works correctly)
4. But layout serialization (`plugin_map.rs:242`) reads the static `plugin_config.initial_cwd` = `None`
5. Saved `session-layout.kdl` has NO cwd for the zelligent plugin pane
6. On resurrection: `RunPlugin.initial_cwd` = `None`
7. Fallback: `wasm_bridge.rs:139` uses `zellij_cwd` = server process `current_dir()` at startup
8. If `current_dir()` is `$HOME` (not the repo), the plugin gets the wrong cwd

**Plugin cwd resolution path:**

```
load() -> get_plugin_ids().initial_cwd
  -> PluginEnv.plugin_cwd
  -> wasm_bridge.rs:139: cwd.unwrap_or_else(|| wasm_bridge.zellij_cwd.clone())
  -> zellij_cwd = std::env::current_dir() when Zellij server starts
```

**Potential upstream fix:** In `plugin_map.rs:242`, fall back to `plugin_env.plugin_cwd` (the runtime-resolved value) when `initial_cwd` is None:

```rust
plugin_config.initial_cwd.clone()
    .or_else(|| Some(running_plugin.store.data().plugin_cwd.clone()))
```

### Plugin recovery mode (NotGitRepo)

When the plugin gets a wrong cwd, it enters `NotGitRepo` mode. This mode offers three keyboard shortcuts:

- `d` — dump the session layout to disk (for debugging)
- `x` — nuke the session (`kill_sessions`, terminates the plugin)
- `q` / `Esc` — close the plugin

### Key Zellij source locations

| File | Line | Purpose |
|------|------|---------|
| `zellij-server/src/plugins/wasm_bridge.rs` | 139 | cwd fallback logic |
| `zellij-server/src/plugins/plugin_map.rs` | 237-248 | RunPlugin extraction for serialization |
| `zellij-server/src/plugins/zellij_exports.rs` | 877-882 | get_plugin_ids returns plugin_cwd |
| `zellij-server/src/pty.rs` | 1933-2000 | FillPluginCwd resolves cwd from focused pane |
| `zellij-server/src/lib.rs` | 1827 | zellij_cwd = std::env::current_dir() |
