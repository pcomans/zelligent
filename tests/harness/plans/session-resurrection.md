---
fixture: setup-with-worktrees.sh
launch: zelligent  # INSTALLED CLI — never the fixture clone's ./zelligent.sh (old main; see README "CLI under test")
session_name: zelligent-test-repo
---

# Session Resurrection — serialized-layout lifecycle

Zellij serializes sessions under `~/.cache/zellij/<contract>/session_info/<name>/`
(`session-metadata.kdl`, including each plugin pane's `plugin_url`) and resurrects
EXITED sessions by name. This plan covers the flows the 2026-07 audit never
exercised (drivers always `delete-session`d): clean resurrection, resurrection
after an unclean death, and the stale-`plugin_url` footgun behind issue #155
(magic-header error: a script's `#!/b` bytes where `\0asm` was expected).

Conventions: tmux socket `zt-driver-test`, session `zt-driver`, `view` 220x60 +
`ctrl`; captures per step (plain + ANSI) to the run's archive dir; never
`pkill zellij`; check `zellij.log` for `magic header` after each phase.

## R1 — Clean resurrection after hard server death
- Launch `zelligent` in the fixture repo; confirm footer version; note the
  serialized `plugin_url` lines: `grep plugin_url ~/.cache/zellij/*/session_info/zelligent-test-repo/session-metadata.kdl`
  (expect exactly one `file:` URL = the installed plugin wasm).
- Hard-kill the server: `kill -9` the `zellij --server …/<session>` process
  (this simulates a crash/reboot; `kill-session` would clean up too much).
- `zellij list-sessions` → session shows `EXITED - attach to resurrect`.
- `zellij attach zelligent-test-repo` from the view window.
- PASS if the sidebar renders (header, rows, footer version) with no
  `ERROR IN PLUGIN` pane and no new `magic header` line in zellij.log.

## R2 — zelligent startup over an EXITED session
- Hard-kill the server again. Run `zelligent` (not `zellij attach`) from the
  repo dir.
- PASS if a working session comes up (sidebar healthy) — record whether it
  resurrected the serialized layout or created fresh (compare tab set with
  pre-kill `query-tab-names`). Either is acceptable; a broken plugin pane is a FAIL.

## R3 — Stale plugin_url footgun (issue #155)
- Hard-kill the server. Tamper the serialized metadata:
  `sed -i 's|file:.*zelligent-plugin.wasm|file:'"$HOME"'/.local/bin/zelligent|' <metadata-file>`
  (points the plugin at the CLI script — `#!/b…`, exactly the #155 bytes).
- `zellij attach zelligent-test-repo`.
- Record the outcome precisely:
  - If the plugin pane shows the magic-header error: REPRODUCED — the #155
    footgun. If `zelligent` (issue #155's guard, once implemented) instead
    detects and deletes the stale session with a message, that's FIXED.
  - If zellij starts fresh and self-heals (re-serialized URL correct), record
    NOT-REPRODUCED-VIA-THIS-PATH and note which flow was used.
- Cleanup: `zellij kill-session` + `delete-session --force` the session
  regardless of outcome (a tampered session must not leak into later plans).

## R4 — Nuke recovery
- Recreate the tampered state of R3 (if R3 reproduced). From inside a broken
  or healthy session, use the sidebar's nuke binding (`x` per footer hint) or
  `zelligent nuke` from the repo dir.
- PASS if the session is gone from `zellij list-sessions` AND the serialized
  cache dir for it is removed/replaced, and a subsequent `zelligent` launch
  is fully healthy.
