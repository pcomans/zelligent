---
fixture: setup-with-worktrees.sh
launch: ZELLIGENT_PLUGIN_SRC="$HOME/.local/share/zelligent/zelligent-plugin.wasm" ./zelligent.sh
session_name: zelligent-test-repo
---

# UI Audit 02 — Multi-Tab Switching, Renames, Duplicate Names

Many tabs open at once; every sidebar activation must land on EXACTLY the named
tab. Hunts: wrong-tab activation, highlight desync after native switches,
stale/duplicate rows after rename and name collisions.

Harness window: `tmux -L zt-driver-test new-session -d -s zt-driver -n view -x 220 -y 60 -c /tmp/zelligent-test-repo`

## Evidence & scrutiny rules (MANDATORY)

Same as ui-audit-01 (read that section there if needed), with `ARCHIVE=/tmp/zelligent-ui-run/02-multitab`. Key points: archive plain+ANSI capture per step; quote observed lines; FAIL ⇒ repro-NN.sh; anomalies section; REAL clicks/keys only — `zellij action` CLI is allowed ONLY where a step explicitly says "via ctrl window" (setup), never as the tested interaction; re-locate click rows from a fresh capture every time; sleep 1s after clicks, 8s after spawns/new tabs.

Mouse encoding: left click `\033[<0;COL;ROWM\033[<0;COL;ROWm`, wheel up/down `\033[<64/65;COL;ROWM` via `tmux send-keys -l "$(printf ...)"`. COL=10 for sidebar rows. For TAB BAR clicks use the row of the tab bar line (usually line 1) and a COL inside the target tab's name as found in the capture.

Rendering contract: see ui-audit-01 — ▌ cursor axis, BOLD CYAN active-tab axis, subtitles `branch: X` / `current repo` / `user tab`, two-click activation.

## Harness corrections from run 01 (READ FIRST — these override the steps below)

