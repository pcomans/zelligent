# Worktree Lifecycle

## Spawn (`zelligent spawn <branch> [agent-cmd]`)

1. **Resolve repo root** — even from inside a worktree, finds the main repo via `git rev-parse --git-common-dir`
2. **Detect base branch** — uses `git symbolic-ref refs/remotes/origin/HEAD`, falls back to `main`
3. **Create worktree** — at `~/.zelligent/worktrees/<repo>/<branch>`
   - If branch exists: `git worktree add <path> <branch>`
   - If new branch: `git worktree add -b <branch> <path> <base>`
   - If worktree dir already exists: skips creation, just opens the tab
4. **Run setup hook** — if `.zelligent/setup.sh` exists and this is a new worktree, it runs before the agent command. Setup receives `$REPO_ROOT` and `$WORKTREE_PATH` as args.
5. **Generate layout** — builds a KDL layout file with agent pane (70%) and lazygit pane (30%)
6. **Open tab** — behavior depends on context:
   - Inside Zellij: `zellij action new-tab --layout <file> --name <session-name>`
   - Outside Zellij, session exists: `ZELLIJ_SESSION_NAME=<repo> zellij action new-tab ...` then `zellij attach`
   - Outside Zellij, no session: `zellij --new-session-with-layout <file> --session <repo>`

## Remove (`zelligent remove <branch>`)

1. **Find worktree** — looks up the worktree path for the branch via `git worktree list --porcelain`
2. **Validate ownership** — confirms the worktree is under `~/.zelligent/worktrees/<repo>/`
3. **Run teardown hook** — if `.zelligent/teardown.sh` exists, runs it first. Aborts if it fails.
4. **Remove worktree** — `git worktree remove <path>` (fails if uncommitted changes)
5. **Print instructions** — reminds user to close the tab manually and notes the branch is preserved

## Nuke (`zelligent nuke`)

Destroys the entire Zellij session for the repo:
1. `zellij delete-session --force`
2. Kills any lingering server/client processes (via `ps` + `kill -9`)
3. Removes resurrection cache from Zellij's session_info directory
4. Cleans up stale socket files