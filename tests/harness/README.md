# UI Test Harness

Agent-driven UI tests for zelligent using a tmux-based harness and the tmux
skill.

For PR 83 and later persistent-sidebar work, the harness is for visible,
end-to-end behavior:

- startup through `zelligent`
- sidebar-visible session layout
- spawned tab layout
- manual-tab sidebar inheritance

Contract and failure-path coverage such as layout precedence, placeholder
validation, and `doctor` behavior remains in `bash test.sh`.

## Structure

```
tests/harness/
├── plans/          # Test plan markdown files
│   ├── empty-repo.md
│   ├── sidebar-mouse-interaction.md
│   ├── sidebar-layout-smoke.md
│   ├── with-worktrees.md
│   └── ui-audit-01..06-*.md   # exhaustive mouse/tab audit suite (2026-07)
├── fixtures/       # Setup scripts (one per scenario)
│   ├── setup-empty-repo.sh
│   ├── setup-with-worktrees.sh
│   ├── setup-many-worktrees.sh
│   └── teardown.sh
└── README.md
```

## How it works

1. Each **test plan** is a markdown file with test steps and frontmatter:
   - `fixture`: setup script
   - `launch`: command to start in the view window, usually `./zelligent.sh`
   - `session_name`: Zellij session name to target from the control window
2. Each **fixture** is a shell script that creates the test repo state
3. The **test-driver** subagent (`.claude/agents/test-driver.md`) reads the plan, runs the fixture, wraps Zellij in a tmux session, executes all test steps by reading `capture-pane` output, and reports results
4. The **tmux skill** (`.claude/skills/tmux/SKILL.md`) is available for manual proofs, inspection, and ad hoc UI interaction outside the automated test-driver flow

### tmux harness architecture

```
tmux session: zt-driver  (isolated socket: zt-driver-test)
├── window 0 "view"  — runs the plan's `launch` command, usually `zelligent`
└── window 1 "ctrl"  — runs shell commands to drive the test
```

## Running a test plan

Ask Claude to run a specific test plan:

```
Run the test plan at tests/harness/plans/with-worktrees.md
```

Claude delegates to the `test-driver` subagent, which handles everything autonomously.

**Important:** Test plans must run sequentially, not in parallel. They share the
same tmux socket (`zt-driver-test`) and test repo path
(`/tmp/zelligent-test-repo`), and each plan controls its own Zellij session, so
concurrent runs will conflict.

Prerequisites:

- `zellij` and `tmux` installed locally
- a built sidebar plugin available to the repo-local script, typically via
  `ZELLIGENT_PLUGIN_SRC="$HOME/.local/share/zelligent/zelligent-plugin.wasm"`
  after `bash dev-install.sh`

Current plans:

- `sidebar-layout-smoke.md`: PR 83 smoke test for startup, spawned tabs, and
  manual-tab inheritance
- `empty-repo.md`: empty-state sidebar startup in a repo with no worktrees
- `sidebar-mouse-interaction.md`: wheel navigation and click-to-select/open
  behavior in the persistent sidebar
- `with-worktrees.md`: embedded sidebar stability in a repo with seeded
  worktrees
- `ui-audit-01-mouse-core.md` … `ui-audit-05-agent-status-modes.md`: exhaustive
  real-input audit of mouse selection/activation, multi-tab switching, scrolled
  viewports, worktree lifecycle staleness, and agent-status glyphs. Written for
  the 2026-07 UI audit (see `docs/reports/ui-audit-2026-07-05.md` if present);
  each plan carries a "Harness corrections" section with hard-won tmux/SGR
  driving rules — read it before running.
- `ui-audit-06-repro-verification.md`: minimal from-clean-fixture repros for
  every bug found in the audit (Z-1 through Z-8). Use as a regression suite
  when fixing those bugs.

**CLI-under-test rule:** fixtures clone the source repo's checked-out branch
(usually `main`) into `/tmp/zelligent-test-repo`, so `./zelligent.sh` inside the
test repo is the OLD CLI. Plugin changes are injected via
`ZELLIGENT_PLUGIN_SRC`, but to test CLI changes you must invoke the installed
`zelligent` (from `bash dev-install.sh` of the branch under test) — verify with
`command -v zelligent` plus a `grep` for the change. A prior #140 verification
produced a false FAILED verdict by running the fixture clone's script.

Harness driving rules learned in the audit (apply to all mouse plans): enable
`tmux set-option -g mouse on` on the harness socket before SGR click injection
and send press/release as separate `send-keys` calls; there is no tab bar —
verify the active tab via the main pane's frame title, the sidebar's bold-cyan
row, and `zellij action query-tab-names`; never run `zelligent.sh spawn` from a
control pane (it attaches a second Zellij client).

## Writing a new test plan

1. Create a fixture script in `fixtures/` if you need a new repo state
2. Create a plan in `plans/` with frontmatter pointing to the fixture:

```markdown
---
fixture: setup-my-scenario.sh
launch: ZELLIGENT_PLUGIN_SRC="$HOME/.local/share/zelligent/zelligent-plugin.wasm" ./zelligent.sh
session_name: zelligent-test-repo
---

# My Test Scenario

## Test 1: Something works
- Action: Press a key or run a shell command described in the plan
- Expected: The resulting UI state matches the plan
```

## Fixture scripts

Fixture scripts must:
- Create the test repo at `/tmp/zelligent-test-repo`
- Seed any repo-local layout files the plan depends on
- Print `REPO_DIR=/tmp/zelligent-test-repo` to stdout
- Be idempotent (clean up before setting up)

The `teardown.sh` script kills the isolated tmux socket, removes the known test
Zellij sessions, clears stale session resurrection state, and removes the
temporary repo and worktrees.

## Harness Learnings

- Test setup must be boring and deterministic. If teardown "usually" works, it is broken.
- Treat session isolation as part of the test contract. If stale tabs or stale session state leak across runs, the result is invalid.
- Prefer absolute paths when driving tmux on macOS. `/tmp` often resolves to `/private/tmp`, and getting that wrong creates fake failures.
- Run harness setup serially. Parallel setup made failures harder to trust than the product itself.
