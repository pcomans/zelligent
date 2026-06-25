# Gates — sidebar-click-activate (issue #132)

> FROZEN before dispatch. Read-only for everyone including the builder.
> Any change to this file under `git diff` is an automatic slice FAIL.

Issue #132: "Clicking on a worktree title in the sidebar selects it, but does
not activate it." A single left-click on a sidebar worktree item must both move
selection to it AND activate it (switch to its tab, or spawn if detached) in one
click — not require a second click.

Host target is captured once: `TARGET="$(rustc -vV | awk '/^host:/ {print $2}')"`.

## G1 — behavioral, single-click activates a not-yet-selected item

A unit test in `plugin/src/lib.rs` named exactly
`browse_mouse_single_click_activates_unselected_item` must:

- build a `State` in `Mode::BrowseWorktrees` with ≥2 **attached** worktree items
  using the existing test constructor (`state_with_sidebar()`), `last_rows` set,
  and `selected_index` pointing at an item OTHER than the one about to be clicked;
- call the production function `State::handle_mouse_browse(&Mouse::LeftClick(line, col))`
  where `line` maps (via `sidebar_index_at_line`) to a NON-selected attached item;
- assert BOTH:
  - the returned value is `Action::SwitchToTab(<that item's tab name>)` — i.e. NOT
    `Action::None`;
  - `selected_index` afterwards equals the clicked item's index.

The expected tab name must come from that item's real data, not a bare literal
re-derived independently of the production path.

Command:
```
cd plugin && cargo test --target "$TARGET" browse_mouse_single_click_activates_unselected_item 2>&1
```
PASS threshold: `1 passed; 0 failed`.

## G2 — no regression in mouse handling

All existing mouse-click invariants still hold. These named tests must still pass
(updated in body where the old two-click contract was asserted, but the listed
behaviors below must remain true):

- `browse_mouse_click_on_selected_item_activates_it` — clicking the already-selected
  attached item returns `Action::SwitchToTab("feat-b")`.
- `browse_mouse_click_on_selected_detached_item_spawns_it` — clicking a detached
  item returns `Action::Spawn("feat-a")` and sets the spawning status message.
- `browse_mouse_click_ignores_non_item_lines` — a click on a line that maps to no
  item returns `Action::None` and leaves `selected_index` unchanged (0).
- `browse_mouse_noop_in_empty_state` — in empty state, scroll and click both
  return `Action::None` and leave `selected_index` at 0.

Command:
```
cd plugin && cargo test --target "$TARGET" 2>&1
```
PASS threshold: `0 failed` across lib, main, and render_snapshots test binaries.

## G3 — full repository suite

Command (from repo root):
```
bash test.sh
```
PASS threshold: exit code 0.

## Out-of-band (architect post-flight, not builder-run)

- `git diff` on `docs/gates/` since the freeze commit is empty.
- `git status` shows only `plugin/src/lib.rs` modified (plus the lane report under
  `docs/lanes/`). Any other touched file fails the lane.
- No builder commits on the branch.
