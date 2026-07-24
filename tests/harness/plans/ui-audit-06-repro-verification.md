---
fixture: setup-with-worktrees.sh
launch: zelligent  # INSTALLED CLI — never the fixture clone's ./zelligent.sh (old main; see README "CLI under test")
session_name: zelligent-test-repo
---

# UI Audit 06 — Reproducibility Verification of All Suspected Bugs

Each numbered repro below re-derives one finding from the clean fixture, in one
session, in order. For each: REPRODUCED / NOT-REPRODUCED / PARTIAL with capture
evidence. These are confirmation experiments — follow the exact sequences; where
a repro includes a DISCRIMINATOR sub-step, it distinguishes competing theories,
so capture it carefully.

Harness window: `tmux -L zt-driver-test new-session -d -s zt-driver -n view -x 220 -y 60 -c /tmp/zelligent-test-repo` (TALL — required for R1/R2/R3).

Conventions: as ui-audit-01 (tmux mouse on; press/release separate send-keys; ARCHIVE=/tmp/zelligent-ui-run/06-repro; step captures rNN-*.txt/.ansi; quote evidence; real input only; ctrl window only where stated; ~8s after spawns, ~1s after input).

## R1 — BUG-1: subtitle click selects the NEXT item (tall window)
- Note: since #135/#137, a subtitle click both selects AND activates its item (single-click contract), so each click below now also spawns/switches a tab — wait 8s after each before capturing.
- From clean startup (▌ on local, active repo tab): capture; left-click the `branch: feature-a` SUBTITLE line (locate in fresh capture); wait 8s.
- REPRODUCED (of the original bug) if ▌ (and the newly-active tab) land on `feature-b` instead of `feature-a`. Current code is expected NOT-REPRODUCED: ▌ and active tab both land on `feature-a`, which spawns (detached).
- Then, in feature-a's own sidebar instance: click the `branch: feature-c` SUBTITLE line (last item's subtitle); wait 8s.
- REPRODUCED if nothing changes (maps past end → no-op) — record either way. Expected NOT-REPRODUCED: ▌ and active tab move to `feature-c`, which spawns.
- Then, in feature-c's sidebar instance: click the blank line directly above `local` (a no-op zone under the current contract).
- REPRODUCED if ▌ jumps to `local`. Expected NOT-REPRODUCED: no change, still on `feature-c` (blank separator clicks are a no-op regardless of the single-click contract).

