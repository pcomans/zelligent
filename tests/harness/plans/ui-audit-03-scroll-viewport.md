---
fixture: setup-many-worktrees.sh
launch: ZELLIGENT_PLUGIN_SRC="$HOME/.local/share/zelligent/zelligent-plugin.wasm" ./zelligent.sh
session_name: zelligent-test-repo
---

# UI Audit 03 — Viewport Scrolling & Click Mapping Under Scroll

10 worktrees in a SHORT window so the sidebar viewport must scroll. Prime
hunt: clicks landing on the wrong row once `viewport.start > 0` (the plugin
maps mouse line → `viewport.start + line/2`), wrong branch spawned from a
scrolled list, selected-row-always-visible violations, truncation and
dir≠branch rendering.

Harness window (SHORT — this is the point):
`tmux -L zt-driver-test new-session -d -s zt-driver -n view -x 220 -y 24 -c /tmp/zelligent-test-repo`

## Evidence & scrutiny rules (MANDATORY)

Same as ui-audit-01, with `ARCHIVE=/tmp/zelligent-ui-run/03-scroll`. Plain+ANSI capture per step; quote observed lines; FAIL ⇒ repro-NN.sh; anomalies section; REAL input only; re-locate rows from a fresh capture before every click; sleep 1s after input, 8s after spawns.

Mouse encoding: left click `\033[<0;COL;ROWM\033[<0;COL;ROWm`, wheel `\033[<64/65;COL;ROWM`, COL=10, via `tmux send-keys -l "$(printf ...)"`.

Expected item list (11 items, in order): `local`, `wt-01`…`wt-08`, `agent-mouse-test`, `feature-very-long-branch-name-for-truncation-check`.

Viewport rule (from code): `max_items = max(1, (rows-5)/2)` for the sidebar pane's height; the selected item is ALWAYS within the visible window; `start = selected - max_items + 1` once selected ≥ max_items.

## Harness corrections from run 01 (READ FIRST — these override the steps below)

1. MOUSE SETUP: before any click, run `tmux -L zt-driver-test set-option -g mouse on`; send press and release as SEPARATE send-keys calls. Wheel events need no setup.
2. THERE IS NO TAB BAR. Verify "a tab named X is active" via the MAIN pane's frame title and the sidebar's bold-cyan row; corroborate via ctrl window `zellij --session zelligent-test-repo action query-tab-names` (read-only only).
3. KNOWN BUG (confirmed in run 01, in the UNSCROLLED state): clicking a SUBTITLE line selects the NEXT item; clicking the blank line under the header selects item 0; the in-pane header line is missing entirely (content starts with a blank line). For THIS plan: Tests 7/8/9 measure the mapping in the SCROLLED state — run them exactly as written and report the exact delta between clicked row and selected row (we need to know whether the offset is the same one line, or compounds with viewport start).
4. The ▌ gutter renders on BOTH lines of the selected item — correct behavior, not a finding.

## Test 1: Startup — partial list, no corruption
- Action: launch, wait ~8s, capture.
- Expected: header + `local` row (▌, bold cyan) + as many following items as fit, each exactly 2 lines; narrow footer visible at bottom; NO overlap of footer and rows; no SCROLL counter. Record exactly which items are visible (this defines max_items empirically — note the number).

## Test 2: Truncation with `…`
- Action: wheel-down repeatedly until the long-name row is visible/selected, capture ANSI.
- Expected: title of the long worktree is clipped to the content width and ends with `…`; subtitle `branch: feature-very-long-branch-name-for-truncation-check` also fits the pane (clipped with `…` if needed); no wrapping onto extra lines.

## Test 3: dir≠branch row renders dir as title, branch in subtitle
- Action: navigate (wheel) until `agent-mouse-test` visible, capture.
- Expected: title `agent-mouse-test`, subtitle `branch: agent/mouse-test`.

## Test 4: Selected row is always visible while wheeling down the full list
- Action: starting from `local`, wheel-down one step at a time through ALL 11 items, capturing at each step (batch: wheel+sleep+capture).
- Expected: at every step the ▌ row is on-screen; once selection exceeds the visible count the top rows scroll away and the list window slides; items always appear as complete 2-line pairs (never a title without its subtitle at the window edges); header and footer never scroll away.

## Test 5: Wrap-around at bottom jumps viewport to top
- Action: with ▌ on the LAST item, wheel-down once.
- Expected: ▌ on `local`, viewport back at the top of the list.

## Test 6: Wrap-around at top jumps viewport to bottom
- Action: wheel-up once from `local`.
- Expected: ▌ on the long-name item (last), viewport shows the tail of the list.

## Test 7: CLICK MAPPING WITH SCROLLED VIEWPORT — the critical check
- Action: viewport is now scrolled (start > 0 from Test 6). Capture; pick a visible row in the MIDDLE of the window that is NOT selected, e.g. `wt-06` (use whatever is visible); left-click its TITLE line.
- Expected: ▌ moves to EXACTLY the clicked row (`wt-06`), not to `wt-05`/`wt-07` or an item offset by the scroll amount. An offset here explains "click activates wrong tab" — if it happens, note the exact delta and write the repro.

## Test 8: Click mapping on a SUBTITLE line in scrolled state
- Action: click the `branch: wt-05` subtitle line (or whichever is visible).
- Expected: ▌ moves to that same item (title/subtitle pair maps to one index).

## Test 9: Activation from a scrolled viewport spawns the RIGHT branch
- Action: with a row selected in the scrolled window (e.g. `wt-05`), click it a second time; wait 8s.
- Expected: status shows `Spawning 'wt-05'...` naming EXACTLY the selected branch; the new tab in the tab bar is named `wt-05`; the `wt-05` row becomes bold cyan. A different branch spawning = critical FAIL.

## Test 10: Post-spawn sidebar in short window remains sane
- Action: capture after Test 9 settles.
- Expected: still 11 items total (row count unchanged); no duplicates; selected/active row visible; footer intact.

## Test 11: Click on the footer in a short window is a no-op
- Action: click the version line at the bottom.
- Expected: no selection change, no activation. (In a short window the last item rows sit close to the footer — an off-by-N mapping would make footer clicks select rows.)

## Test 12: Keyboard parity in scrolled state
- Action: press `k` five times (sidebar focused from clicks), capturing after each.
- Expected: identical movement/scrolling behavior to wheel-up, selected row always visible.

## Test 13: Anomaly sweep
- Action: final plain+ANSI capture.
- Expected: single ▌, single bold-cyan title, complete 2-line items, no artifacts. List anomalies.
