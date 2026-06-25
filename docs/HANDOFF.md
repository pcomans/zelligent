# HANDOFF — zelligent (Architect Loop)

> Repo memory for the Architect Loop. The architect (Claude) maintains this each
> session — consolidating builder lane reports and writing rulings and verdicts.
> Raw evidence only in the results sections. Not in this file = didn't happen.

## TL;DR

- Goal: fix GitHub issue #132 — sidebar worktree click should select **and**
  activate in one click.
- Last slice: `sidebar-click-activate` — **dispatched**, awaiting builder + next-session judgment.
- Next action: when the builder run completes, judge against
  `docs/gates/sidebar-click-activate.md` (run G1/G2/G3 myself), post-flight, integrate.

## Project goal

Zelligent spawns AI coding agents into isolated git worktrees, each in its own
Zellij tab (CLI `zelligent.sh` + Rust WASM plugin in `plugin/`). "Done" for this
slice = a single left-click on a sidebar worktree item both moves selection and
activates it, with the mouse-handling unit tests proving it and the full suite green.

## Verification gate (exact commands)

```
TARGET="$(rustc -vV | awk '/^host:/ {print $2}')"
cd plugin && cargo test --target "$TARGET"   # plugin unit + render snapshot tests
bash test.sh                                  # full repo suite (run from repo root)
```
This devcontainer uses a rustup toolchain (host x86_64-unknown-linux-gnu); the
Homebrew PATH workaround in CLAUDE.md is not needed here but is harmless.

## Frozen contracts

None beyond the gate file — this is a single-function bugfix.

## Current slice

- Spec: fix `State::handle_mouse_browse` `Mouse::LeftClick` arm in `plugin/src/lib.rs`
  (around line 734) so a click on any resolvable item sets `selected_index` and
  returns `action_for_sidebar_item(idx)`, instead of only activating when the click
  lands on the already-selected item.
- Gates: `docs/gates/sidebar-click-activate.md` (frozen at the freeze commit BELOW).
- Lanes: 1 lane (`sidebar-click-activate-01`), files: `plugin/src/lib.rs` only.
  Report: `docs/lanes/sidebar-click-activate-01.md`.
- Effort: `high` — routine, tightly specified single-file change.

| Gate | Command | Threshold | Raw result | Architect verdict |
|------|---------|-----------|------------|-------------------|
| G1 | `cargo test … browse_mouse_single_click_activates_unselected_item` | 1 passed | pending | pending |
| G2 | `cargo test …` (all plugin tests) | 0 failed | pending | pending |
| G3 | `bash test.sh` | exit 0 | pending | pending |

## Raw results (latest run)

Baseline (pre-fix), 2026-06-25: the 5 `browse_mouse_click_*` tests pass and encode
the current two-click behavior; `browse_mouse_click_selects_clicked_item` and
`browse_mouse_click_second_item_subtitle_selects_second_item` assert `Action::None`
on a click to a non-selected item (the bug). Builder results pending.

## Open disagreements (builder writes; architect rules)

| # | Builder's position | Spec's position | Evidence | Ruling |
|---|--------------------|-----------------|----------|--------|
| — | (none yet — PHASE 0 pending) | | | |

## Decisions log

| Date | Decision | Why |
|------|----------|-----|
| 2026-06-25 | Single lane, main checkout, effort `high` | One ~3-line logic change + test updates in one file; no parallelism to gain. |
| 2026-06-25 | Gate G1 requires a named test calling the production `handle_mouse_browse` | Prevents a vacuous test that re-derives expected values; the old tests encoded the buggy contract. |

## Next slice (builder may propose; architect decides)

TBD after #132 lands. Other open issues: #124 (recursion guard in spawn),
#118 (stray stdout lines on remove), #117 (`__COMMIT_SHA__` placeholder).

## Session log

| Date | Role | Slice | Commits | Gates P/F | Notes |
|------|------|-------|---------|-----------|-------|
| 2026-06-25 | Architect | sidebar-click-activate | freeze pending | pending | Grounded, froze gates, dispatched builder. |
