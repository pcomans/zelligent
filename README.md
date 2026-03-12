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

Every zelligent-managed session has a persistent sidebar on the left. The sidebar is a WASM plugin that lists worktrees, spawns new ones, switches tabs, and removes old worktrees without leaving Zellij.

## Quick start

Install with Homebrew:

```bash
brew install pcomans/zelligent/zelligent
```

Run setup:

```bash
zelligent doctor
```

`doctor` installs plugin permissions, sets a few Zellij config defaults, and creates `~/.zelligent/layout.kdl` if it does not already exist. It does not install a `Ctrl-y` keybinding anymore because the sidebar is persistent.

Start a repo session:

```bash
cd my-project
zelligent
```

This creates or attaches to a Zellij session named after the repo. On a new session, zelligent uses the active layout template to render the first tab and configures the session so future manually created tabs inherit the zelligent sidebar frame.

Spawn a worktree tab:

```bash
zelligent spawn feature/my-feature claude
```

This creates a git worktree, opens a new tab, and runs the requested agent command next to `lazygit`.

## Commands

```bash
zelligent                                   # launch/attach session for current repo
zelligent spawn <branch-name> [agent-cmd]   # create worktree and open agent tab
zelligent remove <branch-name>              # remove a worktree
zelligent init                              # create .zelligent/ hook stubs
zelligent nuke                              # delete the session so next launch starts fresh
zelligent doctor                            # check and fix setup
```

`zelligent spawn` adapts to context:

| Context | Result |
|---|---|
| Inside a Zellij session | Opens a new tab in the current session |
| Outside Zellij, repo session exists | Opens a new tab in that session and attaches |
| Outside Zellij, no repo session | Creates a new repo session |

## Sidebar behavior

The sidebar is always present in zelligent-managed sessions.

Browse mode keys:

| Key | Action |
|---|---|
| `j/k` or arrows | Move selection |
| `Enter` | Open the selected worktree tab or spawn it if needed |
| `n` | Pick an existing git branch |
| `i` | Type a new branch name |
| `d` then `y` | Remove the selected worktree |
| `r` | Refresh |

`q` and `Esc` do nothing in normal sidebar mode. Closing the sidebar would violate the product model.

## Hooks

Run `zelligent init` to create hook stubs, or create them manually.

### Setup hook

`.zelligent/setup.sh` runs when a worktree is first created:

```bash
#!/bin/bash
REPO_ROOT=$1
WORKTREE_PATH=$2

cp "$REPO_ROOT/.env" "$WORKTREE_PATH/"
cd "$WORKTREE_PATH" && npm install
```

If setup fails, the agent pane stays open so the failure is visible.

### Teardown hook

`.zelligent/teardown.sh` runs before a worktree is removed:

```bash
#!/bin/bash
REPO_ROOT=$1
WORKTREE_PATH=$2

rm -f "$WORKTREE_PATH/.env"
```

## Layouts

Zelligent always renders from a layout template file. Runtime precedence is:

1. repo-local `.zelligent/layout.kdl`
2. user-level `~/.zelligent/layout.kdl`
3. hard error if neither exists

The shipped default template is installed to `~/.zelligent/layout.kdl` by `zelligent doctor` if that file is missing.

### Layout contract

`.zelligent/layout.kdl` is a full Zellij layout file. It is not a body fragment.

Supported placeholders:

| Placeholder | Requirement | Meaning |
|---|---|---|
| `{{zelligent_sidebar}}` | Required exactly once | Expands to the persistent left sidebar pane |
| `{{cwd}}` | Optional | Repo root for `zelligent`, worktree path for `spawn` |
| `{{agent_cmd}}` | Optional | Shell command fragment intended for `bash -c` |

Unknown placeholders are left untouched.

`{{agent_cmd}}` is not just the raw agent binary. On freshly created worktrees it includes the setup-hook preamble before the final `exec ...`. If you want that behavior, use it inside `args "-c" "{{agent_cmd}}"`.

Default shipped layout:

```kdl
layout {
    pane split_direction="Vertical" {
        {{zelligent_sidebar}}
        pane {
            pane command="bash" cwd="{{cwd}}" size="70%" {
                args "-c" "{{agent_cmd}}"
            }
            pane command="lazygit" cwd="{{cwd}}" size="30%"
        }
    }
    pane size=1 borderless=true {
        plugin location="zellij:status-bar"
    }
}
```

You can replace the inner body completely as long as the sidebar placeholder appears exactly once.

Example custom layout:

```kdl
layout {
    pane split_direction="Vertical" {
        {{zelligent_sidebar}}
        pane split_direction="Horizontal" {
            pane command="bash" cwd="{{cwd}}" size="65%" {
                args "-c" "{{agent_cmd}}"
            }
            pane command="npm" cwd="{{cwd}}" {
                args "run" "dev"
            }
        }
    }
}
```

### Manual tabs

When `zelligent` creates a new session, it also configures that session so manually created tabs inherit the zelligent sidebar frame.

That sidebar inheritance is guaranteed. Matching the full custom inner body for manual tabs is best-effort and depends on what Zellij exposes through `default_tab_template`.

## Agent status notifications

When using Claude Code or another compatible agent, zelligent shows agent status in the sidebar:

| Indicator | Meaning |
|---|---|
| `●` (green) | Agent is working |
| `●` (yellow) | Agent needs input |
| `✓` (green) | Agent finished |

On macOS, `zelligent doctor` also sets up the Claude Code plugin hooks used for desktop notifications.

## Session resurrection

Zellij periodically snapshots the session layout and restores it on reattach. `zelligent doctor` sets `serialization_interval 5` so the saved state stays close to what you actually see.

## Installing from source

```bash
git clone https://github.com/pcomans/zelligent.git
cd zelligent
PATH="$HOME/.rustup/toolchains/stable-$(rustc -vV | grep host | cut -d' ' -f2)/bin:$PATH" bash dev-install.sh
```

Requires [Rust via rustup](https://rustup.rs) with the `wasm32-wasip1` target:

```bash
rustup target add wasm32-wasip1
```
