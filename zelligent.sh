#!/bin/bash
# commit: __COMMIT_SHA__

# Exit immediately if a command exits with a non-zero status
set -e

ZELLIJ_CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}/zellij"
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

# Wrapper with 3s timeout to avoid hanging on stale Zellij sockets
zellij_list_sessions() {
  local output
  if output=$(perl -e 'alarm 3; exec @ARGV' -- zellij list-sessions --no-formatting --short 2>/dev/null); then
    printf '%s\n' "$output"
  else
    local status=$?
    # 142 = SIGALRM (128 + 14) — the timeout fired
    if [ "$status" -eq 142 ]; then
      local socket_dir
      for d in "$TMPDIR"/zellij-*/; do
        [ -d "$d" ] && socket_dir="$d" && break
      done
      echo "Warning: 'zellij list-sessions' timed out — likely stale session sockets." >&2
      if [ -n "$socket_dir" ]; then
        echo "Clean up with:  rm -rf ${socket_dir}" >&2
      fi
    fi
    return 1
  fi
}

escape_kdl() {
  local value="$1"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  printf '%s' "$value"
}

escape_shell_single() {
  printf "%s" "$1" | sed "s/'/'\"'\"'/g"
}

resolve_plugin_path() {
  if [ -n "$ZELLIGENT_PLUGIN_SRC" ]; then
    [ -f "$ZELLIGENT_PLUGIN_SRC" ] || return 1
    printf '%s\n' "$ZELLIGENT_PLUGIN_SRC"
    return 0
  fi

  local zelligent_bin zelligent_prefix homebrew_plugin dev_plugin
  zelligent_bin=$(command -v zelligent 2>/dev/null || true)
  if [ -n "$zelligent_bin" ]; then
    zelligent_prefix=$(dirname "$(dirname "$zelligent_bin")")
    homebrew_plugin="$zelligent_prefix/share/zelligent/zelligent-plugin.wasm"
    if [ -f "$homebrew_plugin" ]; then
      printf '%s\n' "$homebrew_plugin"
      return 0
    fi
  fi

  dev_plugin="$HOME/.local/share/zelligent/zelligent-plugin.wasm"
  if [ -f "$dev_plugin" ]; then
    printf '%s\n' "$dev_plugin"
    return 0
  fi

  return 1
}

resolve_default_layout_path() {
  if [ -n "$ZELLIGENT_DEFAULT_LAYOUT_SRC" ]; then
    [ -f "$ZELLIGENT_DEFAULT_LAYOUT_SRC" ] || return 1
    printf '%s\n' "$ZELLIGENT_DEFAULT_LAYOUT_SRC"
    return 0
  fi

  local zelligent_bin zelligent_prefix homebrew_layout dev_layout source_layout
  zelligent_bin=$(command -v zelligent 2>/dev/null || true)
  if [ -n "$zelligent_bin" ]; then
    zelligent_prefix=$(dirname "$(dirname "$zelligent_bin")")
    homebrew_layout="$zelligent_prefix/share/zelligent/default-layout.kdl"
    if [ -f "$homebrew_layout" ]; then
      printf '%s\n' "$homebrew_layout"
      return 0
    fi
  fi

  dev_layout="$HOME/.local/share/zelligent/default-layout.kdl"
  if [ -f "$dev_layout" ]; then
    printf '%s\n' "$dev_layout"
    return 0
  fi

  source_layout="$SCRIPT_DIR/default-layout.kdl"
  if [ -f "$source_layout" ]; then
    printf '%s\n' "$source_layout"
    return 0
  fi

  return 1
}

count_placeholder_in_file() {
  local file="$1" needle="$2"
  grep -oF -- "$needle" "$file" | wc -l | tr -d ' '
}

validate_layout_template() {
  local file="$1" subcommand="$2" count
  count=$(count_placeholder_in_file "$file" "{{zelligent_sidebar}}")
  if [ "$count" -ne 1 ]; then
    echo "$subcommand: layout '$file' must contain {{zelligent_sidebar}} exactly once." >&2
    return 1
  fi
}

resolve_layout_template_path() {
  if [ -f "$REPO_ROOT/.zelligent/layout.kdl" ]; then
    printf '%s\n' "$REPO_ROOT/.zelligent/layout.kdl"
    return 0
  fi

  local user_layout="$HOME/.zelligent/layout.kdl"
  if [ -f "$user_layout" ]; then
    printf '%s\n' "$user_layout"
    return 0
  fi

  return 1
}

