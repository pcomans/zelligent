---
fixture: setup-with-worktrees.sh
launch: zelligent  # INSTALLED CLI — never the fixture clone's ./zelligent.sh (old main; see README "CLI under test")
session_name: zelligent-test-repo
---

# UI Audit 05 — Agent Status Glyphs & Non-Browse Modes

Fake agents + status pipe events; glyph rendering interacting with selection
and active-tab highlight; mouse no-ops in SelectBranch/InputBranch/Confirming
modes. Hunts: glyph on wrong row, glyph lost on tab switch, mode leaks where
clicks still act.

Harness window: `tmux -L zt-driver-test new-session -d -s zt-driver -n view -x 220 -y 60 -c /tmp/zelligent-test-repo`

## Evidence & scrutiny rules (MANDATORY)

Same as ui-audit-01, with `ARCHIVE=/tmp/zelligent-ui-run/05-status-modes`. Plain+ANSI per step; quote lines; FAIL ⇒ repro-NN.sh; anomalies section. REAL input for all sidebar interaction; ctrl-window commands allowed ONLY where a step says so (pipe events simulate the real agent-hook mechanism — that mechanism is what's being tested). Sleep 1s after input, 8s after spawns.

Mouse encoding: left click `\033[<0;COL;ROWM\033[<0;COL;ROWm`, wheel `\033[<64/65;COL;ROWM`, COL=10, via `tmux send-keys -l "$(printf ...)"`.

ANSI glyph markers: working = `\x1b[32m●`, needs-input = `\x1b[33m●`, done = `\x1b[32m✓`, at the RIGHT end of the title line, worktree rows only.

## Harness corrections from run 01 (READ FIRST — these override the steps below)

1. MOUSE SETUP: before any click, run `tmux -L zt-driver-test set-option -g mouse on`; send press and release as SEPARATE send-keys calls. Wheel events need no setup.
2. THERE IS NO TAB BAR. Test 6's "tab-bar click zelligent-test-repo" becomes: press `C-t` then digit `1` (real keystrokes, repo tab is position 1). Verify active tab via the MAIN pane's frame title and the sidebar's bold-cyan row.
3. MAPPING CONTRACT (the run-01 subtitle-offset bug is FIXED — #135; contract: `docs/PRODUCT_SENSE.md` § "Sidebar interaction contract"): a subtitle line maps to the SAME item as its title, and a single click on EITHER line selects AND activates that item (#137) — spawn or tab switch. For select/focus-only purposes, use wheel or keyboard instead of clicking an item row, or accept the activation. Blank-separator, header, and footer clicks are no-ops.
4. The ▌ gutter renders on BOTH lines of the selected item — correct, not a finding. The in-pane header line (` <repo> ` + `─` fill, bold cyan) DOES render in a tall pane (#156; fixed since run 01 — don't report its presence as a deviation).
5. Keyboard keys (`n`, `i`, `d`, Esc, typed characters) go to the SIDEBAR pane — it must have focus. Focus it by clicking a no-op line (header, blank separator, or footer) — any item-row click would activate that row and a resulting tab switch moves keyboard focus to the new tab's main pane.
6. OOB-SPAWN SAFETY (ctrl window): a bare `./zelligent.sh spawn` outside Zellij ends in `zellij attach`, turning ctrl into a second mirrored client whose keystrokes leak into the live session. The ctrl-window spawn step below therefore forces the CLI's non-attaching `inside-zellij` mode with the env prefix `ZELLIJ=1 ZELLIJ_SESSION_NAME=zelligent-test-repo` — the CLI then runs `zellij action new-tab` plus a backgrounded pipe, both non-attaching and safe from ctrl. Never run a ctrl-window spawn without this prefix; if one attaches by accident, kill the ctrl pane immediately and open a fresh ctrl shell before ANY further ctrl command.

## Test 1: Spawn a tab with a fake long-running agent
- Action: via ctrl window (note 6 — the `ZELLIJ=1 ZELLIJ_SESSION_NAME=...` prefix forces the non-attaching spawn path; NEVER omit it from ctrl): `cd /tmp/zelligent-test-repo && ZELLIJ=1 ZELLIJ_SESSION_NAME=zelligent-test-repo ZELLIGENT_PLUGIN_SRC="$HOME/.local/share/zelligent/zelligent-plugin.wasm" ./zelligent.sh spawn fake-agent 'bash -c "echo agent running; sleep 600"'`; wait 8s; capture.
- Expected: tab `fake-agent` opens; its main pane shows `agent running`; sidebar row `fake-agent` / `branch: fake-agent` (not `user tab` after self-heal); no glyph yet (no status events sent).

## Test 2: Start event renders a green working dot
- Action: via ctrl window: `zellij --session zelligent-test-repo pipe --name zelligent-status --args "event=Start,tab=fake-agent"`; wait 2s; capture ANSI.
- Expected: `fake-agent` title line ends with green `●`; NO glyph on any other row; row still 2 lines.

## Test 3: Glyph coexists with cursor and active highlight
- Action: click the `fake-agent` row once (`fake-agent` is already the active tab, so this selects and re-activates idempotently — no new tab; if the sidebar was unfocused the first click is the focus claim, #189), capture ANSI.
- Expected: the row shows ▌ (cyan) + bold-cyan title (it's the active tab) + green ● simultaneously — three independent axes on one row, none clobbering another.

## Test 4: PermissionRequest turns the dot yellow
- Action: ctrl window: `zellij --session zelligent-test-repo pipe --name zelligent-status --args "event=PermissionRequest,tab=fake-agent"`; wait 2s; capture ANSI.
- Expected: glyph now yellow `●` on `fake-agent` only.

## Test 5: Stop renders a green check
- Action: ctrl window: pipe `event=Stop,tab=fake-agent`; wait 2s; capture ANSI.
- Expected: glyph now green `✓`.

## Test 6: Glyph persists across tab switches
- Action: tab-bar click `zelligent-test-repo`; capture ANSI.
- Expected: this tab's sidebar shows `fake-agent` with the green ✓ still present; `fake-agent` title no longer bold cyan (not active); `local` bold cyan.

## Test 7: Unknown event → red status message, glyph unchanged
- Action: ctrl window: pipe `event=Bogus,tab=fake-agent`; wait 2s; capture ANSI.
- Expected: red status `Unknown agent event: Bogus`; ✓ unchanged on the row.

## Test 8: Event for unknown tab is ignored
- Action: ctrl window: pipe `event=Start,tab=no-such-tab`; wait 2s; capture.
- Expected: NO row changes, no glyph anywhere new. The red `Unknown agent event: Bogus` status from Test 7 is STILL showing — error statuses persist until a newer status or a real Key/Mouse interaction with the sidebar clears them (#186), and a ctrl-window pipe is neither. Do not read this as "no status change (self-cleared already)"; it is specifically "same red status, unchanged".

## Test 9: Glyphs never appear on local/user rows
- Action: ctrl window: `zellij --session zelligent-test-repo action new-tab --name scratch`; wait 3s; pipe `event=Start,tab=scratch`; wait 2s; capture ANSI. NOTE: `new-tab` switches the session to the new `scratch` tab, so this capture shows the NEW tab's own sidebar instance.
- Expected: `scratch` row (`user tab`) shows NO glyph even though a tab named scratch exists — from code, status only renders for rows with a matched branch. Record what actually happens; a glyph on the user row = FAIL. The Test 7 red status is NOT expected in this capture: the banner is per plugin instance, and this brand-new instance never received the Test 7 pipe (the #140/Z-6 status replay covers agent-status glyphs only, not the banner). Its absence here is correct behavior, not a persistence failure.
- Sub-step 9b (persistence check on the ORIGINAL instance): switch back to the `zelligent-test-repo` tab via real keystrokes (`C-t` then its digit — a native tab switch, not a sidebar interaction); capture ANSI as 09b.
- Expected (9b): the repo tab's pre-existing sidebar instance STILL shows the red `Unknown agent event: Bogus` — errors persist per instance until a newer status or a real Key/Mouse interaction reaches that instance (#186), and nothing in Tests 8–9 was one. Banner gone here = FAIL (#186 regression).

## Test 10: `n` enters SelectBranch mode; clicks are dead there
- Action: focus the sidebar by clicking a no-op line (footer or header — an item-row click would activate that row); move ▌ to the `feature-b` row with `j`/`k` (keyboard selection does not activate); press `n`; capture. Then left-click on one of the listed branch rows; capture. Wheel-down; capture.
- Expected: this focusing click (in the repo tab's sidebar, where 9b left you) is the FIRST real Key/Mouse interaction that instance has received since the Test 7 error was set — it clears that instance's red status immediately, before anything else in this step (#186); expect it gone from this capture on. After `n`: a branch-selection list renders with footer `↑/k up  ↓/j down  Enter create  Esc back` (or the narrow variant), with the picker cursor on the FIRST branch row — the picker has its own cursor and always opens at the top, regardless of the browse selection (#184); the click does NOT change the highlighted branch and does NOT activate anything; the wheel does NOT move the selection (mouse is a no-op in this mode). Any mouse effect = FAIL (mode leak).

## Test 11: Esc returns to browse with the cursor where it was
- Action: press Esc; capture.
- Expected: browse list back; ▌ still on `feature-b` — exactly where Test 10 left it before entering the picker; leaving the picker never moves the browse selection (#184); all rows intact.

## Test 12: `i` enters InputBranch; typing renders; clicks are dead
- Action: press `i`; type `my-new-branch`; capture. Left-click a row line; capture. Press Backspace twice; capture.
- Expected: input UI shows `my-new-branch` as typed; footer `Enter create  Esc back`; the click changes nothing; after backspaces the buffer shows `my-new-bran`.

## Test 13: Esc cancels input cleanly
- Action: press Esc; capture.
- Expected: browse list; no new worktree/tab created; no leftover input text anywhere.

## Test 14: InputBranch happy path spawns a new branch
- Action: press `i`; type `typed-branch`; press Enter; wait 8s; capture.
- Expected: `Spawning 'typed-branch'...` then a `typed-branch` tab, active, with a correctly labeled row (`branch: typed-branch`); row count +1; no duplicates.

## Test 15: Confirming mode ignores mouse
- Action: focus the sidebar via a no-op line click (an item-row click would activate and switch tabs, moving keyboard focus away); move ▌ to `fake-agent` with `j`/`k` (keyboard selection does not activate); press `d`; capture. Left-click the `local` row line; capture. Press `n` to cancel.
- Expected: confirm dialog stays up across the click; the click neither cancels, confirms, nor moves anything; after `n` the browse list is unchanged with `fake-agent` still present.

## Test 16: Anomaly sweep
- Action: final plain+ANSI capture of both windows.
- Expected: consistent sidebar; single ▌; one bold-cyan title; glyphs only where set. List anomalies.
