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

Tabs opened with `zellij action new-tab --layout FILE` must include the sidebar
plugin and any other chrome explicitly in `FILE`. `default_tab_template` only
applies to tabs created without `--layout`.

## Template variables

Custom layouts (`.zelligent/layout.kdl`) are fragment-based pane lists. They use
these template variables, substituted by the CLI at runtime:

| Variable | Replaced with |
|----------|--------------|
| `{{zelligent_sidebar}}` | Embedded sidebar plugin block |
| `{{zelligent_children}}` | Main tab body or literal `children` in `default_tab_template` |
| `{{cwd}}` | Repo root or worktree path, depending on render context |
| `{{agent_cmd}}` | Rendered shell command for the tab |

## Session vs tab layout wrapping

The CLI handles wrapping based on context:
- **Inside Zellij** (`new-tab`): flat panes, no `tab { }` wrapper
- **Outside Zellij** (new session): panes wrapped in `tab name="..." { }`

Custom layouts should contain only the pane content. The CLI adds the outer
`layout { ... }`, `tab { ... }`, and `default_tab_template { ... }` wrappers.
