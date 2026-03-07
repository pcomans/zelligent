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

Tab-bar and status-bar plugins must be included explicitly in the layout file. Every zelligent layout includes:

```kdl
pane size=1 borderless=true {
    plugin location="zellij:tab-bar"
}
// ... content panes ...
pane size=1 borderless=true {
    plugin location="zellij:status-bar"
}
```

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
