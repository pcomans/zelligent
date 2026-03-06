```
  ▄▄▄▄▄▄▄▄      ▄▄ ▄▄
 █▀▀▀▀▀██▀       ██ ██                      █▄
      ▄█▀        ██ ██ ▀▀    ▄▄       ▄    ▄██▄
    ▄█▀    ▄█▀█▄ ██ ██ ██ ▄████ ▄█▀█▄ ████▄ ██
  ▄█▀    ▄ ██▄█▀ ██ ██ ██ ██ ██ ██▄█▀ ██ ██ ██
 ████████▀▄▀█▄▄▄▄██▄██▄██▄▀████▄▀█▄▄▄▄██ ▀█▄██
                             ██
                           ▀▀▀
```

Zelligent runs AI coding agents in isolated git worktrees, each in its own [Zellij](https://zellij.dev) tab.

You give it a branch name and an agent command. It creates a worktree, opens a new tab with the agent on the left and [lazygit](https://github.com/jesseduffield/lazygit) on the right. When you're done, it cleans up the worktree.

Use the CLI to spawn worktrees, or press **`Ctrl-y`** inside Zellij to manage them from an interactive plugin UI.

## Quick start

Install with Homebrew (pulls in Zellij and lazygit automatically):

```bash
brew install pcomans/zelligent/zelligent
```

Run the setup wizard:

```bash
zelligent doctor
```

This installs the Zellij plugin, adds the `Ctrl-y` keybinding to your Zellij config, and syncs the bundled Claude skill to `~/.claude/skills/zelligent-spawn-claude/SKILL.md` (Homebrew installs use a symlink to keep it updated across upgrades). It also sets up clipboard support on macOS. Safe to run more than once.

Start a session:

```bash
cd my-project
zelligent
```

This opens a Zellij session named after your repo. If the session already exists, it reattaches to it.

Spawn an agent in a new worktree:

```bash
zelligent spawn feature/my-feature claude
```

This creates a git worktree branched from your default branch, opens a new Zellij tab, and runs `claude` (Claude Code) in it. The tab is split 70/30 between the agent and lazygit.

On first launch, Zellij will ask you to grant the plugin permissions. Select `y` — the plugin needs these to manage worktrees and tabs.

## How it works

Zelligent has two parts: a shell script that manages git worktrees and Zellij sessions, and a Rust plugin (compiled to WASM) that provides an interactive UI inside Zellij.

### The CLI

The `zelligent` command handles worktree creation, layout generation, and session management.

When you run `zelligent spawn feature/auth claude`:

1. It creates a git worktree at `~/.zelligent/worktrees/<repo>/<branch>/`, branched from your default branch (usually `main`)
2. It generates a Zellij layout file (KDL format) with two panes: agent left, lazygit right
3. If you're inside Zellij, it opens a new tab. If you're outside, it creates or attaches to the repo's session

If the branch already exists, it reuses the existing worktree instead of creating a new one.

### The plugin

Press `Ctrl-y` inside Zellij to open the plugin as a floating pane. It lists your active worktrees and lets you create, open, or remove them without leaving the terminal.

| Key | Action |
|---|---|
| `j/k` or arrows | Navigate the list |
| `Enter` | Open the selected worktree |
| `n` | Pick from existing git branches |
| `i` | Type a new branch name |
| `d` then `y` | Remove the selected worktree |
| `r` | Refresh |
| `q` / `Esc` | Close |

When you remove a worktree through the plugin, it also closes the corresponding tab.

## Commands

```
zelligent                                   # launch/attach session for current repo
zelligent spawn <branch-name> [agent-cmd]   # create worktree and open agent tab
zelligent remove <branch-name>              # remove a worktree
zelligent init                              # create .zelligent/ hook stubs
zelligent doctor                            # check and fix setup
```

`zelligent spawn` adapts to context:

| Context | Result |
|---|---|
| Inside a Zellij session | Opens a new tab in the current session |
| Outside Zellij, session exists | Attaches to the session, opens a new tab |
| Outside Zellij, no session | Creates a new session |

`zelligent remove` refuses to delete worktrees with uncommitted changes. The local branch is kept.

## Per-repo hooks

Run `zelligent init` to create hook stubs, or create them manually.

### Setup hook

`.zelligent/setup.sh` runs inside the new tab when a worktree is first created. Use it to copy config files, install dependencies, or anything else the agent needs before it starts.

```bash
#!/bin/bash
REPO_ROOT=$1
WORKTREE_PATH=$2

cp "$REPO_ROOT/.env" "$WORKTREE_PATH/"
cd "$WORKTREE_PATH" && npm install
```

If the setup script fails, the agent command won't start and the pane stays open so you can read the error. The script only runs on first creation — reopening an existing worktree skips it.

### Teardown hook

`.zelligent/teardown.sh` runs when a worktree is removed:

```bash
#!/bin/bash
REPO_ROOT=$1
WORKTREE_PATH=$2

rm -f "$WORKTREE_PATH/.env"
```

## Custom layout

Create `.zelligent/layout.kdl` to override the default tab layout. Use `{{cwd}}` and `{{agent_cmd}}` as placeholders:

```kdl
layout {
    pane size=1 borderless=true {
        plugin location="zellij:tab-bar"
    }
    pane split_direction="vertical" {
        pane command="{{agent_cmd}}" cwd="{{cwd}}" size="70%"
        pane command="lazygit" cwd="{{cwd}}" size="30%"
    }
    pane size=1 borderless=true {
        plugin location="zellij:status-bar"
    }
}
```

Custom layouts bypass the automatic `setup.sh` preamble. If you need setup to run before the agent, wrap it in your command, e.g. `args "-c" "bash .zelligent/setup.sh /repo /worktree && exec {{agent_cmd}}"`.

## Agent status notifications

When using Claude Code (or another agent with hook support), zelligent tracks agent status and shows it in the plugin UI:

| Indicator | Meaning |
|---|---|
| `●` (green) | Agent is working |
| `●` (yellow) | Agent needs input (permission prompt, etc.) |
| `✓` (green) | Agent finished |

On macOS, you also get desktop notifications with a sound when an agent needs input or finishes. This uses `osascript` and is currently macOS-only.

`zelligent doctor` sets up the required hooks automatically when Claude Code is installed.

## Navigating tabs

These are Zellij's default keybindings:

| Action | Keybinding |
|---|---|
| Next tab | `Ctrl-t n` |
| Previous tab | `Ctrl-t p` |
| Rename tab | `Ctrl-t r` |
| Close tab | `Ctrl-t x` |
| Switch panes | `Ctrl-p` + arrow keys |

## Session resurrection

Zellij automatically saves your session layout periodically and restores it when you reattach. By default it snapshots every 60 seconds, which means tabs you closed in the last minute can reappear after a restart.

`zelligent doctor` sets `serialization_interval 5` in your Zellij config so snapshots are taken every 5 seconds. This keeps the saved state close to what you actually see, so closed tabs stay closed after resurrection.

## Installing from source

```bash
git clone https://github.com/pcomans/zelligent.git
git clone https://github.com/zellij-org/zellij.git
cd zelligent
ZELLIGENT_ZELLIJ_SRC=../zellij \
  PATH="$HOME/.rustup/toolchains/stable-$(rustc -vV | grep host | cut -d' ' -f2)/bin:$PATH" \
  bash dev-install.sh
```

Requires [Rust via rustup](https://rustup.rs) with the `wasm32-wasip1` target (`rustup target add wasm32-wasip1`).

`ZELLIGENT_ZELLIJ_SRC` must point to a local [Zellij](https://github.com/zellij-org/zellij) checkout (main branch). The dev-install script builds Zellij from source and installs it alongside zelligent.
