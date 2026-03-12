# PR 83 Layout Follow-Up Plan

This document captures the follow-up work identified after the initial sidebar
layout foundation landed.

## Summary

Refactor zelligent so layout behavior comes from one shared pipeline and one
canonical default layout asset, not from duplicated KDL generation in
`zelligent.sh`. The runtime should always resolve a layout file, validate the
required sidebar placeholder, substitute context values, and then use that same
rendered source for initial session startup, spawned tabs, and manual-tab
inheritance. At the same time, remove the old `Ctrl-y` and floating-plugin
assumptions from setup, docs, and tests.

## Key Changes

### 1. Canonical layout asset and resolution

- Introduce a shipped default layout asset alongside other packaged shared
  assets:
  - Homebrew install: `share/zelligent/default-layout.kdl`
  - Dev install: `~/.local/share/zelligent/default-layout.kdl`
- Use one layout source order everywhere:
  1. repo-local `.zelligent/layout.kdl`
  2. user-level `~/.zelligent/layout.kdl`
  3. hard error if neither exists
- Remove the hardcoded built-in layout from `zelligent.sh`; the script should no
  longer embed the default tab body as heredoc KDL.

### 2. New layout contract

- Keep `.zelligent/layout.kdl` as a full layout file.
- Support exactly three template placeholders:
  - `{{zelligent_sidebar}}` required exactly once
  - `{{cwd}}` optional
  - `{{agent_cmd}}` optional
- Treat all three as plain text substitution.
- Leave unknown placeholders untouched.
- Do not add broader KDL validation yet; only enforce the placeholder contract
  for now.
- Update the documented contract to say repo and user layouts can control the
  entire inner body, but the sidebar is mandatory.

### 3. Shared rendering pipeline in `zelligent.sh`

- Add a single layout-resolution and rendering path used by both plain
  `zelligent` and `zelligent spawn`.
- Centralize:
  - plugin and default-layout path resolution
  - layout source selection
  - runtime validation of the selected layout when used
  - placeholder substitution
- Context-specific substitutions:
  - plain `zelligent`: `cwd = repo root`, `agent_cmd = $SHELL`
  - `spawn`: `cwd = worktree path`, `agent_cmd = explicit command or $SHELL`
- Fix the startup regression by eliminating hardcoded `bash` in the no-args
  path.

### 4. Session startup and manual-tab inheritance

- When plain `zelligent` creates a new session, it must also configure the
  session so future manual tabs inherit the zelligent sidebar frame.
- Use the shared rendered layout as the initial tab content, and build the
  session wrapper around it so manual tabs get the sidebar frame too.
- Treat this as a guaranteed invariant:
  - manual tabs must always inherit the sidebar frame
  - matching the full inner body is desirable, but if Zellij cannot fully
    control manual-tab bodies, document that limitation explicitly
- If the plugin cannot be resolved for startup, fail clearly instead of falling
  back to a plain Zellij session.

### 5. `doctor`, `init`, and install behavior

- Change `doctor` so it no longer adds the old `Ctrl-y` keybinding.
- `doctor` should:
  - create `~/.zelligent/layout.kdl` only if missing
  - compare the existing user-level layout byte-for-byte against the shipped
    default
  - if different, print an informational overwrite command and nothing more
- `doctor` should only talk about `~/.zelligent/layout.kdl`, not repo-local
  overrides.
- Keep `init`, but keep it limited to per-repo `setup.sh` and `teardown.sh`
  stubs; do not add KDL scaffolding.
- Update packaging and install flows so both Homebrew and dev installs ship the
  default layout asset next to the plugin.

### 6. Docs and design-history cleanup

- Update the source-of-truth docs so they match the new product:
  - `README.md`
  - `docs/PRODUCT_SENSE.md`
  - `docs/ARCHITECTURE.md`
  - `docs/references/zellij-kdl-layout.md`
  - any design or reference docs that still describe floating `Ctrl-y` behavior
    or tab-bar-based chrome
- Document explicitly:
  - layout source precedence
  - required `{{zelligent_sidebar}}` placeholder
  - optional `{{cwd}}` and `{{agent_cmd}}`
  - `cwd` semantics for initial vs spawned tabs
  - manual-tab sidebar inheritance rule and any verified Zellij limitation on
    body control
  - `doctor`'s new layout behavior
- Keep `docs/design-docs/sidebar-rewrite-plan.md`, but update it so it
  documents the implemented system rather than the stale or contradictory
  version.
- Add the relevant docs to `docs/design-docs/index.md`.

## Test Plan

- Update `test.sh` to cover:
  - no-args startup honors `$SHELL` instead of hardcoded `bash`
  - startup fails clearly if the sidebar plugin cannot be resolved
  - layout source precedence: repo override, then user default, then hard error
  - placeholder validation: `{{zelligent_sidebar}}` missing or duplicated fails
  - `doctor` creates `~/.zelligent/layout.kdl` only when missing
  - `doctor` reports layout drift with an overwrite command and does not rewrite
    the file
  - `doctor` no longer adds `Ctrl-y`
  - `dev-install.sh` installs the default layout asset next to the dev plugin
- Add or update headless integration checks to verify:
  - brand-new session startup includes the sidebar frame
  - spawned tabs use the same source layout contract
  - manual tabs inherit the sidebar frame in a zelligent-managed session
- Add doc-level acceptance checks where practical so the repo no longer claims
  floating `Ctrl-y` behavior after the refactor.

## Assumptions and Defaults

- Only placeholder validation is required for v1 of this refactor; stronger
  KDL-shape validation is deferred.
- Unknown placeholders remain untouched.
- The shipped default layout asset is treated as an install invariant.
- `doctor`'s layout comparison is byte-for-byte.
- If Zellij cannot fully control manual-tab bodies, the docs must state that
  only the sidebar-frame inheritance is guaranteed.
