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

A shell script for spawning AI coding agents in isolated git worktrees, each in their own Zellij tab.

## Installation

```bash
curl -fsSL https://raw.githubusercontent.com/pcomans/zelligent/main/install.sh | bash
```

Installs `zelligent` to `/usr/local/bin` (or `~/.local/bin` if that isn't writable).

## Usage

```bash
zelligent spawn <branch-name> [agent-command]
```

- `branch-name` — created from the default branch if it doesn't exist, reattached if it does
- `agent-command` — command to run in the main pane (default: `$SHELL`)

Examples:

```bash
zelligent spawn feature/my-feature          # opens a shell
zelligent spawn feature/my-feature claude   # opens Claude Code
```

Behaviour depends on context:

| Context | Result |
|---|---|
| Inside a Zellij session | Opens a new tab in the current session |
| Outside Zellij, repo session exists | Attaches to the repo session, opens a new tab |
| Outside Zellij, no repo session | Creates a new session named after the repo |

Each worktree opens as a tab named after the branch (`feature/my-feature` → tab `feature-my-feature`).

Worktrees are stored under `~/.zelligent/<repo-name>/<branch-name>`.

Each tab opens with the agent command on the left (70%) and lazygit on the right (30%).

## Removing a worktree

```bash
zelligent remove <branch-name>
```

Runs `.zelligent/teardown.sh` (if present), removes the worktree, and prints a reminder to close the tab. Fails with a clear error if the worktree has uncommitted changes. The local git branch is not deleted.

## Init

```bash
zelligent init
```

Creates `.zelligent/setup.sh` and `.zelligent/teardown.sh` in the current repo if they don't already exist.

## Per-repo hooks

Create `.zelligent/setup.sh` to run custom setup when a worktree is created (copy `.env`, install deps, etc.). The setup script runs **inside the new Zellij tab** as a preamble to the agent command, so you can see its progress. If the setup script fails (non-zero exit), the agent command will not start and the pane stays open so you can read the error.

```bash
#!/bin/bash
REPO_ROOT=$1
WORKTREE_PATH=$2

cp "$REPO_ROOT/.env" "$WORKTREE_PATH/"
cd "$WORKTREE_PATH" && npm install
```

The setup script only runs once when the worktree is first created. Reopening an existing worktree skips it.

Create `.zelligent/teardown.sh` to clean up when a worktree is removed (stop dev servers, remove copied files, etc.):

```bash
#!/bin/bash
REPO_ROOT=$1
WORKTREE_PATH=$2

rm -f "$WORKTREE_PATH/.env"
```

## Custom layout

Create `.zelligent/layout.kdl` to override the default Zellij layout. Use `{{cwd}}` and `{{agent_cmd}}` as placeholders.

> **Note:** Custom layouts bypass the automatic `setup.sh` preamble. If you need setup to run before the agent, wrap it in your template's command, e.g. `command="bash"` with `args "-c" "bash .zelligent/setup.sh /repo /worktree && exec {{agent_cmd}}"`.


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

## Navigating tabs

Each worktree opens as a tab in your current Zellij session. The keybindings below are Zellij's defaults — they may differ if you have a custom config. See the [Zellij docs](https://zellij.dev/documentation/) for reference.

| Action | Keybinding |
|---|---|
| Next tab | `Ctrl-t n` |
| Previous tab | `Ctrl-t p` |
| Rename tab | `Ctrl-t r` |
| Close tab | `Ctrl-t x` |
| Switch between panes | `Ctrl-p` + arrow keys |
| New pane | `Ctrl-p n` |
| Split pane right | `Ctrl-p d` |
| Split pane down | `Ctrl-p D` |

## Zellij setup

### Copy/paste (macOS)

Zellij requires an explicit copy command. Add this to your `~/.config/zellij/config.kdl`:

```kdl
copy_command "pbcopy"
```

## Zellij plugin

A WASM plugin that provides an interactive UI for managing worktrees, launched via a keybinding as a floating pane.

### Building

Requires Rust (via [rustup](https://rustup.rs)) with the `wasm32-wasip1` target:

```bash
rustup target add wasm32-wasip1
cd plugin && bash build.sh
```

This compiles the plugin and copies it to `~/.config/zellij/plugins/`.

> **Note:** If you also have Rust installed via Homebrew, its `cargo` may take precedence and fail with "can't find crate for core". Fix by ensuring rustup's toolchain is first on PATH:
> ```bash
> PATH="$HOME/.rustup/toolchains/stable-$(rustc -vV | grep host | cut -d' ' -f2)/bin:$PATH" bash build.sh
> ```

### Keybinding

Add to your `~/.config/zellij/config.kdl`:

```kdl
keybinds {
    shared_except "locked" {
        bind "Ctrl y" {
            LaunchOrFocusPlugin "file:~/.config/zellij/plugins/zelligent-plugin.wasm" {
                floating true
                move_to_focused_tab true
                agent_cmd "claude"
            }
        }
    }
}
```

### Controls

| Key | Action |
|---|---|
| `j/k` or arrows | Navigate list |
| `Enter` | Open selected worktree |
| `n` | Pick from existing git branches |
| `i` | Type a new branch name |
| `d` then `y` | Remove selected worktree |
| `r` | Refresh |
| `q` / `Esc` | Close |

## Requirements

- git
- [Zellij](https://zellij.dev)
- [lazygit](https://github.com/jesseduffield/lazygit)
- Rust with `wasm32-wasip1` target (for building the plugin)