1. MOUSE SETUP: before any click, run `tmux -L zt-driver-test set-option -g mouse on`; send press and release as SEPARATE send-keys calls (`\033[<0;C;RM` then `\033[<0;C;Rm`). Wheel events need no setup.
2. THERE IS NO TAB BAR (layout has only a bottom status-bar). Wherever a step says "tab bar click X" or "click X in the tab bar": instead switch natively with REAL KEYSTROKES — press `C-t` (tab mode) then the digit of the target tab's 1-based position (then Esc if a mode indicator lingers in the status bar). Wherever a step says "active tab in the tab bar": verify via (a) the MAIN pane's frame title (e.g. `┌ feature-a` or the repo tab's pane titles) and (b) the sidebar's bold-cyan row; optional corroboration via ctrl window `zellij --session zelligent-test-repo action query-tab-names` (read-only, never as the tested action). Track tab positions yourself as tabs are created (order of creation = order in query-tab-names).
3. KNOWN BUG (confirmed in run 01, do NOT re-report, do NOT trip over it): clicking a row's SUBTITLE line selects the NEXT item (one-line mapping offset), and clicking the blank separator under the header selects item 0. Therefore: ALWAYS click TITLE lines when selecting rows. If selection lands one row off after a title-line click, THAT is new information — report it.
4. KNOWN RENDER DEVIATION (confirmed, do not re-report): the in-pane header line is missing (content starts with a blank line); the ▌ gutter appears on BOTH lines of the selected item (this is correct behavior).
5. Sidebar plugin state may be PER-TAB (each tab's sidebar pane is its own plugin instance with its own ▌ cursor). When you switch tabs, the cursor may sit elsewhere — this is under investigation, record cursor position after every switch.

## Test 1: Startup sanity
- Action: launch, wait ~8s, capture.
- Expected: rows `local` (▌, bold cyan), `feature-a`, `feature-b`, `feature-c`; exactly 4 items.

## Test 2: Spawn feature-a via two clicks
- Action: click `feature-a` title line; capture; click it again; wait 8s.
- Expected: `Spawning 'feature-a'...` then tab `feature-a` active; sidebar present in the new tab; `feature-a` bold cyan; 4 rows, no duplicates.

## Test 3: Spawn feature-b from within the feature-a tab
- Action: in the current (feature-a) tab's sidebar: click `feature-b`, capture, click again; wait 8s.
- Expected: tab `feature-b` opens and becomes active; sidebar in it shows `feature-b` bold cyan, `feature-a` plain; still 4 rows.

## Test 4: Spawn feature-c likewise
- Action: two clicks on `feature-c`; wait 8s.
- Expected: 4 tabs total in tab bar: `zelligent-test-repo`, `feature-a`, `feature-b`, `feature-c`; active = feature-c; sidebar: exactly 4 rows, each worktree row now an open tab (all subtitles `branch: X`), no stray rows.

## Test 5: Round-robin activation lands on EXACTLY the named tab
- Action: perform this sequence, capturing after each activation: two-click `local` → two-click `feature-b` → two-click `feature-a` → two-click `feature-c`.
- Expected: after each pair, the ACTIVE tab in the tab bar is exactly the row clicked, and the bold-cyan row matches it. Any single mismatch = FAIL with the full capture quoted (this is the primary wrong-tab hunt).

## Test 6: Rapid successive switches stay consistent
- Action: two-click `feature-a` then IMMEDIATELY two-click `feature-c` (minimal sleep between the two pairs, ~300ms), then wait 2s and capture.
- Expected: final active tab = `feature-c`; bold cyan on `feature-c` only; ▌ on `feature-c`; no intermediate corruption (single bold-cyan row).

## Test 7: Native switch via TAB BAR click is reflected in the sidebar
- Action: click directly on the `feature-b` tab name in the top tab bar.
- Expected: Zellij activates feature-b; the sidebar's bold-cyan row updates to `feature-b` WITHOUT any sidebar click; ▌ stays wherever it was (independent axis). Desync here = FAIL.

## Test 8: Native switch again to repo tab
- Action: click `zelligent-test-repo` in the tab bar.
- Expected: `local` row becomes bold cyan; all other titles plain.

## Test 9: Manual tab appears as a user-tab row
- Action: via ctrl window: `zellij --session zelligent-test-repo action new-tab --name scratch`; wait 3s; capture view.
- Expected: a 5th row `scratch` / subtitle `user tab` appended after the worktree rows; no status glyph on it; the new tab is active (bold cyan on `scratch` if the sidebar is present in it — if the manual tab has NO sidebar pane, record that as a finding with capture).

## Test 10: Two-click on the user-tab row switches to it
- Action: first click a DIFFERENT tab in the tab bar (e.g. `feature-a`) to move away; then in the sidebar two-click the `scratch` row.
- Expected: active tab = `scratch`; bold cyan on `scratch` row.

## Test 11: Renaming a user tab updates its row with no stale leftover
- Action: via ctrl window: `zellij --session zelligent-test-repo action rename-tab renamed-scratch` (renames the active tab = scratch); wait 3s; capture.
- Expected: row now reads `renamed-scratch` / `user tab`; NO row named `scratch` remains (stale-row hunt); total rows unchanged (5).

## Test 12: Renaming a WORKTREE tab decouples it — document exact behavior
- Action: tab-bar click `feature-a`; via ctrl window rename it: `zellij --session zelligent-test-repo action rename-tab feature-a-renamed`; wait 3s; capture.
- Expected (from code): worktree `feature-a` no longer has a matching tab → its row reverts to detached (`branch: feature-a`, plain title), AND a NEW user-tab row `feature-a-renamed` appears (bold cyan, it's active). Record the exact row list. Stale or duplicated `feature-a` rows in any other form = FAIL.

## Test 13: Activating the now-detached feature-a row — exploratory
- Action: two-click the `feature-a` worktree row.
- Expected (from code): a spawn is attempted for `feature-a` (worktree already exists). Record EXACTLY what happens: new tab named `feature-a`? error status? duplicate rows? A crash, duplicate row, or error message = document with captures either way. This is an exploratory step: report observed behavior in detail; FAIL only on visible corruption (dup rows, garbled UI, wrong tab).

## Test 14: Duplicate tab names — name-based ops ambiguity
- Action: via ctrl window: `zellij --session zelligent-test-repo action new-tab --name feature-b` (collides with the existing worktree tab name); wait 3s; capture.
- Expected: record how the sidebar renders — from code, tab matching is name-based, so watch for: duplicate `feature-b` rows, the manual tab absorbed into the worktree row, or a `user tab` row. Then two-click the `feature-b` row and record WHICH tab activates (there are two with that name — tab bar position tells them apart). Ambiguous activation is expected-ish; sidebar corruption (dup/stale rows) = FAIL.

## Test 15: Closing tabs cleans rows with no stales
- Action: with the duplicate `feature-b` tab active, via ctrl window: `zellij --session zelligent-test-repo action close-tab`; wait 3s; capture.
- Expected: the duplicate tab is gone from the tab bar; sidebar shows NO leftover extra `feature-b` row; the worktree `feature-b` row remains (its real tab still open); row count back to pre-Test-14.

## Test 16: 5+ tabs ordering stability sweep
- Action: capture the sidebar; compare row ORDER against: `local` first, then worktrees in list order (feature-a, feature-b, feature-c), then user tabs in tab order.
- Expected: exact ordering; any reordering across the session (compare with earlier captures) = FAIL.

## Test 17: Full anomaly sweep
- Action: final capture, plain + ANSI.
- Expected: single ▌; exactly one bold-cyan title; no SCROLL counter; no garbled lines. List anomalies.
