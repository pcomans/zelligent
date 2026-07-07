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

## `default_tab_template`'s `children` marker doesn't fill in for bare `new-tab` (#139)

`default_tab_template`'s `{{zelligent_children}}` substitution (the bare
`children` keyword) is only filled in when Zellij merges an **explicit** tab
body into the template at layout-parse time — that's how the session's own
initial `tab { }` block gets its shell+lazygit panes.

A tab created later via `zellij action new-tab --name X` with **no**
`--layout` has no explicit body to merge in. Zellij's fallback for that case
(`Layout::new_tab()` in zellij-utils) does not recurse into nested panes to
find the `children` marker — it only fills a marker that is a *direct* child
of `default_tab_template`'s root. Since zelligent's `children` marker sits one
level deep (inside the sidebar's `pane split_direction="Vertical" { ... }`
wrapper), the fallback fill is a no-op and the tab renders as the sidebar
alone, full width, with no shell pane.

The fix is a separate KDL node, `new_tab_template { ... }`, which Zellij
parses like a literal `tab { }` (no `children`-marker merge at all) and
prefers over `default_tab_template` specifically for this no-layout case. The
CLI writes both: `default_tab_template` still wraps the session's own
explicit initial tab, and `new_tab_template` carries real, literal
sidebar+shell+lazygit content (generic — no worktree cwd or agent command,
since neither is known for a tab created later by the user) for every manual
tab created afterward. See `write_session_layout` in `zelligent.sh`.

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
