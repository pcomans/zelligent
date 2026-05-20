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

### Bug 2: Plugin gets wrong cwd on resurrection (worked around in plugin)

**Problem:** After session resurrection, the WASM plugin gets `/` (or `$HOME`) as its `initial_cwd` instead of the repo directory. The plugin then runs `git rev-parse` from that bogus cwd and reports `NotGitRepo`.

**Root cause chain:**

1. Plugin launched via the persistent sidebar layout. Even with `cwd` set on the `plugin {}` block, Zellij's resurrection serializer (`plugin_map.rs:242`) reads static `plugin_config.initial_cwd` and writes the saved `session-layout.kdl` *without* a cwd attribute on the plugin.
2. On resurrection: `RunPlugin.initial_cwd` = `None`.
3. Fallback (`wasm_bridge.rs:139`): `zellij_cwd` = server process `current_dir()` at startup. The Zellij server daemonizes early, so this often ends up as `/`.
4. The plugin's `get_plugin_ids().initial_cwd` returns `/`.

**Plugin cwd resolution path:**

```
load() -> get_plugin_ids().initial_cwd
  -> PluginEnv.plugin_cwd
  -> wasm_bridge.rs:139: cwd.unwrap_or_else(|| wasm_bridge.zellij_cwd.clone())
  -> zellij_cwd = std::env::current_dir() when Zellij server starts
```

**Workaround (`plugin/src/lib.rs::load`):** Pass `repo_root` through the plugin user-config block. That block IS preserved verbatim across resurrection. When `repo_root` is set, it is **authoritative** — the plugin uses it as `initial_cwd` regardless of what the runtime cwd looks like. Runtime cwd is consulted only as a fallback for the manual-launch case where `repo_root` is absent (plugin loaded by hand without `zelligent.sh`). CLI emits `repo_root "<repo>"` from `sidebar_plugin_content` in `zelligent.sh`.

An earlier version of this workaround only fell back to `repo_root` when runtime cwd was `""`, `/`, or `.`. The upstream code path (step 3 above) can leak literally any directory the server happened to start in — `$HOME`, the worktree of another open tab, anything — so there is no finite list of "bogus" values to enumerate. See zelligent issue #105 for the investigation.

**Potential upstream fix (not blocked on us):** In `plugin_map.rs:242`, fall back to `plugin_env.plugin_cwd` (the runtime-resolved value) when `initial_cwd` is None:

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
