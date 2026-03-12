# Zellij KDL Layout Reference

For general syntax, see the official Zellij docs:
- [Layouts overview](https://zellij.dev/documentation/layouts.html)
- [Creating a layout](https://zellij.dev/documentation/creating-a-layout.html)
- [Layout examples](https://zellij.dev/documentation/layout-examples)

This document covers zelligent-specific layout rules.

## Runtime layout precedence

Whenever zelligent needs a layout, it resolves:

1. repo `.zelligent/layout.kdl`
2. user `~/.zelligent/layout.kdl`
3. hard error if neither exists

`zelligent doctor` creates `~/.zelligent/layout.kdl` from the shipped default only if that file is missing.

## Contract: full layout file, not a fragment

`.zelligent/layout.kdl` must be a complete `layout { ... }` document.

Supported placeholders:

| Placeholder | Rule | Expansion |
|---|---|---|
| `{{zelligent_sidebar}}` | Required exactly once | Left sidebar plugin pane |
| `{{cwd}}` | Optional | Repo root or worktree path depending on context |
| `{{agent_cmd}}` | Optional | Shell command fragment for `bash -c` |

Unknown placeholders are left untouched.

## `{{agent_cmd}}` is a shell fragment

This trips people up.

`{{agent_cmd}}` is not guaranteed to be a plain executable name like `claude`. On freshly created worktrees it may expand to:

- exported tab metadata
- setup hook preamble
- final `exec ...`

If you want zelligent-managed setup behavior, use it inside a shell wrapper:

```kdl
pane command="bash" cwd="{{cwd}}" {
    args "-c" "{{agent_cmd}}"
}
```

If you use `command="{{agent_cmd}}"` directly, you are on your own.

## `new-tab` does not inherit `default_tab_template`

`zellij action new-tab --layout FILE` does not inherit the session's `default_tab_template`.

That means spawned tabs must render the sidebar directly into the layout being opened right now. Session startup is different: it must render the initial tab and also install a session-level `default_tab_template` for future manual tabs.

## Manual-tab limitation

Zelligent guarantees that manual tabs created inside a zelligent-managed session inherit the sidebar frame.

Full body parity with the active custom layout is best-effort. Zellij's `default_tab_template` mechanism controls the frame reliably, but not every part of the inner tab body.
