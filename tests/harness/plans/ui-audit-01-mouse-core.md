---
fixture: setup-with-worktrees.sh
launch: zelligent  # INSTALLED CLI — never the fixture clone's ./zelligent.sh (old main; see README "CLI under test")
session_name: zelligent-test-repo
---

# UI Audit 01 — Core Mouse Interaction Contract

Exhaustive check of click/wheel semantics on the persistent sidebar with the
seeded 3-worktree fixture. Hunts: click-activates-wrong-row (offset bugs),
selection/active-highlight desync, single-click select+activate landing on the
wrong row, and clicks leaking through the focus-claim guard.

Harness window: `tmux -L zt-driver-test new-session -d -s zt-driver -n view -x 220 -y 60 -c /tmp/zelligent-test-repo`

## Evidence & scrutiny rules (MANDATORY)

- `ARCHIVE=/tmp/zelligent-ui-run/01-mouse-core` — `mkdir -p "$ARCHIVE"` before starting.
- After EVERY step save both captures (batch action+sleep+captures in one command):
  - plain: `tmux -L zt-driver-test capture-pane -t zt-driver:view -p > $ARCHIVE/step-NN.txt`
  - ANSI: `tmux -L zt-driver-test capture-pane -t zt-driver:view -p -e -J > $ARCHIVE/step-NN.ansi`
- PASS only if EVERY visual detail in Expected matches. Quote the relevant capture lines in your report for each step, PASS or FAIL.
- On FAIL: additionally write `$ARCHIVE/repro-NN.sh` — minimal commands from fresh fixture to reproduce.
- Record ANY on-screen anomaly (garbled rows, SCROLL counters, duplicated frames, wrong colors, artifacts) in an "Anomalies" section, even on passing steps.
- REAL INPUT ONLY: every action is a real key or SGR mouse escape sent into the `view` window. NEVER substitute `zellij action ...` CLI for a click or keypress. If input can't be delivered, mark the step BLOCKED with the reason.
- Mouse encoding (1-based coords; ROW = line number of the target line in the LATEST capture, COL = 10 unless stated; re-locate the row from a fresh capture before every click):
  - Left click: `tmux -L zt-driver-test send-keys -t zt-driver:view -l "$(printf '\033[<0;COL;ROWM\033[<0;COL;ROWm')"`
  - Wheel up: `... -l "$(printf '\033[<64;COL;ROWM')"` — Wheel down: `... -l "$(printf '\033[<65;COL;ROWM')"`
  - Right click: `... -l "$(printf '\033[<2;COL;ROWM\033[<2;COL;ROWm')"`
- Sleep 1s after clicks/keys, 8s after spawns, then capture.
- ANSI markers: cursor gutter `\x1b[36m▌`; active-tab title has `\x1b[1m` + `\x1b[36m`; green `\x1b[32m`, yellow `\x1b[33m`, red `\x1b[31m`.

## Rendering contract (assert against this)

