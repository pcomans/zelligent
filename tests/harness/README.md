# UI Test Harness

Agent-driven UI tests for the zelligent Zellij plugin using Chrome DevTools MCP.

## Structure

```
tests/harness/
├── plans/          # Test plan markdown files
│   ├── empty-repo.md
│   └── with-worktrees.md
├── fixtures/       # Setup scripts (one per scenario)
│   ├── setup-empty-repo.sh
│   └── setup-with-worktrees.sh
└── README.md
```

## How it works

1. Each **test plan** is a markdown file with test steps and a `fixture` reference
2. Each **fixture** is a shell script that creates the test repo state
3. The **test-driver** subagent (`.claude/agents/test-driver.md`) reads the plan, runs the fixture, bootstraps a Zellij web session, executes all test steps via Chrome DevTools MCP, and reports results

## Running a test plan

Ask Claude to run a specific test plan:

```
Run the test plan at tests/harness/plans/with-worktrees.md
```

Claude delegates to the `test-driver` subagent (Sonnet), which handles everything autonomously.

**Important:** Test plans must run sequentially, not in parallel. They share the same Zellij web port (8083) and test repo path (`/tmp/zelligent-test-repo`), so concurrent runs will conflict.

## Writing a new test plan

1. Create a fixture script in `fixtures/` if you need a new repo state
2. Create a plan in `plans/` with frontmatter pointing to the fixture:

```markdown
---
fixture: setup-my-scenario.sh
---

# My Test Scenario

## Test 1: Something works
- Action: Press Ctrl+Y
- Expected: The plugin opens
```

## Fixture scripts

Fixture scripts must:
- Create the test repo at `/tmp/zelligent-test-repo`
- Print `REPO_DIR=/tmp/zelligent-test-repo` to stdout
- Be idempotent (clean up before setting up)
