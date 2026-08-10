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
3. SINGLE-CLICK SELECT+ACTIVATE (#135/#137, reaffirmed PR #211): a real click select+activates its row — spawn if detached, switch if open — after the focus-claim click (the first click following any cross-tab landing is swallowed). SUBTITLE-OFFSET BUG FIXED (reverified 2026-08-05): clicks map to their OWN item at ZERO offset; clicking a subtitle line hits that same row, and the blank line above `local` is a no-op. Clicking TITLE lines is still the cleaner habit but is no longer required to dodge an offset. Where a step says "two-click X", read it as focus-claim click + one real activating click.
4. IN-PANE HEADER RENDERS: post-#218 the sidebar is borderless and the repo-name header renders as the SOLE title line (no longer "missing"). KNOWN (BUG-2): a cold-start blank header can persist a whole session on large fixtures — note once, don't re-diagnose. The ▌ gutter renders on BOTH lines of the selected item — correct, not a finding.
5. Keyboard keys (`d`, `y`, `n`, `r`) go to the SIDEBAR pane — ensure it has focus first (a title-line click on the already-selected row would ACTIVATE; instead focus by clicking a title line of a row you intend to select anyway, per the step).

## Test 1: Startup sanity
- Action: launch, wait ~8s, capture.
- Expected: `local` (▌, bold cyan) + `feature-a/b/c` detached rows; 4 items.

## Test 2: Out-of-band worktree tab self-heals within one refresh (#127)
- Action: create the worktree AND a matching-named tab out-of-band with NON-ATTACHING primitives — NEVER `zelligent spawn` from ctrl (outside Zellij it execs `zellij attach` and mirrors ctrl into the live session; see README "What must NEVER run"). Via ctrl window: `git -C /tmp/zelligent-test-repo worktree add "$HOME/.zelligent/worktrees/zelligent-test-repo/oob-test" -b oob-test`, then `zellij --session zelligent-test-repo action new-tab --name oob-test --cwd "$HOME/.zelligent/worktrees/zelligent-test-repo/oob-test"`. `action new-tab` makes the new (sidebar-less) tab active, so switch back to a sidebar-bearing tab to observe: `zellij --session zelligent-test-repo action go-to-tab-name zelligent-test-repo`. Capture view; wait 3s; capture again. (This reproduces the exact self-heal condition — a worktree tab the running sidebar did not create — without the attaching-spawn footgun.)
- Expected: the running sidebar (in the repo tab) gains ONE row `oob-test`. Because a worktree named `oob-test` now exists, the sidebar matches the tab to it and the subtitle settles on `branch: oob-test` — NOT `user tab`. If the FIRST capture shows `user tab` but the later capture shows `branch: oob-test`, that is the designed one-shot self-heal (the worktree-list cache refreshes on the next pipe/refresh): record timings, PASS with note. A `user tab` label that persists = FAIL (#127 regression). Duplicate oob-test rows = FAIL.

## Test 3: Row ordering after out-of-band spawn
- Action: inspect the last capture.
- Expected: order is `local`, worktrees in list order (now including `oob-test` wherever `list-worktrees` places it — record where), no user-tab rows. Exactly 5 items.

## Test 4: External removal from a plain shell leaves the tab for manual close (auto-close is $ZELLIJ-gated)
- Action: ensure the `zelligent-test-repo` tab is focused (native `C-t` + digit `1`). Then via ctrl window — a plain shell OUTSIDE Zellij — run `cd /tmp/zelligent-test-repo && ./zelligent.sh remove oob-test 2>&1 | tee /tmp/zelligent-ui-run/04-lifecycle/remove-oob.log`; wait 5s; capture. (Tee the stdout: the alt-screen wipes tmux scrollback and the CLI's message is the evidence. `zelligent remove` is non-attaching and safe from ctrl.)
- Expected: the removal SUCCEEDS — the worktree is deleted on disk — but the tab is NOT auto-closed. zelligent's tab auto-close block is deliberately gated on `[ -n "$ZELLIJ" ]` (see the code comment near zelligent.sh:1498), and ctrl is outside Zellij, so instead the CLI prints `ℹ️  Close the 'oob-test' tab manually if still open.` (confirm in the tee'd log). Verify: (a) the `oob-test` tab STILL EXISTS in `zellij --session zelligent-test-repo action query-tab-names` — it lingers; (b) the sidebar DEGRADES the `oob-test` row to `user tab` (worktree gone → no longer matches a worktree → subtitle `user tab`, title plain), NOT a stale `branch: oob-test` row and NOT a duplicate; (c) the focused tab is still `zelligent-test-repo`. The #122 wrong-tab-closed race is UNREACHABLE on this out-of-band path because the close code never runs. Finish cleanup from ctrl with the non-attaching pair: `zellij --session zelligent-test-repo action go-to-tab-name oob-test` then `zellij --session zelligent-test-repo action close-tab`; row count then returns to 4.

## Test 5: Spawn feature-a via real sidebar clicks
- Action: two-click `feature-a` in the sidebar; wait 8s.
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
- Action: two-click `feature-b` (spawn, wait 8s) → press `d`, `y` (remove, wait 8s) → if the row is gone, recreate via ctrl window `git -C /tmp/zelligent-test-repo worktree add "$HOME/.zelligent/worktrees/zelligent-test-repo/feature-b" feature-b 2>/dev/null || git -C /tmp/zelligent-test-repo worktree add "$HOME/.zelligent/worktrees/zelligent-test-repo/feature-b" -b feature-b`; press `r`; then two-click `feature-b` again (spawn, wait 8s). Capture at every stage.
- Expected: at NO stage do duplicate `feature-b` rows exist; no stale row for the removed tab between remove and respawn; final state has exactly one `feature-b` row, open+active; tab bar consistent throughout.

## Test 13: Stale-row sweep across all captures
- Action: `grep -c` sanity over archived captures for each known row name; final capture.
- Expected: final sidebar rows exactly: `local`, remaining worktrees, `scratch` user tab. Report any capture where a row appeared twice or a removed row lingered ≥ 2 captures after its removal.

## Test 14: Anomaly sweep
- Action: final plain+ANSI capture.
- Expected: single ▌, one bold-cyan title, no artifacts. List anomalies.
