# Session Resurrection

## How Zellij resurrection works

Zellij periodically serializes the session layout to its cache directory. When a session is killed (Ctrl-Q) and later re-attached, Zellij restores the layout from the cached `session-layout.kdl`.

The `serialization_interval` config controls how frequently Zellij snapshots the layout. `zelligent doctor` sets this to 5 seconds (default is 60s, which means quick exits lose the layout).

### Cache paths

- macOS: `~/Library/Caches/org.Zellij-Contributors.Zellij/<version-dir>/session_info/<session>/`
- Linux: `~/.cache/zellij/<version-dir>/session_info/<session>/`

`<version-dir>` is **not stable** across zellij releases: 0.43.1 used the bare
version string (e.g. `0.43.1`), 0.44.x uses `contract_version_<N>` (observed
on disk as `contract_version_1`), and it can drift again on future releases.
Code that needs to find a session's cache dir must glob
(`*/session_info/<session>`) rather than hardcode a version string — see Bug 3.

Two files live under each session's dir:

- `session-metadata.kdl` — display-only, used by `zellij list-sessions`. Refreshed ~1s.
- `session-layout.kdl` — the file resurrection actually reads. Written only
  when the layout is "dirty" (pane count changed etc.), on the
  `serialization_interval` timer.

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

### Bug 3: Stale serialized plugin URL resurrects a broken sidebar (#155/#157/#158)

**Problem:** `session-layout.kdl` stores each plugin's location as a resolved
`file:<path>` URL (config-alias indirection does NOT survive serialization —
zellij resolves aliases before writing the layout, so this can't be worked
around by using an alias). If that path stops holding a valid zelligent
wasm — the install moved from dev to Homebrew or back, the binary was
deleted, `$HOME` moved (container rebuild) — resurrecting the session loads
whatever is now at that path and Zellij shows `ERROR IN PLUGIN … magic
header not detected` in the pane. Because resurrection re-serializes, the
corruption is sticky: every future resurrection and every new tab inherits
the bad URL.

The trap: `zellij list-sessions --short` (what `zelligent`'s existence probe
uses) prints EXITED sessions identically to alive ones. So **zelligent's own
normal startup and spawn flows** — not just a stray manual `zellij attach` —
walk into resurrecting a broken layout whenever a crash, reboot, or
force-quit left an EXITED session behind. Full research, verified against
zellij source and live experiments: issue #155.

**Fix — three parts:**

1. **`reconcile_serialized_session(name, current_plugin_path)` guard
   (`zelligent.sh`, #157).** Called immediately before every flow that can
   attach to a session by name (the no-arg startup path and the spawn
   attach-session path — both probe `zellij list-sessions --short`). Does
   nothing for an alive or nonexistent session. For an EXITED session, it
   greps every `file:` URL out of the session's `session-layout.kdl`
   (deliberately not a KDL parse) and validates each one: file exists, first
   4 bytes are the wasm magic number, and — for URLs whose basename is
   `zelligent-plugin.wasm` — the path matches the currently resolved plugin
   path. Any failure (on any plugin's URL, not just ours — a broken
   third-party plugin URL is just as fatal to resurrection) drops the
   session (`zellij delete-session --force` + removes its cache dirs) and
   prints one line naming the bad path, then falls through to a fresh
   session. No `file:` URLs at all, or an unreadable/missing layout file,
   fails open (session left untouched) — the guard must never block startup
   on ambiguity.
2. **`zelligent doctor` sweep (#157).** Enumerates every serialized session
   across the cache glob (not just the current repo's) and runs the same
   validation. Auto-fixes (deletes + reports) only EXITED sessions whose
   *own* zelligent-plugin URL is stale. Everything else — an alive session
   with a stale URL, or an EXITED session broken only by a third-party
   plugin's URL — is reported with the fix command, never auto-deleted;
   doctor doesn't get to unilaterally kill a live session or clean up a
   plugin that isn't zelligent's.
3. **`nuke` cache glob fix (#158).** `nuke` used to remove
   `<cache_base>/$zellij_version/session_info/$REPO_NAME`, which silently
   no-ops on any zellij whose version-dir name doesn't match (true for every
   0.44.x install, since `zellij --version` reports `0.44.3` but the cache
   dir is `contract_version_1`). Fixed to glob for any dir with a
   `session_info/<name>` entry, same helper the guard and doctor use.

**Not fixed:** rewriting the URL in place to preserve resurrected tabs (v1
always drops and recreates — the session is from an older install anyway, so
fresh is arguably better); an env-var opt-out of resurrection entirely
(`session_serialization false` in the layout is a documented mechanism if
ever needed, but the guard makes it unnecessary for zelligent's own flows).

### Key Zellij source locations

| File | Line | Purpose |
|------|------|---------|
| `zellij-server/src/plugins/wasm_bridge.rs` | 139 | cwd fallback logic |
| `zellij-server/src/plugins/plugin_map.rs` | 237-248 | RunPlugin extraction for serialization |
| `zellij-server/src/plugins/zellij_exports.rs` | 877-882 | get_plugin_ids returns plugin_cwd |
| `zellij-server/src/pty.rs` | 1933-2000 | FillPluginCwd resolves cwd from focused pane |
| `zellij-server/src/lib.rs` | 1827 | zellij_cwd = std::env::current_dir() |
