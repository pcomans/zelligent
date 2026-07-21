---
fixture: setup-with-worktrees.sh
launch: zelligent  # INSTALLED CLI — never the fixture clone's ./zelligent.sh (old main; see README "CLI under test")
session_name: zelligent-test-repo
---

# UI Audit 04 — Stale/Duplicate Rows & Worktree Lifecycle

Out-of-band spawns, removals, and rapid lifecycle churn. Hunts: stale rows,
duplicate rows, the #127 "user tab" mislabel self-heal, the #122 wrong-tab-
closed race, remove-flow rendering.

Harness window: `tmux -L zt-driver-test new-session -d -s zt-driver -n view -x 220 -y 60 -c /tmp/zelligent-test-repo`

## Evidence & scrutiny rules (MANDATORY)

Same as ui-audit-01, with `ARCHIVE=/tmp/zelligent-ui-run/04-lifecycle`. Plain+ANSI capture per step; quote observed lines; FAIL ⇒ repro-NN.sh; anomalies section. REAL input for all sidebar interaction; ctrl-window shell commands are allowed ONLY where a step explicitly says so (they simulate out-of-band/external events — that is the thing being tested). Sleep 1s after input, 8s after spawns/removes. Re-locate rows from fresh captures.

Mouse encoding: left click `\033[<0;COL;ROWM\033[<0;COL;ROWm`, COL=10, via `tmux send-keys -l "$(printf ...)"`.

## Harness corrections from run 01 (READ FIRST — these override the steps below)

1. MOUSE SETUP: before any click, run `tmux -L zt-driver-test set-option -g mouse on`; send press and release as SEPARATE send-keys calls. Wheel events need no setup.
2. THERE IS NO TAB BAR. Wherever a step says "tab bar click X": switch natively with real keystrokes — `C-t` then the digit of the target tab's 1-based position. Verify the active tab via the MAIN pane's frame title and the sidebar's bold-cyan row; corroborate via ctrl window `zellij --session zelligent-test-repo action query-tab-names` (read-only only). "Tab disappears from the tab bar" ⇒ verify via query-tab-names + main pane.
3. KNOWN BUG (confirmed, do NOT trip over it): clicking a SUBTITLE line selects the NEXT item. ALWAYS click TITLE lines to select. The blank line under the (missing) header selects item 0 — avoid it.
4. The ▌ gutter renders on BOTH lines of the selected item — correct, not a finding. The in-pane header line is missing — known, don't re-report.
5. Keyboard keys (`d`, `y`, `n`, `r`) go to the SIDEBAR pane — ensure it has focus first. Under the single-click select+activate contract (#137), a click on ANY item row — selected or not — now activates it; to focus the sidebar pane without disturbing selection or triggering activation, click a no-op line instead (header, blank separator, or footer).

## Test 1: Startup sanity
- Action: launch, wait ~8s, capture.
- Expected: `local` (▌, bold cyan) + `feature-a/b/c` detached rows; 4 items.

## Test 2: Out-of-band spawn self-heals within one refresh (#127)
- Action: via ctrl window: `cd /tmp/zelligent-test-repo && ZELLIGENT_PLUGIN_SRC="$HOME/.local/share/zelligent/zelligent-plugin.wasm" ./zelligent.sh spawn oob-test`; wait 3s; capture view; wait 3s more; capture again.
- Expected: a new tab `oob-test` appears and the sidebar gains ONE row `oob-test` with subtitle `branch: oob-test` — NOT `user tab`. If the FIRST capture shows `user tab` but the second shows `branch: oob-test`, that is the designed one-shot self-heal: record timings, PASS with note. A `user tab` label that persists in the second capture = FAIL (#127 regression). Duplicate oob-test rows = FAIL.

## Test 3: Row ordering after out-of-band spawn
- Action: inspect the last capture.
- Expected: order is `local`, worktrees in list order (now including `oob-test` wherever `list-worktrees` places it — record where), no user-tab rows. Exactly 5 items.

## Test 4: External removal does NOT close the wrong tab (#122)
- Action: first note the ACTIVE tab in the tab bar (should be `oob-test`'s tab or whichever is active — record it). Via tab bar click, switch to the `zelligent-test-repo` tab so a NON-target tab is focused. Then via ctrl window: `cd /tmp/zelligent-test-repo && ./zelligent.sh remove oob-test`; wait 5s; capture.
- Expected: the `oob-test` tab disappears from the tab bar; the FOCUSED tab is still `zelligent-test-repo` (origin tab NOT closed — the #122 race); sidebar drops the `oob-test` row completely (no stale row); back to 4 items.

## Test 5: Spawn feature-a via a real sidebar click
- Action: click `feature-a` in the sidebar; wait 8s.
- Expected: tab `feature-a` active; row bold cyan; 4 items.

## Test 6: `d` on a worktree row shows the confirm dialog
- Action: sidebar focused; ensure ▌ on `feature-a` (click it once if needed); press `d`; capture.
- Expected: the sidebar switches to a confirmation UI mentioning removal of `feature-a` (record the EXACT text rendered); footer/keybind area changes accordingly.

## Test 7: `n` cancels the confirm dialog
- Action: press `n`; capture.
- Expected: back to the browse list, unchanged rows, no removal happened, `feature-a` tab still open.

## Test 8: `d` then `y` removes: status, tab close, row update, focus safety
- Action: press `d` again; capture; press `y`; wait 8s; capture.
- Expected: status shows `Removing 'feature-a'...` (green) during the operation; the `feature-a` tab closes; NO other tab closes (tab bar: `zelligent-test-repo` remains, count drops by exactly 1); the `feature-a` row either disappears (worktree deleted) — record which; no stale bold-cyan row pointing at the closed tab; focus lands on a surviving tab with a working sidebar.

## Test 9: `d` on the local row errors correctly
- Action: click `local` once (select); press `d`; capture ANSI.
- Expected: red status message `Only worktree tabs can be removed`; NO confirm dialog; rows unchanged.

## Test 10: `d` on a user-tab row errors correctly
- Action: via ctrl window: `zellij --session zelligent-test-repo action new-tab --name scratch`; wait 3s; in the sidebar click the `scratch` row once; press `d`; capture ANSI.
- Expected: same red error; `scratch` row intact.

## Test 11: `r` refresh is visible and harmless
- Action: press `r`; capture.
- Expected: green status `Refreshed`; row list identical before/after.

## Test 12: Rapid churn — spawn, remove, respawn the same branch
- Action: click `feature-b` (spawn, wait 8s) → press `d`, `y` (remove, wait 8s) → if the row is gone, recreate via ctrl window `git -C /tmp/zelligent-test-repo worktree add "$HOME/.zelligent/worktrees/zelligent-test-repo/feature-b" feature-b 2>/dev/null || git -C /tmp/zelligent-test-repo worktree add "$HOME/.zelligent/worktrees/zelligent-test-repo/feature-b" -b feature-b`; press `r`; then click `feature-b` again (spawn, wait 8s). Capture at every stage.
- Expected: at NO stage do duplicate `feature-b` rows exist; no stale row for the removed tab between remove and respawn; final state has exactly one `feature-b` row, open+active; tab bar consistent throughout.

## Test 13: Stale-row sweep across all captures
- Action: `grep -c` sanity over archived captures for each known row name; final capture.
- Expected: final sidebar rows exactly: `local`, remaining worktrees, `scratch` user tab. Report any capture where a row appeared twice or a removed row lingered ≥ 2 captures after its removal.

## Test 14: Anomaly sweep
- Action: final plain+ANSI capture.
- Expected: single ▌, one bold-cyan title, no artifacts. List anomalies.
