# Zellij Behaviors Reference

For general Zellij CLI behavior, see the official docs:
- [CLI commands](https://zellij.dev/documentation/commands.html)
- [CLI actions](https://zellij.dev/documentation/cli-actions)
- [Session resurrection](https://zellij.dev/documentation/session-resurrection)

This file captures zelligent-specific tribal knowledge.

## Session creation gotchas

| Command | Actual behavior |
|---|---|
| `zellij --session NAME --layout FILE` | Does not create a new session. It expects the session to exist already. |
| `zellij --new-session-with-layout FILE --session NAME` | Creates a new named session with the provided layout. |
| `zellij --new-session-with-layout FILE` inside Zellij | Creates a nested Zellij, which is wrong for zelligent. |

## Layout gotchas

- `new-tab --layout` does not inherit `default_tab_template`
- `default_tab_template` is the only reliable way to make future manual tabs inherit the sidebar frame
- `split_direction="Vertical"` means left/right, not top/bottom
- `dump-layout` normalizes `split_direction` to lowercase in output

See [../references/zellij-kdl-layout.md](../references/zellij-kdl-layout.md) for the placeholder contract and manual-tab limitation.

## Testing without a live terminal

Zellij supports headless sessions for automated checks:

```bash
zellij attach --create-background SESSION
ZELLIJ_SESSION_NAME=SESSION zellij action new-tab --layout my-layout.kdl --name test
ZELLIJ_SESSION_NAME=SESSION zellij action dump-layout
zellij kill-session SESSION
```

That is useful for verifying whether the sidebar frame shows up in fresh sessions and manual tabs.

## Environment signals

- `$ZELLIJ` is set inside a running session
- `ZELLIJ_SESSION_NAME` can target a session for external `zellij action ...` calls
- WASM plugins inherit host environment variables via Zellij's WASI setup
