# Sidebar Stack Fix Sequence

This document captures how follow-up fixes should land across the stacked
persistent-sidebar PRs.

## Summary

Fix the stacked PRs directly, in stack order, by editing each PR branch itself
rather than accumulating all work on `sidebar/layout-foundation`. The rule is:
land a fix in the earliest PR whose reviewed scope should contain it, then let
later PRs inherit it through the stack.

The sequence is:

1. `sidebar/model` for PR `#84`
2. `sidebar/render-core` for PR `#85`
3. `sidebar/visual-polish` for PR `#86`
4. `sidebar/mouse-support-clean` for PR `#87`
5. `sidebar/docs-followup` for PR `#88`

## Direct PR Fixes

### PR 84: `sidebar/model`

- Fix persistent-sidebar behavior at the model and action layer.
- Remove sidebar-destructive behavior from keyboard navigation:
  - `SwitchToTab` must not call `close_self()`
  - browse-mode `q` and `Esc` must be no-ops
- Keep all tab and worktree-derived sidebar item logic here.
- Add or update unit tests that prove keyboard tab switches keep the sidebar
  alive.
- If cursor restoration or active-tab mapping needs adjustment to support the
  fixed navigation model, do it here.

### PR 85: `sidebar/render-core`

- Update render and help output to match PR 84's behavior changes.
- Refresh snapshots so footer and help text no longer claim the sidebar can be
  quit from browse mode.
- Keep any sidebar row, subtitle, width-fitting, or empty-state rendering fixes
  here.
- Do not move behavior changes back into this PR unless they are rendering-only
  fallout.

### PR 86: `sidebar/visual-polish`

- Keep this PR strictly for visual fallout from the corrected render behavior.
- Only land spacing, highlight, header, or footer presentation changes here if
  they are needed after PR 85 snapshot updates.
- Do not land CLI, docs, or interaction semantics here.

### PR 87: `sidebar/mouse-support-clean`

- Verify mouse interactions respect the persistent-sidebar invariant.
- Fix any case where click-to-open, double-click, or scroll behavior can break
  selection, target the wrong row, or conflict with the new non-closing sidebar
  model.
- Add mouse-focused tests here only.

### PR 88: `sidebar/docs-followup`

- Bring docs and harness plans in sync with the actual final sidebar behavior
  from PRs 84 through 87.
- Remove remaining floating-plugin language and stale `Ctrl-y` assumptions from
  source-of-truth docs.
- Document the persistent-sidebar interaction model, including keyboard and
  mouse behavior as actually shipped.
- Update harness plans and design docs only after the runtime behavior in lower
  PRs is settled.

## Out of This Stack

- The CLI and layout-contract work from PR 83 remains separate from these direct
  PR fixes unless that PR is explicitly included in the review and fix sequence.
- PR 83 should not be treated as the landing spot for follow-up fixes that
  belong to PRs 84 through 88.

## Validation

- PR 84: plugin unit tests for action and model behavior.
- PR 85: snapshot tests for rendering, help, and footer output.
- PR 86: snapshot verification only if visuals change.
- PR 87: plugin tests for mouse interaction behavior.
- PR 88: doc and harness consistency check plus any affected repo tests.

## Defaults

- The fix sequence is direct on the named PR branches, not on their common
  base.
- Each fix lands in the lowest PR that should own it, then higher PRs are
  rebased or updated by stack inheritance.
- If a problem spans multiple PRs, the behavior fix goes in the earliest owning
  PR and later PRs only take the necessary fallout updates.
