#!/bin/bash
# commit: __COMMIT_SHA__

# Exit immediately if a command exits with a non-zero status
set -e

ZELLIJ_CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}/zellij"

# --- Commands that do not require a git repo ---

usage() {
  echo "Usage: zelligent                              Launch/attach Zellij session for current repo"
  echo "       zelligent spawn <branch> [agent-cmd]   Create worktree and open agent tab"
  echo "       zelligent remove <branch>              Remove a worktree"
  echo "       zelligent init                         Create .zelligent/ hook stubs"
  echo "       zelligent doctor                       Check and fix zelligent setup"
  echo "       zelligent --version                    Print version"
  echo "       zelligent --help                       Show this help"
}

if [ "$1" = "--version" ]; then
  echo "zelligent __COMMIT_SHA__"
  exit 0
fi

if [ "$1" = "--help" ] || [ "$1" = "help" ]; then
  usage
  exit 0
fi

if [ "$1" = "doctor" ]; then
  ERRORS=0

  # 1. Check zellij is installed
  if command -v zellij &>/dev/null; then
    echo "  zellij: ok"
  else
    echo "  zellij: not found. Install with: brew install zellij"
    ERRORS=1
  fi

  # 2. Install the Zellij plugin
  # Find the plugin source
  if [ -n "$ZELLIGENT_PLUGIN_SRC" ]; then
    PLUGIN_SRC="$ZELLIGENT_PLUGIN_SRC"
  else
    ZELLIGENT_BIN=$(command -v zelligent 2>/dev/null || echo "$0")
    ZELLIGENT_PREFIX=$(dirname "$(dirname "$ZELLIGENT_BIN")")
    PLUGIN_SRC="$ZELLIGENT_PREFIX/share/zelligent/zelligent-plugin.wasm"
  fi

  PLUGIN_DEST="$ZELLIJ_CONFIG_HOME/plugins/zelligent-plugin.wasm"

  if [ ! -f "$PLUGIN_SRC" ]; then
    echo "  plugin: source not found at $PLUGIN_SRC"
    echo "          If installed from source, run: cd plugin && bash build.sh"
    ERRORS=1
  elif [ -f "$PLUGIN_DEST" ] && cmp -s "$PLUGIN_SRC" "$PLUGIN_DEST"; then
    echo "  plugin: ok"
  else
    mkdir -p "$(dirname "$PLUGIN_DEST")"
    cp "$PLUGIN_SRC" "$PLUGIN_DEST"
    echo "  plugin: installed to $PLUGIN_DEST"
  fi

  # 3. Patch Zellij config
  CONFIG="$ZELLIJ_CONFIG_HOME/config.kdl"
  mkdir -p "$(dirname "$CONFIG")"
  touch "$CONFIG"

  if grep -qF 'zelligent-plugin.wasm' "$CONFIG"; then
    echo "  keybinding: ok"
  else
    cat >> "$CONFIG" <<KDL

keybinds {
    shared_except "locked" {
        bind "Ctrl y" {
            LaunchOrFocusPlugin "file:$PLUGIN_DEST" {
                floating true
                move_to_focused_tab true
                agent_cmd "claude"
            }
        }
    }
}
KDL
    echo "  keybinding: added Ctrl-y to $CONFIG"
  fi

  if [ "$(uname)" = "Darwin" ]; then
    if grep -qF 'copy_command' "$CONFIG"; then
      echo "  copy_command: ok"
    else
      echo 'copy_command "pbcopy"' >> "$CONFIG"
      echo "  copy_command: added pbcopy to $CONFIG"
    fi
  fi

  if [ "$ERRORS" -ne 0 ]; then
    echo ""
    echo "Some checks failed. Fix the issues above and run 'zelligent doctor' again."
    exit 1
  fi

  echo ""
  echo "All good! Restart Zellij to apply any config changes."
  exit 0
fi

# --- Everything below requires a git repo ---

# Require git repo — resolve to the main repo root even when run from a worktree.
if ! GIT_COMMON_DIR=$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null); then
  echo "Error: not inside a git repository." >&2
  exit 1
fi

REPO_ROOT="${GIT_COMMON_DIR%/.git}"
REPO_NAME=$(basename "$REPO_ROOT")
WORKTREES_DIR="$HOME/.zelligent/worktrees/$REPO_NAME"

# No args: launch or attach to Zellij session for this repo
if [ -z "$1" ]; then
  # Check plugin is installed
  if [ ! -f "$ZELLIJ_CONFIG_HOME/plugins/zelligent-plugin.wasm" ]; then
    echo "Plugin not installed. Run 'zelligent doctor' to set up." >&2
    exit 1
  fi

  if [ -n "$ZELLIJ" ]; then
    echo "Already inside a Zellij session. Use 'zelligent spawn <branch>' to open a worktree tab."
    exit 0
  fi

  if zellij list-sessions --no-formatting --short 2>/dev/null | grep -qxF "$REPO_NAME"; then
    echo "Attaching to session '$REPO_NAME'..."
    exec zellij attach "$REPO_NAME"
  else
    echo "Creating Zellij session '$REPO_NAME'..."
    exec zellij --session "$REPO_NAME"
  fi