- Sidebar pane = 36 cols. Header: ` zelligent-test-repo ` + `─` fill, bold cyan (repo name only, #156).
- Each item = 2 lines. Title line: `▌ name` (cyan bar + space) when the cursor is on it, else two spaces + name. Subtitle line (dim): `branch: X` for worktrees, `current repo` for local, `user tab` for manual tabs.
- The Zellij-active tab's row title is BOLD CYAN — independent axis from the ▌ cursor.
- Footer (36 cols = narrow variant): `↑/k up  ↓/j down  Enter open` / `n branch  i new  d del  r ↻` / version line.
- Single-click select+activate contract (#135/#137, reaffirmed PR #211): a real click on a non-active row immediately SELECTS and ACTIVATES it (switch if its tab is open, spawn if detached) — there is NO separate select click. The one caveat is the focus-claim click (see Harness corrections): the first click after every cross-tab landing is swallowed with zero state change, so it takes one focus-claim click plus one real click to activate from a freshly-landed sidebar. Blank/header/footer clicks: no-op. Wheel and `j`/`k`: cursor-only, ▌ moves one row and wraps at both ends, no activation; `Enter` activates the ▌ row. Right-click: no-op.

## Harness corrections (READ FIRST — these reflect the current contract)

1. MOUSE SETUP: before any click, run `tmux -L zt-driver-test set-option -g mouse on`; send press and release as SEPARATE send-keys calls (`\033[<0;C;RM` then `\033[<0;C;Rm`). Wheel events need no setup.
2. SINGLE-CLICK SELECT+ACTIVATE (#135/#137, reaffirmed PR #211): a real click on a non-active row immediately SELECTS and ACTIVATES it — spawn if detached, switch if open. There is NO two-click select-then-activate contract. Wheel and `j`/`k` are cursor-only (move ▌ without activating); `Enter` activates the ▌ row.
3. FOCUS-CLAIM CLICK: a sidebar pane that is not click-focused eats exactly one click with zero state change, and this recurs after EVERY cross-tab landing (a spawn/switch moves keyboard+click focus to the new tab's main pane). So the FIRST click after each landing is swallowed; the next real click is the one that activates. Count clicks from the first one the plugin actually receives.
4. SUBTITLE-OFFSET BUG FIXED: clicks map to their OWN item at ZERO offset, scrolled or not (reverified in the 2026-08-05 run at viewport starts 1, 2, 3). Clicking a row's subtitle line selects/activates THAT row, not the next one. The blank line above `local` and the header/footer lines are true no-ops (not an item-0 offset).
5. IN-PANE HEADER RENDERS: post-#218 the sidebar is borderless and the repo-name header (` zelligent-test-repo `) renders as the SOLE title line — it is no longer "missing." KNOWN (BUG-2): a cold-start blank header can still occur and, on large fixtures, persist for the whole session; note it once, do not re-diagnose.
6. The ▌ gutter renders on BOTH lines of the selected item — correct behavior, not a finding.
7. KNOWN OPEN OBSERVATION: `Action CliPipe did not complete within 1s timeout` fires in bursts (10–34/session) correlated with every spawn/remove/pipe-invalidate. Record the count and continue; see the README "Known open observations."

## Test 1: Startup render is exactly as specified
- Action: launch, wait ~8s, capture.
- Expected: header line as above; rows in order: `local` (▌ cursor, BOLD CYAN title, subtitle `current repo`), `feature-a`, `feature-b`, `feature-c` (all plain titles, subtitles `branch: feature-X`, no status glyphs); narrow footer; exactly 4 items, no duplicates; no SCROLL counter anywhere.

## Test 2: ANSI styling of startup state
- Action: inspect step-01.ansi (no new input).
- Expected: `▌` preceded by `\x1b[36m` on the local row only; `local` title styled bold+cyan; feature titles unstyled; subtitles dim (`\x1b[2m`).

## Test 3: Wheel-down moves cursor, active highlight stays put
- Action: wheel-down at any sidebar coordinate (e.g. COL=10, ROW = line of `feature-a`).
- Expected: ▌ now on `feature-a`; `local` title STILL bold cyan (active tab unchanged) — cursor and highlight on different rows simultaneously.

## Test 4: Wheel-down to last item
- Action: wheel-down twice more.
- Expected: ▌ on `feature-c`; no viewport shift; local still bold cyan.

## Test 5: Wheel-down wraps last → first
- Action: wheel-down once.
- Expected: ▌ back on `local`.

## Test 6: Wheel-up wraps first → last
- Action: wheel-up once.
- Expected: ▌ on `feature-c`.

## Test 7: The first click is the focus-claim — it is swallowed
- Action: after the wheel steps the sidebar pane is not click-focused. Capture; find the `feature-b` title line; left-click COL=10 on that line; capture.
- Expected: ZERO change — ▌ stays where the wheel left it, no activation, no `Spawning` message, active tab still `zelligent-test-repo`. Zellij's click-to-focus ate this click (it recurs after every cross-tab landing, not just startup). Count real clicks from the NEXT one.

## Test 8: A real click select+activates EXACTLY that detached row (zero-offset spawn)
- Action: the sidebar is now click-focused. Left-click the `feature-b` title line.
- Expected: single-click select+activate — ▌ moves to `feature-b` AND it activates. `feature-b` is detached, so status shows `Spawning 'feature-b'...` (green); after ~8s a tab named `feature-b` exists and is active; `feature-b` row title now BOLD CYAN; subtitle still `branch: feature-b`. It must be `feature-b` — NOT feature-a, NOT feature-c (zero-offset mapping, the off-by-one hunt). Still exactly 4 rows; no duplicate `feature-b` row, no stray `user tab` row (self-heal check).

## Test 9: Subtitle click maps to its OWN item (subtitle-offset bug FIXED)
- Action: you just landed in the `feature-b` tab, so its sidebar is not click-focused. Left-click the `branch: feature-a` SUBTITLE line once (focus-claim — expect no change), then left-click the same subtitle line again.
- Expected: the second (real) click moves ▌ to `feature-a` and activates it (spawns — feature-a is detached). The subtitle maps to `feature-a` at ZERO offset — the historical "clicking a SUBTITLE selects the NEXT item" deviation is FIXED (#135/#137). It must NOT land on feature-b or local.

## Test 10: Click on the header line is a no-op (no item-0 offset)
- Action: you landed in the feature-a tab. Left-click the ` zelligent-test-repo ` header line once (focus-claim — no change), then click the header line again.
- Expected: the second header click does nothing — ▌ unchanged, no activation, no spawn message. If ▌ jumps to `local` or any activation fires, that is an offset-mapping bug — FAIL and write repro.

## Test 11: Blank and footer clicks are no-ops
- Action: the sidebar is now click-focused. Click the blank separator line directly above the `local` title; capture. Click a blank line 2–3 lines below `branch: feature-c`; capture. Click the `n branch  i new  d del  r ↻` footer line; capture.
- Expected: none of these change selection or activate anything.

## Test 12: Right-click is a no-op
- Action: right-click the `feature-c` title line.
- Expected: no selection change; nothing activates.

## Test 13: Click on an OPEN worktree row switches (does not spawn)
- Action: `feature-a` and `feature-b` tabs are open now. Ensure the sidebar is click-focused (spend a focus-claim click on a title line you intend to land on anyway if it is not). Click the `feature-b` title line.
- Expected: no `Spawning` message; active tab switches to `feature-b` (bold cyan + main-pane frame title agree); NO new tab created (tab count unchanged).

## Test 14: Click `local` switches back to the repo tab
- Action: sidebar focused; click the `local` title line.
- Expected: active tab = `zelligent-test-repo`; `local` BOLD CYAN; feature titles plain. One real click (after focus is claimed) both selects and switches — there is no separate select step.

## Test 15: Keyboard parity — j/k are cursor-only, Enter activates
- Action: sidebar focused; press `k`, capture; press `k`, capture; press Enter.
- Expected: each `k` moves ▌ up one row with wrap semantics identical to the wheel and WITHOUT activating anything (keyboard navigation is cursor-only); Enter then activates the ▌ row (switch if its tab is open, spawn if detached) exactly as a real click would.

## Test 16: Full-screen anomaly sweep
- Action: capture both windows' full output once more.
- Expected: no garbled lines, no SCROLL: N/N counter, no duplicated sidebar frames, no orphan tabs. List anything odd. Expect a burst of `Action CliPipe did not complete within 1s timeout` lines correlated with the spawns/switches above (known open observation — see the README); record the count, do not re-diagnose.
