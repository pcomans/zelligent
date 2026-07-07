# Tab Management

## Tab creation

The CLI creates tabs using `zellij action new-tab --layout FILE --name NAME`. The layout must be a flat pane list (no `tab { }` wrapper) because `new-tab` provides its own tab context.

Important: `new-tab --layout` does NOT inherit `default_tab_template`. Tab-bar and status-bar plugins must be included explicitly in the layout file.

Manual tabs created the other way — `zellij action new-tab --name X` with no
`--layout` at all — get their content from a separate `new_tab_template` node
instead, since `default_tab_template`'s own `children` marker doesn't fill in
for that case (issue #139). See
[docs/references/zellij-kdl-layout.md](../references/zellij-kdl-layout.md#default_tab_templates-children-marker-doesnt-fill-in-for-bare-new-tab-139).

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

## Duplicate tab names

Because all tab operations are name-based (see above), the plugin has no way to distinguish two tabs that share a name. If a manual tab is created with the same name as an existing worktree tab — e.g. `zellij action new-tab --name feature-a` while a `feature-a` worktree tab is already open — Zellij happily creates a second tab named `feature-a`, but the sidebar shows only **one** `feature-a` row (the worktree row). The duplicate gets no row of its own, and any sidebar action that targets `feature-a` (switch, close) resolves to whichever tab `go_to_tab_name` picks first, so the duplicate is unreachable from the sidebar.

This follows directly from the workaround above: the plugin's tab list is keyed by name, and Zellij's own name-based actions (`go_to_tab_name`) have no concept of "the second tab named X" either. Index-based disambiguation is not an option here for the same reason it was ruled out for close/rename — the plugin cannot obtain a tab's internal index, only its position, and position drifts as tabs open and close (see [Tab index vs position bug](#tab-index-vs-position-bug) above).

No corruption results: closing the duplicate (natively, via `zellij action close-tab` or the tab-mode `x` keybinding) leaves no stale sidebar row, since the sidebar was never tracking it. This is a documented limitation, not a bug to fix — see issue #142 and the repro in [tests/harness/plans/ui-audit-06-repro-verification.md](../../tests/harness/plans/ui-audit-06-repro-verification.md) (R9).

**Guidance:** avoid naming manual tabs after worktree branch names. The sidebar intentionally shows one row per unique tab name; giving a manual tab the same name as a worktree tab is user error, not a plugin defect.

## Plugin tab operations

The sidebar plugin provides:
- **Switch to tab** — `go_to_tab_name(worktree_tab_name)` then closes the plugin
- **Close tab** — uses the name-based workaround above, then refreshes the worktree list
- **Session nuke** — `kill_sessions` (terminates the plugin's own process, nothing after it runs). Note: this only kills the session, it does NOT clean up resurrection cache or stale processes like `zelligent nuke` does. The session may resurrect on next attach.
