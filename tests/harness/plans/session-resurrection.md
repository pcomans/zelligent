---
fixture: setup-with-worktrees.sh
launch: zelligent  # INSTALLED CLI — never the fixture clone's ./zelligent.sh (old main; see README "CLI under test")
session_name: zelligent-test-repo
---

# Session Resurrection — serialized-layout lifecycle

Zellij serializes sessions under `~/.cache/zellij/<contract>/session_info/<name>/`.
Two files live there: `session-metadata.kdl` (session-manager display only) and
`session-layout.kdl` — resurrection reads ONLY the latter (zellij-utils
`sessions.rs`; see the #155 design comment). EXITED sessions resurrect by name,
and `zellij list-sessions --short` prints EXITED sessions as bare names, so
zelligent's own existence probe can attach into a resurrection. This plan covers
the flows the 2026-07 audit never exercised (drivers always `delete-session`d):
clean resurrection, resurrection after an unclean death, and the
stale-plugin-path footgun behind issue #155 (magic-header error: a script's
`#!/b` bytes where `\0asm` was expected).

Conventions: tmux socket `zt-driver-test`, session `zt-driver`, `view` 220x60 +
`ctrl`; captures per step (plain + ANSI) to the run's archive dir; never
`pkill zellij`; check `zellij.log` for `magic header` after each phase.

## R1 — Clean resurrection after hard server death
- Launch `zelligent` in the fixture repo; confirm footer version; note the
  serialized plugin reference: `grep -o 'file:[^"]*' ~/.cache/zellij/*/session_info/zelligent-test-repo/session-layout.kdl`
  (expect the installed plugin wasm path; the file appears within ~5s of launch).
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
- Hard-kill the server. Tamper the serialized LAYOUT (not metadata — only
  `session-layout.kdl` feeds resurrection):
  `sed -i 's|file:[^"]*zelligent-plugin.wasm|file:'"$HOME"'/.local/bin/zelligent|' ~/.cache/zellij/*/session_info/zelligent-test-repo/session-layout.kdl`
  (points the plugin at the CLI script — `#!/b…`, exactly the #155 bytes).
- Run `zelligent` from the repo dir (its existence probe matches the EXITED
  name via `list-sessions --short` and attaches → resurrection).
- Expected once the #155 guard is implemented: zelligent prints its one-line
  stale-session message, deletes and recreates the session, sidebar healthy,
  and NO new `magic header` line in zellij.log — record FIXED. Pre-guard
  expected: the plugin pane shows the magic-header error (REPRODUCED — this
  is the verified end-to-end repro; note that zellij then RE-SERIALIZES the
  broken layout, making the corruption sticky).
- Cleanup: `zellij kill-session` + `delete-session --force` the session
  regardless of outcome (a tampered session must not leak into later plans).

## R4 — Spawn-flow guard variant
- Recreate the tampered EXITED state of R3. From OUTSIDE any zellij session,
  run `zelligent spawn feature-a bash` in the repo dir (spawn's attach-session
  path must hit the same guard before attaching).
- SAFETY NOTE: this is the ONE legitimate `zelligent spawn` from outside Zellij.
  The ui-audit plans forbid it because there it would attach a SECOND mirrored
  client into a LIVE session and leak keystrokes; here the target session is
  EXITED (dead) and spawn's attach-session path is precisely what must trip the
  #155 guard, so it cannot be replaced with a sidebar spawn. Run it as the SOLE
  client — do not have another window attached at the same time.
- Expected with the guard: stale session dropped + recreated, spawn lands in
  a healthy session. Pre-guard: same magic-header error.

## R5 — Alive session is left alone
- With a HEALTHY session running, run `zelligent` again from the repo dir.
- PASS if it attaches normally with no stale-session message and no
  delete/recreate (the guard must only act on EXITED sessions; `zelligent
  doctor` may warn, never kill, on alive sessions).

## R6 — Nuke recovery
- Recreate the tampered state of R3 (if R3 reproduced). From inside a broken
  or healthy session, use the sidebar's nuke binding (`x` per footer hint) or
  `zelligent nuke` from the repo dir.
- PASS if the session is gone from `zellij list-sessions` AND the serialized
  cache dir for it is removed/replaced, and a subsequent `zelligent` launch
  is fully healthy.
