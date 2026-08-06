---
fixture: setup-with-worktrees.sh
launch: zelligent  # INSTALLED CLI — never the fixture clone's ./zelligent.sh (old main; see README "CLI under test")
session_name: zelligent-test-repo
---

# Refresh-failure staleness (#216 / #219)

End-to-end proof of the reworked refresh lifecycle. When a `list-worktrees`
refresh cannot run, the sidebar must:

1. keep showing the last known worktree list (usable-but-flagged beats blank),
2. show a **persistent** yellow `⚠ stale · retrying — <reason>` marker that
   does NOT expire with the 8s status TTL (staleness is state, not an event),
3. make the full error recoverable on demand via the `e` key, and
4. actually retry — clearing the marker once the refresh can succeed again,
   whether via the backoff timer (no keypress) or a manual `r`.

It also proves the failure does NOT spin: the sidebar stays responsive the
whole time instead of pinning the CPU/descriptors with back-to-back respawns.

The fixture seeds three managed worktrees: `feature-a`, `feature-b`,
`feature-c`. The failure is simulated by moving the installed `zelligent` CLI
aside so the plugin's spawned `zelligent list-worktrees` fails to launch — the
same `io::Error` path the reported EMFILE (`os error 24`) took.

> Driving reminders (see README "Driving rules"): take a FRESH capture before
> every click; a not-yet-click-focused sidebar eats exactly one focus-claim
> click; capture BOTH plain (`-p`) and ANSI (`-p -e`) — the marker's yellow
> (`\x1b[33m`) is load-bearing; batch each step's action+sleep+capture into one
> shell call; NEVER `pkill zellij` and NEVER `zelligent spawn` from ctrl.

## Test 1: Build identity — prove what is under test
- Action: Read the sidebar footer version string, and in the control window run `zelligent --version` and `command -v zelligent`.
- Expected: The footer plugin version and the CLI version both match the branch under test (dev-install `0.2.X+<sha>`), and `command -v zelligent` resolves to the installed binary (e.g. under `~/.local/bin`). If either mismatches, STOP.

## Test 2: Healthy baseline — list visible, no marker
- Action: Wait for `launch: zelligent` to finish, capture the view pane (plain + ANSI).
- Expected: The left sidebar lists `feature-a`, `feature-b`, `feature-c`. There is NO `stale · retrying` line and NO yellow (`\x1b[33m`) marker.

## Test 3: Disable the CLI so the next refresh fails
- Action: In the control window, capture the path and move it aside in ONE call:
  `Z="$(command -v zelligent)"; mv "$Z" "$Z.disabled"; ls -l "$Z" "$Z.disabled" 2>&1 || true`
- Expected: `$Z` no longer exists; `$Z.disabled` does. (The running session is unaffected — only newly spawned `zelligent list-worktrees` processes will now fail.)

## Test 4: Trigger a refresh → list stays, marker appears
- Action: In the view window, click the sidebar once to claim focus (fresh capture first to locate a sidebar row), then press `r`. Record a timestamp (`date +%s`) at the keypress. Wait ~2s, capture plain + ANSI.
- Expected:
  - The worktree list is STILL `feature-a`, `feature-b`, `feature-c` — the last known list was NOT cleared.
  - A yellow (`\x1b[33m`) `⚠ stale · retrying — <reason>` marker is shown between the header and the list. (The `<reason>` reflects the spawn failure text; with the binary missing it is a generic first-line reason, not necessarily "too many open files".)
  - A transient red status line describing the failure may also be visible at this moment.

## Test 5: Staleness is STATE — the marker outlives the 8s status TTL
- Action: Using the timestamp from Test 4, wait until at least ~10s have elapsed since the `r` keypress (well past `STATUS_MESSAGE_TTL_SECS = 8`), then capture plain + ANSI. Batch the wait+capture in one shell call so tool overhead can't skew the timing.
- Expected: The transient red status line has cleared, but the yellow `⚠ stale · retrying` marker **persists**, and the list is still shown. This is the core #216 fix: a silently-frozen list is no longer possible.

## Test 6: Full error recoverable on demand — press `e`
- Action: Re-click the sidebar once to ensure keyboard focus, then press `e`. Wait ~1s, capture plain + ANSI.
- Expected: The full failure text (a `Failed to list worktrees: …` line) reappears in the status line — recoverable long after its original transient window closed. (Pressing `e` again re-shows it; it is not a one-shot.)

## Test 7: The failure does NOT spin
- Action: Over the stale period, confirm the sidebar stays interactive: press `j` then `k` (re-click to focus first), wait ~1s, capture. Optionally, from ctrl, sample `ps -eo comm | grep -c zelligent` a couple of seconds apart.
- Expected: Cursor navigation still responds and the marker/list remain stable — no runaway growth of `zelligent`/`git` processes, no frozen UI. (Pre-fix, every `TabUpdate` respawned two processes with no backoff.)

## Test 8: Restore the CLI
- Action: In the control window: `Z="$(command -v zelligent || true)"; SRC="${Z:+$Z}"; RB="$(ls "$HOME"/.local/bin/zelligent.disabled /usr/local/bin/zelligent.disabled 2>/dev/null | head -1)"; mv "$RB" "${RB%.disabled}"; ls -l "${RB%.disabled}"`
  (Equivalently, restore the exact `$Z.disabled` path recorded in Test 3.)
- Expected: The installed `zelligent` binary exists again at its original path.

## Test 9: Timer-driven retry clears the marker (no keypress)
- Action: WITHOUT pressing any key in the view window, wait up to ~35s, capturing plain + ANSI every ~5s in batched shell calls.
- Expected: On one of the backoff wake-ups the refresh now succeeds; the yellow `stale · retrying` marker DISAPPEARS on its own and the list re-renders cleanly. (This proves the retry is timer-driven, not dependent on a `TabUpdate` or a manual refresh.)
- Timing note: the retry cadence is the EXPONENTIAL backoff that started at the Test 4 `r` press (2s, then 4s, 8s, 16s, … capped at 60s). Only `r` resets the backoff — the `e`/`j`/`k` presses in Tests 6–7 do NOT — so by restore time the window may be ~16–30s; hence the generous wait. The next wake-up after the CLI is restored succeeds.
- Fallback (only if the marker has not cleared within the window): re-click the sidebar to focus and press `r` (which resets the backoff and retries immediately); the marker must then clear at once. Note in the report whether the timer path or the manual fallback cleared it.

## Test 10: Clean final state
- Action: Capture plain + ANSI one more time.
- Expected: `feature-a`, `feature-b`, `feature-c` listed, no `stale` marker, no yellow marker line, sidebar fully responsive — indistinguishable from the Test 2 baseline.

## Teardown note
If the plan aborts between Test 3 and Test 8, the `zelligent` binary is left as
`<path>.disabled`; restore it (`mv <path>.disabled <path>`) before any other
harness run, or every subsequent plan's launch will fail. `fixtures/teardown.sh`
does not know about this rename.
