---
fixture: setup-with-worktrees.sh
launch: zelligent  # INSTALLED CLI — never the fixture clone's ./zelligent.sh (old main; see README "CLI under test")
session_name: zelligent-test-repo
---

# UI Audit 01 — Core Mouse Interaction Contract

Exhaustive check of click/wheel semantics on the persistent sidebar with the
seeded 3-worktree fixture. Hunts: click-activates-wrong-row (offset bugs),
selection/active-highlight desync, two-click contract violations.

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

- Sidebar pane = 36 cols. Header: ` zelligent / zelligent-test-repo ` + `─` fill, bold cyan.
- Each item = 2 lines. Title line: `▌ name` (cyan bar + space) when the cursor is on it, else two spaces + name. Subtitle line (dim): `branch: X` for worktrees, `current repo` for local, `user tab` for manual tabs.
- The Zellij-active tab's row title is BOLD CYAN — independent axis from the ▌ cursor.
- Footer (36 cols = narrow variant): `↑/k up  ↓/j down  Enter open` / `n branch  i new  d del  r ↻` / version line.
- Two-click contract: first click on a non-selected row moves ▌ only; second click on the already-selected row activates (switch if tab exists, spawn if detached). Blank/header/footer clicks: no-op. Wheel: ▌ moves one row, wraps at both ends. Right-click: no-op.

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

## Test 7: First click on a non-selected row selects EXACTLY that row
- Action: capture, find the line containing `feature-b` (title line), left-click COL=10 on that line.
- Expected: ▌ moves to `feature-b` — NOT feature-a, NOT feature-c (off-by-one hunt). Active tab still `zelligent-test-repo`; NO spawn message; tab bar unchanged.

## Test 8: Clicking a SUBTITLE line maps to the same item
- Action: click the `branch: feature-a` subtitle line.
- Expected: ▌ moves to `feature-a` (title+subtitle map to one item). No activation (feature-a was not selected).

## Test 9: Click on header line is a no-op
- Action: click the ` zelligent / ...` header line.
- Expected: ▌ stays on `feature-a`; no activation, no spawn message. If ▌ jumps to `local` or any activation fires, that is an offset-mapping bug — FAIL and write repro.

## Test 10: Click on the blank separator line (between header and first row) is a no-op
- Action: click the blank line directly above the `local` title line.
- Expected: no selection change, no activation.

## Test 11: Click on a blank line below the list is a no-op
- Action: click a blank line 2–3 lines below `branch: feature-c`.
- Expected: no change.

## Test 12: Click on footer keybind line is a no-op
- Action: click the `n branch  i new  d del  r ↻` line.
- Expected: no change.

## Test 13: Right-click is a no-op
- Action: right-click the `feature-c` title line.
- Expected: ▌ still on `feature-a`; nothing activates.

## Test 14: Second click on selected detached row spawns THAT branch
- Action: click `feature-a` title line (already selected → second click).
- Expected: status area shows `Spawning 'feature-a'...` (green); after ~8s a tab named `feature-a` exists in the tab bar and is active; sidebar still visible in the new tab; `feature-a` row title now BOLD CYAN; subtitle still `branch: feature-a`; still exactly 4 rows — no duplicate feature-a row, no stray `user tab` row (self-heal check).

## Test 15: Switch back to repo tab via two clicks on `local`
- Action: click `local` title line (selects), capture, click again (activates).
- Expected: after first click ▌ on `local` while `feature-a` keeps bold cyan; after second click active tab = `zelligent-test-repo`, `local` bold cyan, `feature-a` plain.

## Test 16: Two-click on an OPEN worktree row switches (not spawns)
- Action: click `feature-a` row, capture, click again.
- Expected: no `Spawning` message; active tab becomes `feature-a` (tab bar + bold cyan agree); no new tab created (tab count unchanged).

## Test 17: Keyboard parity — j/k/Enter agree with mouse
- Action: (sidebar pane has focus from prior clicks) press `k`, capture; press `k`, capture; press Enter.
- Expected: each `k` moves ▌ up one row with wrap semantics identical to wheel; Enter on `local` switches to the repo tab exactly as the second click did.

## Test 18: Full-screen anomaly sweep
- Action: capture both windows' full output once more.
- Expected: no garbled lines, no SCROLL: N/N counter, no duplicated sidebar frames, no orphan tabs in tab bar. List anything odd.
