---
name: zelligent-spawn-claude
description: Use zelligent CLI to spawn Claude subagents into isolated git worktrees, each opened in a new Zellij tab. Trigger when asked to run multiple parallel agents or split tasks across separate branches/tabs.
allowed-tools: Bash
---

Use the CLI flow only. Do not use the interactive plugin UI.

## Workflow

1. Work from inside the target git repository.
2. Start or attach to the repo session:

```bash
zelligent
```

3. For each subagent task, choose a branch name and spawn a new tab:

```bash
zelligent spawn <branch-name> claude
```

4. Repeat step 3 for additional subagents, one branch per task.

## Conventions

- Use descriptive branch names, for example `agent/fix-auth` or `agent/write-tests`.
- Expect tab names to sanitize `/` into `-`:
  - `agent/fix-auth` becomes tab `agent-fix-auth`.
- Keep a 1:1 mapping between branch and task.

## Cleanup

When a subagent branch is finished, remove its worktree:

```bash
zelligent remove <branch-name>
```

This removes the worktree and keeps the local branch.
