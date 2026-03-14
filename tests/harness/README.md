# UI Test Harness

Agent-driven UI tests for zelligent using tmux to wrap a real Zellij session.

## Structure

```
tests/harness/
├── plans/          # Test plan markdown files
│   ├── empty-repo.md
│   ├── persistent-sidebar-behavior.md
│   └── with-worktrees.md
├── fixtures/       # Setup scripts (one per scenario)
│   ├── setup-empty-repo.sh
│   ├── setup-with-worktrees.sh
│   └── teardown.sh
└── README.md
```

## How it works

1. Each **test plan** is a markdown file with test steps and a `fixture` reference
2. Each **fixture** is a shell script that creates the test repo state
3. The **test-driver** subagent (`.claude/agents/test-driver.md`) reads the plan, runs the fixture, starts zelligent inside tmux, executes all test steps against the live terminal, and reports results

## Running a test plan

Ask Claude to run a specific test plan:

```
Run the test plan at tests/harness/plans/persistent-sidebar-behavior.md
```

Claude delegates to the `test-driver` subagent (Sonnet), which handles everything autonomously.

**Important:** Test plans must run sequentially, not in parallel. They share the same Zellij web port (8083) and test repo path (`/tmp/zelligent-test-repo`), so concurrent runs will conflict.

## What to cover

Use the harness for behavior that snapshots cannot prove:

- persistent sidebar stays visible across navigation
- key handling (`Enter`, `q`, `Esc`, `d`, `y`, `n`)
- tab switching across managed worktree tabs and user tabs
- deletion behavior for managed worktrees versus non-removable rows
- startup/session behavior in a live Zellij session

## Writing a new test plan

1. Create a fixture script in `fixtures/` if you need a new repo state
2. Create a plan in `plans/` with frontmatter pointing to the fixture:

```markdown
---
fixture: setup-my-scenario.sh
---

# My Test Scenario

## Test 1: Something works
- Action: Start zelligent in the repo
- Expected: The persistent sidebar is visible
```

## Fixture scripts

Fixture scripts must:
- Create the test repo at `/tmp/zelligent-test-repo`
- Print `REPO_DIR=/tmp/zelligent-test-repo` to stdout
- Be idempotent (clean up before setting up)

## Harness Learnings

These came from running the tmux harness against the persistent-sidebar stack.

- Test setup must be boring and deterministic. If teardown "usually" works, it is broken. Treat fixture cleanup as part of the product quality bar, not as disposable test glue.
- Treat session isolation as a first-class test requirement. If the Zellij session is not clean, the plan result is not trustworthy.
- Clear `ZELLIJ` and `ZELLIJ_SESSION_NAME` before starting the repo-under-test `zelligent.sh` from tmux. If you do not, the shell can think it is already inside another Zellij session and attach to the wrong place.
- Prefer absolute paths in tmux commands. On macOS, `/tmp` resolves to `/private/tmp`, and shell startup can ignore the expected working directory.
- Run tmux setup commands sequentially. Parallel session/window creation was flaky during live runs and produced misleading setup failures.
- `zelligent.sh nuke` may report success while stale resurrection state still exists. If the test session keeps coming back with old tabs, inspect and remove the Zellij cache entry at `~/Library/Caches/org.Zellij-Contributors.Zellij/<version>/session_info/zelligent-test-repo/` on macOS.
- A contaminated harness session showed unrelated tabs such as `feature-d` in supposedly clean fixtures. Treat that as a harness/environment failure, not as a product result.
- The live harness did verify one important behavior on the persistent-sidebar stack: deleting a pure user tab row is a no-op.

## Open Tasks

- Make the harness startup path explicitly open the zelligent plugin. The current plans assume the persistent sidebar appears after launching `zelligent.sh`, but the live session only shows the base Zellij tab and the built-in `About Zellij` pane.
- Update the plans to verify preconditions before asserting sidebar behavior. If the plugin is not visible, fail fast with a setup error instead of pretending later keypress failures are product regressions.
- Add one harness assertion that the `view` window starts in `/private/tmp/zelligent-test-repo` before running `zelligent.sh`. This catches the easy tmux cwd bug immediately.
- Add one harness assertion that teardown removed both the live `zelligent-test-repo` processes and the `session_info` cache entry before the next plan starts.
