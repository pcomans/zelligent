# Zellij Behaviors Reference

For general Zellij CLI usage, see the official documentation:
- [CLI commands](https://zellij.dev/documentation/commands.html)
- [CLI actions](https://zellij.dev/documentation/cli-actions)
- [Session management tutorial](https://zellij.dev/tutorials/session-management/)
- [Session resurrection](https://zellij.dev/documentation/session-resurrection)
- [Controlling Zellij through the CLI](https://zellij.dev/documentation/controlling-zellij-through-cli.html)

This doc covers **hard-won tribal knowledge** about Zellij CLI behaviors that are not in official docs or are surprising. Correct as of Zellij 0.43.1.

## Session creation gotchas

| Command | Actual behavior |
|---------|----------------|
| `zellij --session NAME --layout FILE` | ALWAYS tries to add layout as a new tab to session NAME. Fails with "Session not found" if NAME doesn't exist. **Never creates a new session.** |
| `zellij --new-session-with-layout FILE --session NAME` | ALWAYS creates a new named session. Use this when outside Zellij. |
| `zellij --new-session-with-layout FILE` (inside Zellij) | Creates a nested Zellij (wrong). |
| `session { name "..." }` in layout KDL | Not supported in Zellij 0.43.1. |

## Session name constraints

- Session names must be `[a-zA-Z0-9_-]` only -- no spaces, no slashes.
- The "less than 0 characters" error from Zellij is a display bug masking a name validation failure.

## Tab creation inside Zellij

See [../references/zellij-kdl-layout.md](../references/zellij-kdl-layout.md) for the `tab { }` wrapper and `default_tab_template` gotchas.

## Testing without a live terminal

Zellij supports headless sessions for automated testing. This is not prominently documented:

```bash
# Create a headless background session
zellij attach --create-background SESSION

# Run actions against a specific session from outside it
ZELLIJ_SESSION_NAME=SESSION zellij action new-tab --layout my-layout.kdl --name test

# Inspect the full layout state (use for test assertions)
ZELLIJ_SESSION_NAME=SESSION zellij action dump-layout

# Clean up
zellij kill-session SESSION
```

You can also mock zellij with a fake binary on PATH to test which command the script invokes without blocking.

## `$ZELLIJ` environment variable

- Set by Zellij when running inside a session. Use `[ -n "$ZELLIJ" ]` to detect this.
- Unset it (`ZELLIJ=""`) when testing the outside-Zellij code path.
- NOT available inside WASM plugins (sandbox blocks environment access).
