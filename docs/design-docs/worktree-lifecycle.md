# Worktree Lifecycle

## Spawn (`zelligent spawn <branch> [agent-cmd]`)

1. **Resolve repo root** — even from inside a worktree, finds the main repo via `git rev-parse --git-common-dir`
2. **TTY guard (outside Zellij only)** — `zellij attach` and `zellij --new-session-with-layout` need a controlling terminal. If neither stdin nor stdout is a TTY, spawn refuses early with a friendly error before creating any worktree state. Set `ZELLIGENT_SKIP_TTY_CHECK=1` in test harnesses that use mock zellij stubs.
3. **Detect base branch** — picks the *current* branch (`git symbolic-ref --short HEAD`) so spawning from inside an existing worktree branches off the work in progress. Falls back to `origin/HEAD`'s target, then `main`, when HEAD is detached or unresolvable.
4. **Create worktree** — at `~/.zelligent/worktrees/<repo>/<branch>`
   - If branch exists: `git worktree add <path> <branch>`
   - If new branch: `git worktree add -b <branch> <path> <base>`
   - If worktree dir already exists: skips creation, just opens the tab
5. **Run setup hook** — if `.zelligent/setup.sh` exists and this is a new worktree, it runs before the agent command. Setup receives `$REPO_ROOT` and `$WORKTREE_PATH` as args.
6. **Generate layout** — builds a KDL layout file with agent pane (70%) and lazygit pane (30%). Inside Zellij and the existing-session-outside branches both write a *fragment* layout (panes at root); only the new-session branch writes a full session layout. Feeding `new-tab --layout` a session layout would graft the sidebar pane into the existing tab instead of opening a new one.
7. **Open tab** — behavior depends on context:
   - Inside Zellij: `zellij action new-tab --layout <fragment> --name <session-name>`
   - Outside Zellij, session exists: `ZELLIJ_SESSION_NAME=<repo> zellij action new-tab --layout <fragment> ...` then `zellij attach`
   - Outside Zellij, no session: `zellij --new-session-with-layout <session-layout> --session <repo>`

## Remove (`zelligent remove <branch>`)

1. **Find worktree** — looks up the worktree path for the branch via `git worktree list --porcelain`
2. **Validate ownership** — confirms the worktree is under `~/.zelligent/worktrees/<repo>/`
3. **Run teardown hook** — if `.zelligent/teardown.sh` exists, runs it first. Aborts if it fails.
4. **Remove worktree** — `git worktree remove <path>` (fails if uncommitted changes)
5. **Close the tab when running inside Zellij** — `zelligent` checks `$ZELLIJ`; if set, records the current tab via `zellij action current-tab-info`, switches to the worktree's tab via `zellij action go-to-tab-name <sanitized-branch>`, runs `zellij action close-tab`, then returns to the original tab. This stops the sidebar from showing the stale row as an orphaned "user tab". When invoked outside Zellij, prints a hint to close the tab manually instead.
6. **Note the branch is preserved** — `git worktree remove` does not delete the local branch.

## Nuke (`zelligent nuke`)

Destroys the entire Zellij session for the repo:
1. `zellij delete-session --force`
2. Kills any lingering server/client processes (via `ps` + `kill -9`)
3. Removes resurrection cache from Zellij's session_info directory
4. Cleans up stale socket files