fi

# --- Query subcommands (no zellij/lazygit needed) ---

if [ "$1" = "show-repo" ]; then
  echo "repo_root=$REPO_ROOT"
  echo "repo_name=$REPO_NAME"
  exit 0
fi

if [ "$1" = "list-worktrees" ]; then
  SPAWN_PREFIX="$WORKTREES_DIR/"
  git -C "$REPO_ROOT" worktree list --porcelain | while IFS= read -r line; do
    case "$line" in
      "worktree "*)
        current_path="${line#worktree }"
        ;;
      "branch "*)
        if [[ "$current_path" == "$SPAWN_PREFIX"* ]]; then
          echo "${line#branch refs/heads/}"
        fi
        ;;
    esac
  done
  exit 0
fi

if [ "$1" = "list-branches" ]; then
  git -C "$REPO_ROOT" branch --format='%(refname:short)'
  exit 0
fi

# Handle init subcommand
if [ "$1" = "init" ]; then
  mkdir -p "$REPO_ROOT/.zelligent"
  for script in setup teardown; do
    SCRIPT_PATH="$REPO_ROOT/.zelligent/$script.sh"
    if [ -f "$SCRIPT_PATH" ]; then
      echo "⚠️  .zelligent/$script.sh already exists, skipping"
    else
      cat > "$SCRIPT_PATH" <<'EOF'
#!/bin/bash
REPO_ROOT=$1
WORKTREE_PATH=$2
EOF
      chmod +x "$SCRIPT_PATH"
      echo "✅ Created .zelligent/$script.sh"
    fi
  done
  exit 0
fi

# Handle remove subcommand
if [ "$1" = "remove" ]; then
  if [ -z "$2" ]; then
    echo "Usage: zelligent remove <branch-name>"
    exit 1
  fi
  BRANCH_NAME=$2
  SESSION_NAME="${BRANCH_NAME//\//-}"
  WORKTREE_PATH="$WORKTREES_DIR/$BRANCH_NAME"
  if [ ! -d "$WORKTREE_PATH" ]; then
    echo "Error: worktree '$WORKTREE_PATH' does not exist."
    exit 1
  fi
  if [ -f "$REPO_ROOT/.zelligent/teardown.sh" ]; then
    echo "⚙️  Running .zelligent/teardown.sh..."
    if ! bash "$REPO_ROOT/.zelligent/teardown.sh" "$REPO_ROOT" "$WORKTREE_PATH"; then
      echo "Error: teardown.sh failed. Worktree was NOT removed."
      exit 1
    fi
  fi
  if ! git worktree remove "$WORKTREE_PATH" 2>/dev/null; then
    echo "Error: could not remove worktree. It may have uncommitted changes."
    echo "Commit or stash your changes, then try again."
    exit 1
  fi
  echo "✅ Removed worktree for '$BRANCH_NAME'"
  echo "ℹ️  Close the '$SESSION_NAME' tab manually if still open."
  echo "ℹ️  Local branch '$BRANCH_NAME' was not deleted."
  exit 0
fi

# Handle spawn subcommand
if [ "$1" = "spawn" ]; then
  if [ -z "$2" ]; then
    echo "Usage: zelligent spawn <branch-name> [agent-command]"
    exit 1
  fi
  BRANCH_NAME=$2
  AGENT_CMD=${3:-"$SHELL"}
else
  echo "Unknown command: $1"
  usage
  exit 1
fi

# Check zellij is available before creating any worktrees
if ! command -v zellij &>/dev/null; then
  echo "Error: zellij not found. Run 'zelligent doctor' to set up." >&2
  exit 1
fi

SESSION_NAME="${BRANCH_NAME//\//-}"

# Escape backslashes and double quotes for KDL string embedding
AGENT_CMD_KDL="${AGENT_CMD//\\/\\\\}"
AGENT_CMD_KDL="${AGENT_CMD_KDL//\"/\\\"}"

# Detect default base branch
if BASE_REF=$(git symbolic-ref refs/remotes/origin/HEAD 2>/dev/null); then
  BASE_BRANCH="${BASE_REF#refs/remotes/origin/}"
else
  BASE_BRANCH="main"
fi

# Define the new centralized worktree path
WORKTREE_PATH="$WORKTREES_DIR/$BRANCH_NAME"

NEW_WORKTREE=false

# Check if the worktree directory already exists
if [ -d "$WORKTREE_PATH" ]; then
  echo "⚠️  Worktree already exists, opening new tab..."
