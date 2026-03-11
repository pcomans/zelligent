# UI Test Harness

Agent-driven UI tests for the zelligent Zellij plugin using tmux MCP.

## Structure

```
tests/harness/
├── plans/          # Test plan markdown files
│   ├── sidebar-layout-smoke.md
│   ├── sidebar-navigation.md
│   ├── sidebar-agent-status.md
│   ├── sidebar-delete-confirm.md
│   ├── empty-repo.md
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
3. The **test-driver** subagent (`.claude/agents/test-driver.md`) reads the plan, runs the fixture, wraps Zellij in a tmux session, executes all test steps by reading `capture-pane` output, and reports results

### tmux harness architecture

```
tmux session: zt-driver  (isolated socket: zt-driver-test)
├── window 0 "view"  — runs `zellij --session test-harness`  (read via capture-pane)
└── window 1 "ctrl"  — runs shell commands to drive the test
                       (zelligent spawn, zellij action, etc.)
```

The test driver uses `ZELLIJ=1 ZELLIJ_SESSION_NAME=test-harness zelligent spawn <branch>` from the ctrl window to open sidebar tabs into the running Zellij session. Terminal content is read with `mcp__tmux__capture-pane`.

## Running a test plan

Ask Claude to run a specific test plan:

```
Run the test plan at tests/harness/plans/sidebar-layout-smoke.md
```

Claude delegates to the `test-driver` subagent, which handles everything autonomously.

**Important:** Test plans must run sequentially, not in parallel. They share the same tmux socket (`zt-driver-test`), Zellij session name (`test-harness`), and test repo path (`/tmp/zelligent-test-repo`), so concurrent runs will conflict.

## Writing a new test plan

1. Create a fixture script in `fixtures/` if you need a new repo state
2. Create a plan in `plans/` with frontmatter pointing to the fixture:

```markdown
---
fixture: setup-my-scenario.sh
---

# My Test Scenario

## Test 1: Something works
- Action: Navigate down with `j`
- Expected: The second item is selected
```

## Fixture scripts

Fixture scripts must:
- Create the test repo at `/tmp/zelligent-test-repo`
- Print `REPO_DIR=/tmp/zelligent-test-repo` to stdout
- Be idempotent (clean up before setting up)

The `teardown.sh` script stops the Zellij web server (legacy, kept for compatibility) and cleans up any auth tokens.

## Limitations

- **Empty state not testable via harness:** The zelligent plugin's empty-state logo only appears when zelligent starts a brand-new single-tab session. The harness always adds a sidebar tab to an existing Zellij session (via `new-tab`), so the plugin will always see at least one existing tab and render in browse mode instead.
- **Selection state:** `capture-pane` returns plain text; inverse-video cell attributes are not available. Selection assertions rely on text position rather than cell attributes.
