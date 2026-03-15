# Docs / Harness Follow-up

Remaining follow-up after the persistent-sidebar stack landed on `main`.

## Docs cleanup

- [x] Remove stale `Ctrl-y` / floating-plugin language from harness plans:
  - [tests/harness/plans/empty-repo.md](/Users/philipp/code/zelligent/tests/harness/plans/empty-repo.md)
  - [tests/harness/plans/sidebar-layout-smoke.md](/Users/philipp/code/zelligent/tests/harness/plans/sidebar-layout-smoke.md)
  - [tests/harness/plans/with-worktrees.md](/Users/philipp/code/zelligent/tests/harness/plans/with-worktrees.md)

- [x] Update [docs/design-docs/tab-management.md](/Users/philipp/code/zelligent/docs/design-docs/tab-management.md) to describe the current persistent-sidebar behavior.
  - Replace `Ctrl-Y floating UI` wording.
  - Remove `then closes the plugin` from switch-to-tab behavior.
  - Keep the name-based tab-operation guidance intact.

- [x] Audit current-facing docs for any remaining stale floating-sidebar wording and fix only non-historical references.
  - Likely candidates:
    - [tests/harness/README.md](/Users/philipp/code/zelligent/tests/harness/README.md)
    - [README.md](/Users/philipp/code/zelligent/README.md)
    - [AGENTS.md](/Users/philipp/code/zelligent/AGENTS.md)

- [ ] Decide whether the historical docs need a short note clarifying that they are implementation history, not current behavior.
  - Likely candidates:
    - [docs/design-docs/sidebar-rewrite-history.md](/Users/philipp/code/zelligent/docs/design-docs/sidebar-rewrite-history.md)
    - [docs/design-docs/sidebar-stack-rollout.md](/Users/philipp/code/zelligent/docs/design-docs/sidebar-stack-rollout.md)

## Harness coverage

- [x] Add a focused sidebar navigation harness plan.
  - Suggested file: [tests/harness/plans/sidebar-navigation.md](/Users/philipp/code/zelligent/tests/harness/plans/sidebar-navigation.md)
  - Cover `j`/`k`, selection movement, and `Enter` opening the selected item.

- [x] Add a focused delete-confirm harness plan.
  - Suggested file: [tests/harness/plans/sidebar-delete-confirm.md](/Users/philipp/code/zelligent/tests/harness/plans/sidebar-delete-confirm.md)
  - Cover `d`, confirm/cancel behavior, and the user-tab rejection case.

- [x] Add a focused agent-status harness plan.
  - Suggested file: [tests/harness/plans/sidebar-agent-status.md](/Users/philipp/code/zelligent/tests/harness/plans/sidebar-agent-status.md)
  - Cover status rendering transitions and any notification-adjacent visible UI behavior.

- [ ] Decide whether mouse-specific harness coverage is still worth adding now that mouse support is merged.
  - If yes, add a dedicated plan instead of relying only on unit tests.
  - Historical reference mentions `sidebar-mouse-interaction.md`, but that file does not exist on `main`.

- [ ] Decide whether the old `mixed-three-tabs-switching` scenario is still useful.
  - Historical reference exists, but the plan is not on `main`.
  - If useful, restack it from current sidebar behavior instead of resurrecting the old branch diff.

- [x] Update [tests/harness/README.md](/Users/philipp/code/zelligent/tests/harness/README.md) after adding any new plans so the listed plan set matches reality.

## Scope guard

- [ ] Keep this follow-up docs/harness-only.
  - No plugin behavior changes.
  - No CLI/layout changes.
  - No model changes unless a test cannot be expressed without one.
