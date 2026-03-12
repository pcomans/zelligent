# Tab Management

## Tab creation modes

Zelligent uses two related but different Zellij mechanisms:

- `zellij action new-tab --layout FILE` for tabs opened right now
- `default_tab_template` for future manual tabs inside a session

This distinction matters. `new-tab --layout` does not inherit `default_tab_template`.

## Spawned tabs

Spawned tabs use the fully rendered active layout. That rendered layout must already contain the sidebar placeholder expansion because Zellij will not add it for us.

## Session startup

When plain `zelligent` creates a new session, it has to do two things:

1. render the first tab from the active layout
2. install a `default_tab_template` so future manual tabs inherit the sidebar frame

That second step is why session startup has an outer session wrapper that `spawn` does not need.

## Manual tabs

The product invariant is:

- manual tabs in a zelligent-managed session must inherit the sidebar frame

The stronger claim:

- manual tabs always get the exact same custom inner body

is only true if Zellij exposes enough control through `default_tab_template`. Right now the sidebar frame is the guaranteed part.

## Tab naming

Tabs are named after sanitized branch names. The plugin uses tab names as the primary identifier for tab operations.

## Tab index vs position bug

This is a critical Zellij API gotcha.

Each tab has two numbers:
- **Index**: stable, assigned at creation
- **Position**: visual slot in the tab list

The problem:
- `TabInfo.position` is the visual position
- `close_tab_with_index` and `rename_tab` expect the internal index
- when earlier tabs close, position and index diverge

The plugin therefore uses name-based tab operations instead.

## Plugin tab operations

The sidebar plugin provides:
- **Switch to tab**: `go_to_tab_name(worktree_tab_name)`
- **Close tab after remove**: use the name-based workaround, then refresh
- **Session nuke**: kill the session from the error/recovery mode
