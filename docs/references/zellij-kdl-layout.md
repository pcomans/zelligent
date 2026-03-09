# Zellij KDL Layout Reference

For general layout syntax, see the official Zellij documentation:
- [Layouts overview](https://zellij.dev/documentation/layouts.html)
- [Creating a layout](https://zellij.dev/documentation/creating-a-layout.html)
- [Layout examples](https://zellij.dev/documentation/layout-examples)

This doc covers **zelligent-specific layout rules** not found in official docs.

## `new-tab` layouts must be flat

When opening a tab via `zellij action new-tab --layout FILE`, the layout must be a flat pane list wrapped in `layout { }`. No `tab { }` wrapper.

A `tab { }` wrapper causes session-level replacement, breaking existing tabs and chrome. This is not documented upstream.

## `new-tab` does not inherit `default_tab_template`

Any desired chrome/plugins must be included explicitly in the layout file. The built-in zelligent spawned-tab layout includes:

```kdl
pane split_direction="horizontal" {
    pane size="24%" {
        plugin location="file:/.../zelligent-plugin.wasm"
    }
    // agent + lazygit panes
}
pane size=1 borderless=true {
    plugin location="zellij:status-bar"
}
```

Note: zelligent intentionally omits `zellij:tab-bar` from spawned-tab layouts; the sidebar plugin is the primary navigator.

## Session-level `default_tab_template` in zelligent sessions

When zelligent creates a brand-new session (eg. no existing repo session), it includes a `default_tab_template`
in the session layout. This makes manually created tabs (`Ctrl+t n`) inherit the sidebar + status-bar frame.

This does **not** change the `new-tab --layout` rule above: zelligent still includes chrome explicitly for spawned tabs.

## Template variables

Custom layouts (`.zelligent/layout.kdl`) use these template variables, substituted by the CLI at runtime:

| Variable | Replaced with |
|----------|--------------|
| `{{cwd}}` | Worktree path |
| `{{agent_cmd}}` | Agent command (e.g., `claude`) |

## Session vs tab layout wrapping

The CLI handles wrapping based on context:
- **Inside Zellij** (`new-tab`): flat panes, no `tab { }` wrapper
- **Outside Zellij** (new session): panes wrapped in `tab name="..." { }`

Custom layouts should contain only the pane content — the CLI adds the appropriate wrapping.