extract_layout_body() {
  perl -0pe 's/\A\s*layout\s*\{//s; s/\}\s*\z//s' "$1"
}

render_layout_template() {
  local source="$1" output="$2" cwd_value="$3" agent_cmd_value="$4" sidebar_value="$5"
  CWD_VALUE="$cwd_value" AGENT_CMD_VALUE="$agent_cmd_value" SIDEBAR_VALUE="$sidebar_value" \
    perl -0pe '
      s/\{\{zelligent_sidebar\}\}/$ENV{SIDEBAR_VALUE}/g;
      s/\{\{cwd\}\}/$ENV{CWD_VALUE}/g;
      s/\{\{agent_cmd\}\}/$ENV{AGENT_CMD_VALUE}/g;
    ' "$source" > "$output"
}

build_agent_command_fragment() {
  local session_name="$1" agent_cmd="$2" new_worktree="$3" setup_script="$4" repo_root="$5" worktree_path="$6"
  local session_name_sh setup_script_sh repo_root_sh worktree_path_sh
  session_name_sh=$(escape_shell_single "$session_name")

  if [ "$new_worktree" = true ] && [ -f "$setup_script" ]; then
    setup_script_sh=$(escape_shell_single "$setup_script")
    repo_root_sh=$(escape_shell_single "$repo_root")
    worktree_path_sh=$(escape_shell_single "$worktree_path")
    printf "export ZELLIGENT_TAB_NAME='%s'; bash '%s' '%s' '%s' || { echo 'Setup failed (exit '\$?'). Press Enter to close.'; read; exit 1; }; exec %s" \
      "$session_name_sh" "$setup_script_sh" "$repo_root_sh" "$worktree_path_sh" "$agent_cmd"
  else
    printf "export ZELLIGENT_TAB_NAME='%s'; exec %s" "$session_name_sh" "$agent_cmd"
  fi
}

build_sidebar_pane() {
  local plugin_path="$1" zelligent_path="$2" agent_cmd="$3"
  local plugin_path_kdl zelligent_path_kdl agent_cmd_kdl
  plugin_path_kdl=$(escape_kdl "$plugin_path")
  zelligent_path_kdl=$(escape_kdl "$zelligent_path")
  agent_cmd_kdl=$(escape_kdl "$agent_cmd")
  cat <<EOF
        pane size="24%" {
            plugin location="file:$plugin_path_kdl" {
                zelligent_path "$zelligent_path_kdl"
                agent_cmd "$agent_cmd_kdl"
            }
        }
EOF
}

write_session_layout() {
  local output="$1" tab_name="$2" rendered_body_layout="$3" sidebar_pane="$4"
  local body
  body=$(extract_layout_body "$rendered_body_layout")
  cat > "$output" <<EOF
layout {
    default_tab_template {
        pane split_direction="Vertical" {
$sidebar_pane
            children
        }
    }
    tab name="$tab_name" {
$body
    }
}
EOF
}

# --- Commands that do not require a git repo ---

