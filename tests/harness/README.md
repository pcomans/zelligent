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
│   ├── sidebar-layout-smoke.md
│   └── with-worktrees.md
├── fixtures/       # Setup scripts (one per scenario)
│   ├── setup-empty-repo.sh
│   ├── setup-with-worktrees.sh
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
- `with-worktrees.md`: embedded sidebar stability in a repo with seeded
  worktrees
- `sidebar-navigation.md`: keyboard navigation, mode switching, and user-tab
  identity in the sidebar
- `sidebar-delete-confirm.md`: delete confirmation flow and user-tab removal
  guardrail
- `sidebar-agent-status.md`: visible status-indicator updates for managed tabs

Coverage notes:

- Mouse support currently relies on plugin tests plus manual tmux proofs. There is no dedicated mouse-only harness plan on `main`.
- The old `mixed-three-tabs-switching` scenario was not restacked. Its useful coverage is split across `sidebar-navigation.md` and `with-worktrees.md`.

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
