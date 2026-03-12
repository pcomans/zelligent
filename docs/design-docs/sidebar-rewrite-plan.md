# Sidebar Rewrite Plan

This document now describes the implemented persistent-sidebar model, not the old floating-plugin plan.

## What changed

The old model mixed two separate ideas:

- a floating plugin launched with a keybinding
- script-generated layouts that duplicated chrome in multiple places

That was brittle and already drifting. The persistent-sidebar rewrite replaces it with one layout contract and one rendering pipeline.

## Final model

- Every zelligent-managed session has a mandatory left sidebar
- Layouts come from files, not heredoc KDL in the shell script
- Runtime precedence is repo `.zelligent/layout.kdl`, then `~/.zelligent/layout.kdl`
- The shipped default template is installed by `zelligent doctor` if the user-level file is missing
- Layout files remain full `layout { ... }` documents
- `{{zelligent_sidebar}}` is required exactly once
- `{{cwd}}` and `{{agent_cmd}}` are optional text substitutions
- Unknown placeholders are left untouched

## Why the placeholder exists

Two alternatives were rejected:

1. **Body fragment files**
   - honest but ugly
   - not standalone KDL
   - worse tooling and readability
2. **Pretend full layouts are body-only**
   - hides composition rules in script code
   - guaranteed future confusion

The explicit sidebar placeholder keeps the layout file valid KDL while making the contract obvious.

## Shared rendering pipeline

Plain `zelligent` and `zelligent spawn` now share the same layout pipeline:

1. resolve template path
2. validate required sidebar placeholder
3. substitute placeholders
4. hand the rendered layout to Zellij

The only context-specific inputs are `cwd` and `agent_cmd`.

## Session-startup special case

Session startup still needs one extra Zellij-specific wrapper:

- the first tab must render from the active layout
- the session must also install a `default_tab_template` so future manual tabs inherit the sidebar frame

That is not duplication of product logic. It is a Zellij API constraint.

## Manual tabs

The guaranteed invariant is:

- manual tabs created in a zelligent-managed session inherit the sidebar frame

Exact body parity is desirable but depends on what Zellij allows through `default_tab_template`.

## Doctor behavior

`zelligent doctor` now:

- ensures the plugin asset exists
- ensures the shipped default layout asset exists
- creates `~/.zelligent/layout.kdl` only if missing
- compares the user layout byte-for-byte with the shipped default
- prints a direct overwrite command if they differ

It does not install or manage the old `Ctrl-y` keybinding.