usage() {
  echo "Usage: zelligent                              Launch/attach Zellij session for current repo"
  echo "       zelligent spawn <branch> [agent-cmd]   Create worktree and open agent tab"
  echo "       zelligent remove <branch>              Remove a worktree"
  echo "       zelligent init                         Create .zelligent/ hook stubs"
  echo "       zelligent nuke                         Delete session (start fresh)"
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
  ZELLIGENT_BIN=$(command -v zelligent 2>/dev/null || echo "$0")
  ZELLIGENT_PREFIX=$(dirname "$(dirname "$ZELLIGENT_BIN")")

  # 1. Check zellij is installed
  if command -v zellij &>/dev/null; then
    echo "  zellij: ok"
  else
    echo "  zellij: not found. Install with: brew install zellij"
    ERRORS=1
  fi

  # 2. Find the Zellij plugin
  PLUGIN_PATH=""
  if PLUGIN_PATH=$(resolve_plugin_path); then
    if [ "$PLUGIN_PATH" = "$HOME/.local/share/zelligent/zelligent-plugin.wasm" ]; then
      echo "  plugin: 🔧 dev build ($PLUGIN_PATH)"
    else
      echo "  plugin: ok ($PLUGIN_PATH)"
    fi
  else
    echo "  plugin: not found"
    echo "          Install with: brew install pcomans/zelligent/zelligent"
    echo "          Or from source: bash dev-install.sh"
    ERRORS=1
  fi

  DEFAULT_LAYOUT=""
  if DEFAULT_LAYOUT=$(resolve_default_layout_path); then
    echo "  default layout: ok ($DEFAULT_LAYOUT)"
  else
    echo "  default layout: not found"
    ERRORS=1
  fi

  # 3. Check required assets were found before continuing setup
  if [ -z "$PLUGIN_PATH" ] || [ -z "$DEFAULT_LAYOUT" ]; then
    if [ "$ERRORS" -ne 0 ]; then
      echo ""
      echo "Some checks failed. Fix the issues above and run 'zelligent doctor' again."
      exit 1
    fi
  fi

  CONFIG="$ZELLIJ_CONFIG_HOME/config.kdl"
  mkdir -p "$(dirname "$CONFIG")"
  touch "$CONFIG"
  echo "  keybinding: skipped (persistent sidebar only)"

  if [ "$(uname)" = "Darwin" ]; then
    if grep -v '^\s*//' "$CONFIG" | grep -qF 'copy_command'; then
      echo "  copy_command: ok"
    else
      echo 'copy_command "pbcopy"' >> "$CONFIG"
      echo "  copy_command: added pbcopy to $CONFIG"
    fi
  fi

  # 5. Serialization interval (keeps session snapshots fresh for resurrection)
  if grep -v '^\s*//' "$CONFIG" | grep -qF 'serialization_interval'; then
    echo "  serialization_interval: ok"
  else
    echo 'serialization_interval 5' >> "$CONFIG"
    echo "  serialization_interval: set to 5s in $CONFIG"
  fi

  # 6. Grant plugin permissions
  if [ "$(uname)" = "Darwin" ]; then
    PERM_FILE="$HOME/Library/Caches/org.Zellij-Contributors.Zellij/permissions.kdl"
  else
    PERM_FILE="${XDG_CACHE_HOME:-$HOME/.cache}/zellij/permissions.kdl"
  fi
  mkdir -p "$(dirname "$PERM_FILE")"
  touch "$PERM_FILE"

  if grep -qF "$PLUGIN_PATH" "$PERM_FILE"; then
    # Ensure ReadCliPipes is present (added in agent-status-notifications)
    if ! grep -qF "ReadCliPipes" "$PERM_FILE"; then
      # Append ReadCliPipes inside the existing plugin permissions block
      sed -i.bak "/$PLUGIN_PATH/,/}/ s/}/    ReadCliPipes\\
}/" "$PERM_FILE" && rm -f "$PERM_FILE.bak"
      echo "  permissions: added ReadCliPipes to $PERM_FILE"
    else
      echo "  permissions: ok"
    fi
  else
    cat >> "$PERM_FILE" <<PERMS
"$PLUGIN_PATH" {
    ChangeApplicationState
    ReadApplicationState
    RunCommands
    ReadCliPipes
}
PERMS
    echo "  permissions: granted for $PLUGIN_PATH"
  fi

  LAYOUT_DIR="$HOME/.zelligent"
  USER_LAYOUT="$LAYOUT_DIR/layout.kdl"
  mkdir -p "$LAYOUT_DIR"
  if [ ! -f "$USER_LAYOUT" ]; then
    cp "$DEFAULT_LAYOUT" "$USER_LAYOUT"
    echo "  layout: installed default to $USER_LAYOUT"
  elif cmp -s "$USER_LAYOUT" "$DEFAULT_LAYOUT"; then
    echo "  layout: ok ($USER_LAYOUT)"
  else
    echo "  layout: differs from default"
    echo "          Refresh with: cp \"$DEFAULT_LAYOUT\" \"$USER_LAYOUT\""
  fi

  # 7. Install Claude Code plugin (skill + hooks)
  if ! command -v claude &>/dev/null; then
    echo "  claude plugin: claude CLI not found (skipped)"
  else
    PLUGIN_MARKETPLACE=""
    if [ -n "$ZELLIGENT_PLUGIN_DIR" ]; then
      if [ -d "$ZELLIGENT_PLUGIN_DIR" ]; then
        PLUGIN_MARKETPLACE="$ZELLIGENT_PLUGIN_DIR"
      else
        echo "  claude plugin: ZELLIGENT_PLUGIN_DIR not found ($ZELLIGENT_PLUGIN_DIR)"
        ERRORS=1
      fi
    else
      HOMEBREW_PLUGIN="$ZELLIGENT_PREFIX/share/zelligent/claude-plugin"
      DEV_PLUGIN_DIR="$HOME/.local/share/zelligent/claude-plugin"
      SOURCE_PLUGIN="$SCRIPT_DIR/claude-plugin"
      if [ -d "$HOMEBREW_PLUGIN" ]; then
        PLUGIN_MARKETPLACE="$HOMEBREW_PLUGIN"
      elif [ -d "$DEV_PLUGIN_DIR" ]; then
        PLUGIN_MARKETPLACE="$DEV_PLUGIN_DIR"
      elif [ -d "$SOURCE_PLUGIN" ]; then
        PLUGIN_MARKETPLACE="$SOURCE_PLUGIN"
      fi
    fi

    if [ -z "$PLUGIN_MARKETPLACE" ]; then
      echo "  claude plugin: not bundled (skipped)"
    else
      claude plugin marketplace add "$PLUGIN_MARKETPLACE" 2>/dev/null || true
      if claude plugin list 2>/dev/null | grep -qF 'zelligent@zelligent'; then
        if claude plugin update zelligent@zelligent 2>/dev/null; then
          echo "  claude plugin: updated"
        else
          echo "  claude plugin: ok (update check failed)"
        fi
      else
        if claude plugin install zelligent@zelligent 2>/dev/null; then
          echo "  claude plugin: installed"
        else
          echo "  claude plugin: failed to install (run 'claude plugin install zelligent@zelligent' manually)"
          ERRORS=1
        fi
      fi
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

