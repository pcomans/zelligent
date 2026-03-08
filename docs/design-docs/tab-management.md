# Tab Management

## Tab creation

The CLI creates tabs using `zellij action new-tab --layout FILE --name NAME`. The layout must be a flat pane list (no `tab { }` wrapper) because `new-tab` provides its own tab context.

Important: `new-tab --layout` does NOT inherit `default_tab_template`. Any desired plugins/chrome (sidebar plugin, status-bar, tab-bar, etc.) must be included explicitly in the layout file.

## Tab naming

Tabs are named after the sanitized branch name (see [PRODUCT_SENSE.md](../PRODUCT_SENSE.md#session-name-format)). The plugin uses tab names as the primary identifier for all tab operations.

## Tab index vs position bug

This is a critical Zellij API gotcha that affects the plugin.

Each tab has two numbers:
- **Index** — stable, assigned at creation, never changes
- **Position** — visual slot in the tab bar, shifts when earlier tabs are closed

The problem:
- `TabInfo.position` from `TabUpdate` events gives the **position**, not the index
- `close_tab_with_index` and `rename_tab` expect the internal **index**, not the position
- When a tab has been closed earlier in the session, position != index, and you close/rename the wrong tab
- Upstream issue: [zellij-org/zellij#3535](https://github.com/zellij-org/zellij/issues/3535)

### Workaround (used in the plugin)

Use name-based tab operations instead of index-based:

```rust
// To close a tab:
// 1. Save the currently active tab name
// 2. Guard: check the target tab exists
if self.tabs.iter().any(|t| t.name == tab_name) {
    go_to_tab_name(tab_name);
    close_focused_tab();
    // 3. Return to the previously active tab
    go_to_tab_name(previous_tab);
}
```

There is no plugin API to obtain a tab's internal index. The only reliable identifier is the tab name.

## Plugin tab operations

The plugin (persistent sidebar in spawned tabs, Ctrl-Y floating fallback elsewhere) provides:
- **Switch to tab** — `go_to_tab_name(tab_name)`
- **Close tab** — uses the name-based workaround above, then refreshes the worktree list
- **Session nuke** — `kill_sessions` (terminates the plugin's own process, nothing after it runs). Note: this only kills the session, it does NOT clean up resurrection cache or stale processes like `zelligent nuke` does. The session may resurrect on next attach.