else
  NEW_WORKTREE=true
  mkdir -p "$WORKTREES_DIR"
  echo "🚀 Creating workspace for '$BRANCH_NAME' at $WORKTREE_PATH..."

  # Handle existing vs new branches
  if git show-ref --verify --quiet "refs/heads/$BRANCH_NAME"; then
    echo "🌿 Branch '$BRANCH_NAME' exists. Attaching worktree..."
    git worktree add "$WORKTREE_PATH" "$BRANCH_NAME"
  else
    echo "🌱 Creating new branch '$BRANCH_NAME' from '$BASE_BRANCH'..."
    git worktree add -b "$BRANCH_NAME" "$WORKTREE_PATH" "$BASE_BRANCH"
  fi

fi

# Use repo-level layout if present, otherwise use built-in default
if [ -f "$REPO_ROOT/.zelligent/layout.kdl" ]; then
  LAYOUT_TEMPLATE="$REPO_ROOT/.zelligent/layout.kdl"
else
  LAYOUT_TEMPLATE=""
fi

# Generate temp layout files
mkdir -p "$HOME/.zelligent/tmp"
LAYOUT=$(mktemp "$HOME/.zelligent/tmp/layout-XXXXXX")
trap 'rm -f "$LAYOUT"' EXIT

# Build the agent pane command, prepending setup.sh for new worktrees
SETUP_SCRIPT="$REPO_ROOT/.zelligent/setup.sh"
if [ "$NEW_WORKTREE" = true ] && [ -f "$SETUP_SCRIPT" ]; then
  AGENT_PANE="pane command=\"bash\" cwd=\"$WORKTREE_PATH\" size=\"70%\" {
            args \"-c\" \"bash \\\"\$1\\\" \\\"\$2\\\" \\\"\$3\\\" || { echo 'Setup failed (exit '\$?'). Press Enter to close.'; read; exit 1; }; exec $AGENT_CMD_KDL\" \"--\" \"$SETUP_SCRIPT\" \"$REPO_ROOT\" \"$WORKTREE_PATH\"
        }"
else
  AGENT_PANE="pane command=\"bash\" cwd=\"$WORKTREE_PATH\" size=\"70%\" {
            args \"-c\" \"exec $AGENT_CMD_KDL\"
        }"
fi

# Pane content shared by both layouts
pane_content() {
  cat <<EOF
    pane size=1 borderless=true {
        plugin location="zellij:tab-bar"
    }
    pane split_direction="vertical" {
        $AGENT_PANE
        pane command="lazygit" cwd="$WORKTREE_PATH" size="30%"
    }
    pane size=1 borderless=true {
        plugin location="zellij:status-bar"
    }
EOF
}

if [ -n "$LAYOUT_TEMPLATE" ] && [ -n "$ZELLIJ" ]; then
  # Inside Zellij with custom template: substitute vars, use as-is for new-tab
  sed -e "s|{{cwd}}|$WORKTREE_PATH|g" -e "s|{{agent_cmd}}|$AGENT_CMD|g" "$LAYOUT_TEMPLATE" > "$LAYOUT"
elif [ -n "$LAYOUT_TEMPLATE" ]; then
  # Outside Zellij with custom template: strip outer layout{} and wrap in a named tab
  INNER=$(sed -e "s|{{cwd}}|$WORKTREE_PATH|g" -e "s|{{agent_cmd}}|$AGENT_CMD|g" "$LAYOUT_TEMPLATE" | sed '1d;$d')
  { echo "layout {"; echo "    tab name=\"$SESSION_NAME\" {"; echo "$INNER"; echo "    }"; echo "}"; } > "$LAYOUT"
elif [ -n "$ZELLIJ" ]; then
  # Tab layout: no tab wrapper (new-tab provides the tab context)
  { echo "layout {"; pane_content; echo "}"; } > "$LAYOUT"
else
  # Session layout: wrap in a named tab
  { echo "layout {"; echo "    tab name=\"$SESSION_NAME\" {"; pane_content; echo "    }"; echo "}"; } > "$LAYOUT"
fi

# Inside Zellij: open as a new tab in the current session.
# Outside Zellij: create or attach to a repo-named session, open worktree as a tab.
if [ -n "$ZELLIJ" ]; then
  echo "🪟 Opening tab '$SESSION_NAME'..."
  zellij action new-tab --layout "$LAYOUT" --name "$SESSION_NAME"
elif zellij list-sessions --no-formatting --short 2>/dev/null | grep -qxF "$REPO_NAME"; then
  echo "🪟 Attaching to session '$REPO_NAME', opening tab '$SESSION_NAME'..."
  ZELLIJ_SESSION_NAME="$REPO_NAME" zellij action new-tab --layout "$LAYOUT" --name "$SESSION_NAME"
  zellij attach "$REPO_NAME"
else
  echo "🪟 Creating Zellij session '$REPO_NAME'..."
  zellij --new-session-with-layout "$LAYOUT" --session "$REPO_NAME"
fi