REPO_ROOT=$(cd "${GIT_COMMON_DIR%/.git}" && pwd -P)
REPO_NAME=$(basename "$REPO_ROOT")
# Resolve symlinks on the base dir (e.g. /tmp → /private/tmp on macOS) so path prefix matching works.
# Only resolve the parent (~/.zelligent/worktrees) which always exists after first spawn;
# don't mkdir the repo-specific dir as a side effect of read-only commands.
WORKTREES_BASE="$HOME/.zelligent/worktrees"
if [ -d "$WORKTREES_BASE" ]; then
  WORKTREES_BASE=$(cd "$WORKTREES_BASE" && pwd -P)
fi
WORKTREES_DIR="$WORKTREES_BASE/$REPO_NAME"

# Handle nuke subcommand — delete the repo's Zellij session so it won't resurrect
if [ "$1" = "nuke" ]; then
  if [ -n "$ZELLIJ" ]; then
    echo "Error: cannot nuke from inside a Zellij session. Detach first." >&2
    exit 1
  fi
  # Kill the session if it's currently active
  zellij delete-session --force "$REPO_NAME" 2>/dev/null || true
  # Also kill any lingering server/client processes for this session.
  # delete-session --force removes the socket but stale server processes can survive
  # and keep re-serializing the session layout to the cache directory.
  zellij_version=$(zellij --version 2>/dev/null | awk '{print $2}')
  if [ -n "$zellij_version" ]; then
    socket_path="${TMPDIR:-/tmp}/zellij-$(id -u)/$zellij_version/$REPO_NAME"
    # Force-kill server processes for this session's socket.
    # SIGTERM is often ignored by Zellij servers, so use SIGKILL.
    # Use grep -F instead of pkill -f to avoid regex metacharacter issues.
    server_pids=$(ps -eo pid=,args= | grep -F "zellij --server $socket_path" | grep -v grep | awk '{print $1}' || true)
    if [ -n "$server_pids" ]; then
      kill -9 $server_pids 2>/dev/null || true
    fi
    # Kill client processes attached to this session
    client_pids=$(ps -eo pid=,args= | grep -F "zellij attach $REPO_NAME" | grep -v grep | awk '{print $1}' || true)
    if [ -n "$client_pids" ]; then
      kill -9 $client_pids 2>/dev/null || true
    fi
  fi
  # Wait for processes to exit and finish any final serialization
  sleep 1
  # Remove the resurrection cache so the session won't come back on next attach.
  # Zellij discovers resurrectable sessions by scanning the session_info cache dir.
  # Paths:
  #   macOS: ~/Library/Caches/org.Zellij-Contributors.Zellij/VERSION/session_info/SESSION/
  #   Linux: ~/.cache/zellij/VERSION/session_info/SESSION/
  if [ -n "$zellij_version" ]; then
    for cache_base in \
      "$HOME/Library/Caches/org.Zellij-Contributors.Zellij" \
      "${XDG_CACHE_HOME:-$HOME/.cache}/zellij"; do
      cache_dir="$cache_base/$zellij_version/session_info/$REPO_NAME"
      rm -rf "$cache_dir" 2>/dev/null || true
    done
  fi
  # Clean up stale socket if still present
  if [ -n "$zellij_version" ]; then
    rm -f "${TMPDIR:-/tmp}/zellij-$(id -u)/$zellij_version/$REPO_NAME" 2>/dev/null || true
  fi
  echo "Deleted session '$REPO_NAME'. Next 'zelligent' will start fresh."
  exit 0