## R2 — BUG-2: in-pane header missing, one leading blank line (tall window)
- From the startup capture of R1: REPRODUCED if terminal row 2 (first in-pane content row) is blank and NO line anywhere in the sidebar contains the repo-name header (` zelligent-test-repo ` since #156; ` zelligent / zelligent-test-repo ` on older builds).

## R3 — BUG-8: leading-line count shifts with status text (dynamic mapping offset)
- Click `local` title (selects AND activates — switches to the repo tab if not already there; wait for the switch, then re-click a no-op sidebar line to reclaim keyboard focus before pressing `d`, since a real tab switch moves keyboard focus to the new tab's main pane). Press `d` (sidebar focused) → red error `Only worktree tabs can be removed` wraps to 2 lines at 36 cols.
- Capture: REPRODUCED if the leading blank row is now GONE (items start at the first in-pane row, shifted up 1 vs R2).
- DISCRIMINATOR: now click the `branch: feature-a` SUBTITLE line again; wait 8s (this both selects and activates under the current contract — spawns or switches `feature-a`). If ▌ (and the active tab) land on `feature-a` (correct!) the mapping offset CHANGED with the status height — proving the offset is dynamic. Record exact before/after row positions.

## R4 — BUG-3 / #189: the focus-claim click still costs exactly one extra click (down from three under the retired two-click contract)
- PRECONDITION (live-run finding): the focus-claim only fires on a sidebar pane instance that has NEVER been click-focused, or after a cross-tab landing took focus elsewhere. By this point R1/R3 have already click-focused the repo, feature-a, and feature-c sidebar panes, so a probe against any of THOSE is NOT eaten — a single click acts immediately (that is not a finding). The probe must target a genuinely fresh pane: the NEW tab the spawn below lands in.
- Press `r` to clear status. In the current sidebar, click the `feature-b` TITLE line once — the only still-detached branch; single click selects AND activates, and this pane has been click-focused before, so expect the click to act immediately (`Spawning 'feature-b'...`; if it is eaten instead, record that — it contradicts the precondition). Wait 8s: you land in the feature-b tab; its sidebar pane instance has NEVER been clicked, and keyboard focus is on the MAIN pane (shell).
- In THIS fresh sidebar, click the `local` TITLE line ONCE. Capture. EXPECTED (focus-claim click, #189, still real): NOTHING visibly changes — Zellij's click-to-focus eats this click before the plugin receives it.
- Click the same line again. Capture. EXPECTED under the current single-click contract: this one click both selects AND activates — ▌ moves to `local` and the active tab switches to `zelligent-test-repo`. Total = 2 real clicks on the fresh pane (1 eaten by focus-claim + 1 that selects-and-activates) — REPRODUCED (of the historical 3-click bug) only if activation still needs a 3rd click. If the FIRST click on the fresh pane acts instead of being eaten, the pane was not actually fresh — restate which pane you probed and re-run against a fresh one. Record the exact count needed.

## R5 — BUG-5: manual `new-tab` produces a sidebar-only full-width tab
- Via ctrl window: `zellij --session zelligent-test-repo action new-tab --name manualtab`; wait 3s; capture.
- REPRODUCED if the new tab shows the zelligent sidebar plugin pane at (near-)full width with NO shell pane. Record the pane structure you see.

## R6 — BUG-7: stale row after plugin-driven remove
- Switch to the feature-b tab (spawned in R4; native C-t+digit). In its sidebar: click `feature-b` title once if not selected, press `d`, capture the confirm dialog, press `y`; wait 8s; capture the tab you land on.
- REPRODUCED if the sidebar you land on still shows a `feature-b` row (worktree is deleted on disk — corroborate via ctrl window `ls ~/.zelligent/worktrees/zelligent-test-repo/`).
- Then press `r`; capture. Expect the row disappears → confirms staleness (not a live worktree).

## R7 — FINDING-9: do new tabs miss earlier agent-status events? (expected: no — glyph state is shared)
- PREMISE CORRECTION (two consecutive live runs): glyph state is globally shared/broadcast across sidebar instances — a NEW tab's sidebar immediately shows glyphs from earlier events. NOT-REPRODUCED is the norm here; REPRODUCED would be a regression.
- Via ctrl window: pipe `event=Start,tab=feature-a` (`zellij --session zelligent-test-repo pipe --name zelligent-status --args "event=Start,tab=feature-a"`); wait 2s; capture current tab's sidebar → expect green ● on feature-a row.
- Via ctrl window: `zellij --session zelligent-test-repo action new-tab --name glyphprobe`; wait 3s; capture the new tab's sidebar.
- REPRODUCED if the new tab's sidebar shows NO ● on feature-a.
- DISCRIMINATOR: natively switch back (C-t+digit) to the tab where the glyph was visible; capture. If ● is STILL there → per-instance state (glyphs differ across tabs simultaneously). If it is gone everywhere → global clear on TabUpdate. Record which.

## R8 — BUG-10: auto-Start pipe from spawn is dropped (no working glyph on fresh spawn)
- PREMISE CORRECTION (2026-07-05 verification run): there is NO automatic `event=Start` pipe during spawn — neither `zelligent.sh` nor the plugin pipes anything; the only auto-pipe source is the Claude Code plugin's `UserPromptSubmit`/session hooks, which fire only when a real `claude` agent runs (this fixture's default agent-cmd is `$SHELL`, so nothing pipes). The original audit's "auto-Start dropped" observation was partly a fixture artifact.
- The race this step now tests deterministically: via ctrl window, pipe `event=Start,tab=feature-b` while NO feature-b tab exists (the target must be a branch with no live tab at this point in a sequential run — R1 already spawned feature-a; after R6's removal feature-b qualifies). Capture (no glyph, no error — silently held). Then respawn `feature-b` via the `n` branch picker (its worktree was deleted in R6 so it has no sidebar row to click: press `n` in the focused sidebar, type `feature-b` to filter the list down to it — j/k no longer navigate the picker post-#196, they're filter characters like any other letter — then press Enter; this also exercises the #184 own-cursor behavior: the picker opens on row 0 and leaves the browse ▌ untouched); after the tab opens, natively switch back to the repo tab and capture ANSI.
- With the #141 buffer fix: the feature-b row in the repo tab's sidebar shows the green ● (`\x1b[32m●`) — the pre-existing instance buffered the early event and drained it on the registering TabUpdate. Pre-fix behavior (REPRODUCED): the early event is dropped and no ● ever appears without a second manual pipe.
- Negative control: pipe `event=Start,tab=nonexistent` → no glyph anywhere, no error text.

## R9 — BUG-6: duplicate-named manual tab invisible in sidebar
- Via ctrl window: `zellij --session zelligent-test-repo action new-tab --name feature-a` (feature-a tab already open from R4); wait 3s; capture.
- REPRODUCED if the sidebar shows only ONE feature-a row and NO new `user tab` row (tab count via ctrl `query-tab-names` shows two `feature-a` entries).

## Final: summary table
- For each of R1-R9: REPRODUCED / NOT-REPRODUCED / PARTIAL, one-line evidence citation (capture file + row), and the exact minimal command sequence used.
