# G4 live-path validation — sidebar-click-activate (#132)

STATUS: **BLOCKED (environment)** — the running plugin could not be exercised
because the sidebar worktree list does not populate in this devcontainer. The
blocker is in zellij-server's process-spawn, not in the #132 change. Three
independent attempts (a DeepSeek judge, a manual architect run, and the
`test-driver` Claude subagent) all hit the identical wall.

## What was attempted

- Build + install: `dev-install.sh` succeeded; plugin present at
  `$HOME/.local/share/zelligent/zelligent-plugin.wasm` (1344002 bytes).
- Fixture `tests/harness/fixtures/setup-with-worktrees.sh` ran cleanly; git
  worktrees feature-a/b/c registered under
  `$HOME/.zelligent/worktrees/zelligent-test-repo/`.
- Launched `ZELLIGENT_PLUGIN_SRC=... ./zelligent.sh` under tmux (220x60, `mouse on`).
  Tried clean relaunch after `zellij delete-session` + purging
  `~/.cache/zellij/*/session_info/zelligent-test-repo` and killing leftover
  zellij processes.

## Observed failure (capture-pane, plugin pane status line)

```
  /tmp/zelligent-test-repo is not a git repo: No such file or directory (os error 2)

  0.2.3+06fba16
```

The sidebar shows only the startup row (`...t-test-repo`) and the `d/x/q` menu —
no feature-a/b/c rows — so there is no worktree row to click. G4a/G4b cannot be
exercised.

## Root cause (measured, not inferred)

The plugin discovers worktrees by shelling out through zellij-server's
`run_command` to the `zelligent` helper: `zelligent show-repo` (CMD_GIT_TOPLEVEL),
then `zelligent list-worktrees`. `handle_git_toplevel` receives a non-zero exit
with stderr `No such file or directory (os error 2)` — a Rust process-**spawn**
failure (ENOENT), so the helper never runs and `repo_root` is never set.

Yet the exact command zellij-server would spawn works when run directly:

```
$ (cd /tmp/zelligent-test-repo && env -i /home/vscode/.local/bin/zelligent show-repo)
repo_root=/tmp/zelligent-test-repo
repo_name=zelligent-test-repo        # exit 0
```

Confirmed the spawn target is reachable from the server's own view:
- rendered layout sets `zelligent_path "/home/vscode/.local/bin/zelligent"`,
  `cwd="/tmp/zelligent-test-repo"`, `repo_root="/tmp/zelligent-test-repo"`;
- `/proc/<server-pid>/root/home/vscode/.local/bin/zelligent` — present, `0775`;
- `/proc/<server-pid>/root/bin/bash` — present (shebang is `#!/bin/bash`);
- server cwd (`/proc/<server-pid>/cwd`) = `/tmp/zelligent-test-repo`, lists the repo.
- `RunCommands` permission is granted (`~/.cache/zellij/permissions.kdl`).

So zellij-server (0.44.3) fails to spawn a child process that succeeds with the
identical program, args, cwd, and empty environment from a shell. This is a
zellij-server-in-this-container process-spawn defect, orthogonal to issue #132.

## Bearing on the fix

The #132 change is in `handle_mouse_browse`'s click→activate logic, fully covered
by unit gates G1–G3 (the named single-click test plus all plugin tests pass). The
live gate could not run here for an unrelated environmental reason; it should be
re-run on a host where zellij-server can spawn the helper (the worktree list
populates).

DO-NOT-SHIP on the basis of G4 alone is **not** warranted — G4 is BLOCKED, not
FAILED. Recommend a reviewer run the live click-through
(`tests/harness/plans/sidebar-mouse-interaction.md`, Test 4) on real hardware
before merge.

BLOCKED — environmental zellij-server run_command spawn failure prevents the
worktree list from rendering; the #132 logic is unit-proven (G1–G3) but the live
click could not be exercised in this devcontainer.
