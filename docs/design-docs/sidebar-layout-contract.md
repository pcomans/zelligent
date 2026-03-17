# Sidebar Layout Contract

This document describes the layout contract implemented by the persistent
sidebar CLI pipeline in PR 83 and inherited by the later sidebar PRs.

## Summary

Zelligent resolves one layout fragment, validates a small placeholder contract,
renders it with the current repo or worktree context, and then reuses that same
rendered fragment for:

- new session startup
- `zelligent spawn` tabs
- `default_tab_template` for manual tabs created later in the session

The contract is intentionally fragment-based. `.zelligent/layout.kdl` is not a
full `layout { ... }` document. It is the pane list that belongs inside a tab.

## Layout Source Order

Whenever zelligent needs a layout fragment, it resolves sources in this order:

1. repo-local `.zelligent/layout.kdl`
2. user-level `~/.zelligent/layout.kdl`
3. hard error if neither exists

The shipped default fragment is installed to:

- Homebrew: `share/zelligent/default-layout.kdl`
- Dev install: `~/.local/share/zelligent/default-layout.kdl`

`zelligent doctor` creates `~/.zelligent/layout.kdl` from that shipped default
only when it is missing.

## Placeholder Contract

The supported placeholders are:

- `{{zelligent_sidebar}}` required exactly once
- `{{zelligent_children}}` required exactly once
- `{{cwd}}` optional
- `{{agent_cmd}}` optional

Unknown placeholders are left untouched.

`{{zelligent_sidebar}}` expands to the embedded plugin block only. The layout
fragment owns the surrounding `pane ... { ... }` wrapper, so custom layouts can
control sidebar geometry and pane naming.

`{{zelligent_children}}` is the insertion point for the main tab body:

- startup and spawn tabs receive the default shell+lazygit body unless the
  layout fragment wraps or repositions it
- `default_tab_template` receives literal `children`, which means manual tabs
  inherit the same frame even if Zellij supplies the inner content

## Cwd and Agent Semantics

- Plain `zelligent` startup renders with `cwd = repo root` and `agent_cmd =
  $SHELL`.
- `zelligent spawn` renders the explicit tab with `cwd = worktree path` and
  `agent_cmd = explicit command or $SHELL`.
- The `default_tab_template` for future manual tabs renders with `cwd = repo
  root`.

## Session Behavior

- Startup fails if the sidebar plugin cannot be resolved.
- Startup fails if no repo-local or user-level layout fragment can be resolved.
- Every zelligent-managed session includes a `default_tab_template` derived from
  the same fragment source as the initial tab.
- Manual tabs therefore inherit the same sidebar frame and overall pane
  structure, even if Zellij still controls the default `children` content.

## Validation

Runtime validation deliberately stays narrow:

- enforce the required placeholders exactly once
- do not attempt broader KDL schema validation
- treat the shipped default fragment as the canonical comparison source for
  `zelligent doctor`
