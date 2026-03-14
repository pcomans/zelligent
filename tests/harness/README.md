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