fi

# No args: launch or attach to Zellij session for this repo
if [ -z "$1" ]; then
  if [ -n "$ZELLIJ" ]; then
    echo "Already inside a Zellij session. Use 'zelligent spawn <branch>' to open a worktree tab."
    exit 0
  fi

  if zellij_list_sessions | grep -qxF "$REPO_NAME"; then
    echo "Attaching to session '$REPO_NAME'..."
    exec zellij attach "$REPO_NAME"
  else
    if ! PLUGIN_PATH=$(resolve_plugin_path); then
      echo "zelligent: could not find the sidebar plugin. Run 'zelligent doctor' to install it." >&2
      exit 1
    fi
    if ! LAYOUT_TEMPLATE=$(resolve_layout_template_path); then
      echo "zelligent: could not find a layout template. Run 'zelligent doctor' to install ~/.zelligent/layout.kdl." >&2
      exit 1
    fi
    if ! validate_layout_template "$LAYOUT_TEMPLATE" "zelligent"; then
      exit 1
    fi

    DEFAULT_AGENT_CMD="${SHELL:-bash}"
    ZELLIGENT_CMD=$(command -v zelligent 2>/dev/null || echo "$0")
    SIDEBAR_PANE=$(build_sidebar_pane "$PLUGIN_PATH" "$ZELLIGENT_CMD" "$DEFAULT_AGENT_CMD")
    AGENT_FRAGMENT=$(build_agent_command_fragment "$REPO_NAME" "$DEFAULT_AGENT_CMD" false "" "$REPO_ROOT" "$REPO_ROOT")
    CWD_KDL=$(escape_kdl "$REPO_ROOT")
    AGENT_FRAGMENT_KDL=$(escape_kdl "$AGENT_FRAGMENT")

    mkdir -p "$HOME/.zelligent/tmp"
    BODY_LAYOUT=$(mktemp "$HOME/.zelligent/tmp/layout-body-XXXXXX")
    SESSION_LAYOUT=$(mktemp "$HOME/.zelligent/tmp/layout-session-XXXXXX")
    trap 'rm -f "$BODY_LAYOUT" "$SESSION_LAYOUT"' EXIT

    render_layout_template "$LAYOUT_TEMPLATE" "$BODY_LAYOUT" "$CWD_KDL" "$AGENT_FRAGMENT_KDL" "$SIDEBAR_PANE"
    echo "Creating Zellij session '$REPO_NAME'..."
    write_session_layout "$SESSION_LAYOUT" "$REPO_NAME" "$BODY_LAYOUT" "$SIDEBAR_PANE"
    exec zellij --new-session-with-layout "$SESSION_LAYOUT" --session "$REPO_NAME"
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
        current_dir=""
        if [[ "$current_path" == "$SPAWN_PREFIX"* ]]; then
          current_dir="${current_path#$SPAWN_PREFIX}"
        fi
        ;;
      "branch "*)
        if [ -n "$current_dir" ]; then
          printf '%s\t%s\n' "$current_dir" "${line#branch refs/heads/}"
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
  SESSION_NAME=$(printf '%s' "$SESSION_NAME" | tr -cd 'a-zA-Z0-9_-')
  WORKTREE_PATH=$(git -C "$REPO_ROOT" worktree list --porcelain | awk -v branch="branch refs/heads/$BRANCH_NAME" '
    /^worktree / { path = substr($0, 10) }
    $0 == branch { print path; exit }
  ')
  if [ -z "$WORKTREE_PATH" ] || [ ! -d "$WORKTREE_PATH" ]; then
    echo "Error: no worktree found for branch '$BRANCH_NAME'." >&2
    exit 1
  fi
  case "$WORKTREE_PATH" in
    "$WORKTREES_DIR"/*) ;;
    *)
      echo "Error: worktree '$WORKTREE_PATH' is not managed by zelligent." >&2
      exit 1
      ;;
  esac
  if [ -f "$REPO_ROOT/.zelligent/teardown.sh" ]; then
    echo "⚙️  Running .zelligent/teardown.sh..."
    if ! bash "$REPO_ROOT/.zelligent/teardown.sh" "$REPO_ROOT" "$WORKTREE_PATH"; then
      echo "Error: teardown.sh failed. Worktree was NOT removed." >&2
      exit 1
    fi
  fi
  if ! git worktree remove "$WORKTREE_PATH" 2>/dev/null; then
    echo "Error: could not remove worktree. It may have uncommitted changes." >&2
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
  AGENT_CMD=${3:-${SHELL:-bash}}
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
# Strip any characters outside the safe set for session/tab names
SESSION_NAME=$(printf '%s' "$SESSION_NAME" | tr -cd 'a-zA-Z0-9_-')

# Detect default base branch
if BASE_REF=$(git symbolic-ref refs/remotes/origin/HEAD 2>/dev/null); then
  BASE_BRANCH="${BASE_REF#refs/remotes/origin/}"
else
  BASE_BRANCH="main"
fi

# Resolve assets and the active layout template
if ! PLUGIN_PATH=$(resolve_plugin_path); then
  echo "spawn: could not find the sidebar plugin. Run 'zelligent doctor' to install it." >&2
  exit 1
fi
if ! LAYOUT_TEMPLATE=$(resolve_layout_template_path); then
  echo "spawn: could not find a layout template. Run 'zelligent doctor' to install ~/.zelligent/layout.kdl." >&2
  exit 1
fi
if ! validate_layout_template "$LAYOUT_TEMPLATE" "spawn"; then
  exit 1
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

SETUP_SCRIPT="$REPO_ROOT/.zelligent/setup.sh"
ZELLIGENT_CMD=$(command -v zelligent 2>/dev/null || echo "$0")
SIDEBAR_PANE=$(build_sidebar_pane "$PLUGIN_PATH" "$ZELLIGENT_CMD" "$AGENT_CMD")
AGENT_FRAGMENT=$(build_agent_command_fragment "$SESSION_NAME" "$AGENT_CMD" "$NEW_WORKTREE" "$SETUP_SCRIPT" "$REPO_ROOT" "$WORKTREE_PATH")
CWD_KDL=$(escape_kdl "$WORKTREE_PATH")
AGENT_FRAGMENT_KDL=$(escape_kdl "$AGENT_FRAGMENT")

mkdir -p "$HOME/.zelligent/tmp"
LAYOUT=$(mktemp "$HOME/.zelligent/tmp/layout-XXXXXX")
BODY_LAYOUT=$(mktemp "$HOME/.zelligent/tmp/layout-body-XXXXXX")
trap 'rm -f "$LAYOUT" "$BODY_LAYOUT"' EXIT

if [ -n "$ZELLIJ" ] || zellij_list_sessions | grep -qxF "$REPO_NAME"; then
  render_layout_template "$LAYOUT_TEMPLATE" "$LAYOUT" "$CWD_KDL" "$AGENT_FRAGMENT_KDL" "$SIDEBAR_PANE"
else
  render_layout_template "$LAYOUT_TEMPLATE" "$BODY_LAYOUT" "$CWD_KDL" "$AGENT_FRAGMENT_KDL" "$SIDEBAR_PANE"
  write_session_layout "$LAYOUT" "$SESSION_NAME" "$BODY_LAYOUT" "$SIDEBAR_PANE"
fi

# Inside Zellij: open as a new tab in the current session.
# Outside Zellij: create or attach to a repo-named session, open worktree as a tab.
if [ -n "$ZELLIJ" ]; then
  echo "🪟 Opening tab '$SESSION_NAME'..."
  zellij action new-tab --layout "$LAYOUT" --name "$SESSION_NAME"
elif zellij_list_sessions | grep -qxF "$REPO_NAME"; then
  echo "🪟 Attaching to session '$REPO_NAME', opening tab '$SESSION_NAME'..."
  ZELLIJ_SESSION_NAME="$REPO_NAME" zellij action new-tab --layout "$LAYOUT" --name "$SESSION_NAME"
  zellij attach "$REPO_NAME"
else
  echo "🪟 Creating Zellij session '$REPO_NAME'..."
  zellij --new-session-with-layout "$LAYOUT" --session "$REPO_NAME"
fi
