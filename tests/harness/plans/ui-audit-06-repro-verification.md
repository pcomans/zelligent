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
- From clean startup (▌ on local, active repo tab): capture; left-click the `branch: feature-a` SUBTITLE line (locate in fresh capture).
- REPRODUCED if ▌ lands on `feature-b` (not feature-a).
- Then: click the `branch: feature-c` SUBTITLE line (last item's subtitle).
- REPRODUCED if nothing changes (maps past end → no-op) — record either way.
- Then: click the blank line directly above `local`.
- REPRODUCED if ▌ jumps to `local`.

## R2 — BUG-2: in-pane header missing, one leading blank line (tall window)
- From the startup capture of R1: REPRODUCED if terminal row 2 (first in-pane content row) is blank and NO line anywhere in the sidebar contains the repo-name header (` zelligent-test-repo ` since #156; ` zelligent / zelligent-test-repo ` on older builds).

## R3 — BUG-8: leading-line count shifts with status text (dynamic mapping offset)
- Click `local` title once if not selected. Press `d` (sidebar focused) → red error `Only worktree tabs can be removed` wraps to 2 lines at 36 cols.
- Capture: REPRODUCED if the leading blank row is now GONE (items start at the first in-pane row, shifted up 1 vs R2).
- DISCRIMINATOR: now click the `branch: feature-a` SUBTITLE line again. If ▌ lands on `feature-a` (correct!) the mapping offset CHANGED with the status height — proving the offset is dynamic. Record exact before/after row positions.

## R4 — BUG-3: three clicks to activate from an unfocused sidebar
- Press `r` to clear status. Two-click spawn `feature-a` (or three — count!). Wait 8s: now in feature-a tab, focus on the MAIN pane (shell).
- Click the `feature-b` TITLE line in this tab's sidebar ONCE. Capture. REPRODUCED if NOTHING visibly changes (click eaten by focus claim).
- Click the same line again. Capture. Expect ▌ moves to feature-b now.
- Click again. Capture. Expect activation (spawn of feature-b). Total = 3 clicks. Record the exact count needed.

## R5 — BUG-5: manual `new-tab` produces a sidebar-only full-width tab
- Via ctrl window: `zellij --session zelligent-test-repo action new-tab --name manualtab`; wait 3s; capture.
- REPRODUCED if the new tab shows the zelligent sidebar plugin pane at (near-)full width with NO shell pane. Record the pane structure you see.

## R6 — BUG-7: stale row after plugin-driven remove
- Switch to the feature-b tab (spawned in R4; native C-t+digit). In its sidebar: click `feature-b` title once if not selected, press `d`, capture the confirm dialog, press `y`; wait 8s; capture the tab you land on.
- REPRODUCED if the sidebar you land on still shows a `feature-b` row (worktree is deleted on disk — corroborate via ctrl window `ls ~/.zelligent/worktrees/zelligent-test-repo/`).
- Then press `r`; capture. Expect the row disappears → confirms staleness (not a live worktree).

## R7 — FINDING-9: agent-status glyphs are per-instance (new tabs miss earlier events)
- Via ctrl window: pipe `event=Start,tab=feature-a` (`zellij --session zelligent-test-repo pipe --name zelligent-status --args "event=Start,tab=feature-a"`); wait 2s; capture current tab's sidebar → expect green ● on feature-a row.
- Via ctrl window: `zellij --session zelligent-test-repo action new-tab --name glyphprobe`; wait 3s; capture the new tab's sidebar.
- REPRODUCED if the new tab's sidebar shows NO ● on feature-a.
- DISCRIMINATOR: natively switch back (C-t+digit) to the tab where the glyph was visible; capture. If ● is STILL there → per-instance state (glyphs differ across tabs simultaneously). If it is gone everywhere → global clear on TabUpdate. Record which.

## R8 — BUG-10: auto-Start pipe from spawn is dropped (no working glyph on fresh spawn)
- PREMISE CORRECTION (2026-07-05 verification run): there is NO automatic `event=Start` pipe during spawn — neither `zelligent.sh` nor the plugin pipes anything; the only auto-pipe source is the Claude Code plugin's `UserPromptSubmit`/session hooks, which fire only when a real `claude` agent runs (this fixture's default agent-cmd is `$SHELL`, so nothing pipes). The original audit's "auto-Start dropped" observation was partly a fixture artifact.
- The race this step now tests deterministically: via ctrl window, pipe `event=Start,tab=feature-a` while NO feature-a tab exists yet; capture (no glyph, no error — silently held). Then spawn `feature-a` via real title-line clicks; after the tab opens, natively switch back to the repo tab and capture ANSI.
- With the #141 buffer fix: the feature-a row in the repo tab's sidebar shows the green ● (`\x1b[32m●`) — the pre-existing instance buffered the early event and drained it on the registering TabUpdate. Pre-fix behavior (REPRODUCED): the early event is dropped and no ● ever appears without a second manual pipe.
- Negative control: pipe `event=Start,tab=nonexistent` → no glyph anywhere, no error text.

## R9 — BUG-6: duplicate-named manual tab invisible in sidebar
- Via ctrl window: `zellij --session zelligent-test-repo action new-tab --name feature-a` (feature-a tab already open from R4); wait 3s; capture.
- REPRODUCED if the sidebar shows only ONE feature-a row and NO new `user tab` row (tab count via ctrl `query-tab-names` shows two `feature-a` entries).

## Final: summary table
- For each of R1-R9: REPRODUCED / NOT-REPRODUCED / PARTIAL, one-line evidence citation (capture file + row), and the exact minimal command sequence used.
