# Zellij KDL Layout Reference

For general layout syntax, see the official Zellij documentation:
- [Layouts overview](https://zellij.dev/documentation/layouts.html)
- [Creating a layout](https://zellij.dev/documentation/creating-a-layout.html)
- [Layout examples](https://zellij.dev/documentation/layout-examples)

This doc covers **zelligent-specific layout rules** not found in official docs.

## `split_direction` is counterintuitive — always verify visually

`split_direction="Vertical"` creates a **left/right** (side-by-side) split. `split_direction="Horizontal"` creates a **top/bottom** split. This is the opposite of what many developers expect.

**Rule:** To get a sidebar on the left, use `split_direction="Vertical"`, not `Horizontal`.

```kdl
// CORRECT: sidebar on left, content on right
pane split_direction="Vertical" {
    pane size="24%" { plugin location="file:..." }
    pane { /* content */ }
}

// WRONG: sidebar on top, content below
pane split_direction="Horizontal" {
    pane size="24%" { plugin location="file:..." }
    pane { /* content */ }
}
```

The authoritative reference is Zellij's built-in `strider` layout, which uses `split_direction="Vertical"` for its left file-browser panel. When in doubt, check `~/.config/zellij/layouts/strider.kdl` or run `zellij action dump-layout` on a strider tab.

**QA tip:** A `dump-layout` integration test that asserts `split_direction="vertical"` (and the absence of `split_direction="horizontal"`) will catch this class of bug automatically. Note: `dump-layout` normalizes the value to lowercase regardless of how it was written in the source KDL.

---

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
