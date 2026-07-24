---
fixture: setup-with-worktrees.sh
launch: zelligent  # INSTALLED CLI — never the fixture clone's ./zelligent.sh (old main; see README "CLI under test")
session_name: zelligent-test-repo
---

# UI Audit 01 — Core Mouse Interaction Contract

Exhaustive check of click/wheel semantics on the persistent sidebar with the
seeded 3-worktree fixture. Hunts: click-activates-wrong-row (offset bugs),
selection/active-highlight desync, single-click select+activate contract
violations.

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
- Footer (#192, state-aware — this fixture always has items, so the minimal empty-state footer doesn't apply): two lines — `↑/↓  Enter open  r refresh` / `n pick  i new` — plus `d remove` appended to the second line (`n pick  i new  d remove`) ONLY when the selected row is a removable worktree tab (`selected_sidebar_branch().is_some()`); the `local` row is never removable, so it's hidden whenever ▌ sits on `local` — / version line.
- Interaction contract: see `docs/PRODUCT_SENSE.md` § "Sidebar interaction contract" — one normative source, do not restate it elsewhere. Plan-specific driving note: a single left click on a row's title OR subtitle line selects AND activates it in the same click (switch if a tab exists, spawn if detached); clicking the already-selected/active row re-activates idempotently (no duplicate spawn). Blank/header/footer/past-end clicks: no-op. Wheel: ▌ moves one row, wraps at both ends, never activates. Right-click: no-op.

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

## Test 7: Click on a non-selected row selects AND activates EXACTLY that row (spawn case)
- Action: capture, find the line containing `feature-b` (title line), left-click COL=10 on that line; wait 8s.
- Expected: ▌ moves to `feature-b` — NOT feature-a, NOT feature-c (off-by-one hunt), in the SAME click; status area shows `Spawning 'feature-b'...` (green); after the wait a tab named `feature-b` exists and is active (bold cyan); sidebar still visible in the new tab.

## Test 8: Clicking a SUBTITLE line maps to the same item and activates it
- Action: click the `branch: feature-a` subtitle line; wait 8s.
- Expected: ▌ AND active tab move to `feature-a` (title+subtitle map to one item, off-by-one hunt) — NOT feature-b, NOT feature-c. Since feature-a is detached, this spawns it: `Spawning 'feature-a'...`, then `feature-a` bold cyan and active after the wait.

## Test 9: Click on header line is a no-op
- Action: click the ` zelligent-test-repo ` header line.
- Expected: ▌ and active tab stay on `feature-a`; no activation, no spawn message. If ▌ jumps to `local` or any activation fires, that is an offset-mapping bug — FAIL and write repro.

## Test 10: Click on the blank separator line (between header and first row) is a no-op
- Action: click the blank line directly above the `local` title line.
- Expected: no selection change, no activation.

## Test 11: Click on a blank line below the list is a no-op
- Action: click a blank line 2–3 lines below `branch: feature-c`.
- Expected: no change.

## Test 12: Click on footer keybind line is a no-op
- Action: click the `n pick  i new  d remove` line (▌ is on `feature-a`, a removable worktree row, so `d remove` is showing — see the rendering contract's #192 note).
- Expected: no change.

## Test 13: Right-click is a no-op
- Action: right-click the `feature-c` title line.
- Expected: ▌ and active tab still on `feature-a`; nothing activates.

## Test 14: Click on the already-selected/active row re-activates idempotently
- Action: click `feature-a` title line again (it is already selected AND already the active tab).
- Expected: no NEW `Spawning` message, no duplicate tab; tab count unchanged (`local`, `feature-b`, `feature-a`); `feature-a` remains bold cyan and active; still exactly 4 sidebar rows — no stray `user tab` row (self-heal check).

## Test 15: Single click on `local` switches back to the repo tab
- Action: click `local` title line; wait ~1s.
- Expected: in the SAME click, ▌ AND active tab move to `local`: active tab becomes `zelligent-test-repo`, `local` bold cyan, `feature-a` title now plain. No `Spawning` message (the repo tab already exists).

## Test 16: Single click on an OPEN worktree row switches (not spawns)
- Action: click the `feature-b` title line (already has a tab from Test 7, currently not selected/active).
- Expected: no `Spawning` message; active tab becomes `feature-b` in the same click (tab bar + bold cyan agree); no new tab created (tab count unchanged).

## Test 17: Keyboard parity — j/k select, Enter activates, matching the mouse contract
- Action: (sidebar pane has focus from prior clicks) press `k`, capture; press `k`, capture; press Enter.
- Expected: each `k` moves ▌ up one row with wrap semantics identical to wheel, WITHOUT activating (active tab stays `feature-b` through both presses — keyboard selection and activation are separate steps, unlike the mouse's combined click); `k` then `k` from `feature-b` lands ▌ on `local`; Enter activates the selected `local` row and switches to the repo tab exactly as a single click would.

## Test 18: Full-screen anomaly sweep
- Action: capture both windows' full output once more.
- Expected: no garbled lines, no SCROLL: N/N counter, no duplicated sidebar frames, no orphan tabs in tab bar. List anything odd.
