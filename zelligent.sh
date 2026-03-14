#!/bin/bash
# commit: __COMMIT_SHA__

# Exit immediately if a command exits with a non-zero status
set -e

ZELLIJ_CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}/zellij"
ZELLIGENT_SHARE_DIR_DEV="$HOME/.local/share/zelligent"
ZELLIGENT_USER_DIR="$HOME/.zelligent"

escape_kdl_string() {
  local value="$1"
  value=${value//\\/\\\\}
  value=${value//\"/\\\"}
  printf '%s' "$value"
}

shell_quote() {
  printf "'%s'" "$(printf '%s' "$1" | sed "s/'/'\\\\''/g")"
}

resolve_install_prefix() {
  local zelligent_bin
  zelligent_bin=$(command -v zelligent 2>/dev/null || true)
  if [ -n "$zelligent_bin" ]; then
    dirname "$(dirname "$zelligent_bin")"
  fi
}

# Run a command with a hard timeout and kill its whole process group if it hangs.
run_with_timeout() {
  local timeout_seconds="$1"
  shift

  perl -e '
    use POSIX qw(setsid);

    my $timeout = shift @ARGV;
    my $pid = fork();
    die "fork failed\n" unless defined $pid;

    if ($pid == 0) {
      setsid() or die "setsid failed\n";
      exec @ARGV or die "exec failed: $!\n";
    }

    local $SIG{ALRM} = sub {
      kill "TERM", -$pid;
      select undef, undef, undef, 0.2;
      kill "KILL", -$pid;
      waitpid($pid, 0);
      exit 142;
    };

    alarm $timeout;
    waitpid($pid, 0);
    alarm 0;

    if ($? & 127) {
      exit 128 + ($? & 127);
    }
    exit $? >> 8;
  ' "$timeout_seconds" "$@"
}

# Wrapper with 3s timeout to avoid hanging on stale Zellij sockets
zellij_list_sessions() {
  local output
  if output=$(run_with_timeout 3 zellij list-sessions --no-formatting --short 2>/dev/null); then
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

resolve_shared_asset_path() {
  local override_var="$1"
  local asset_name="$2"
  local override_path zelligent_prefix bundled_asset dev_asset

  override_path="${!override_var}"
  if [ -n "$override_path" ]; then
    [ -f "$override_path" ] || return 1
    printf '%s\n' "$override_path"
    return 0
  fi

  zelligent_prefix=$(resolve_install_prefix)
  if [ -n "$zelligent_prefix" ]; then
    bundled_asset="$zelligent_prefix/share/zelligent/$asset_name"
    if [ -f "$bundled_asset" ]; then
      printf '%s\n' "$bundled_asset"
      return 0
    fi
  fi

  dev_asset="$ZELLIGENT_SHARE_DIR_DEV/$asset_name"
  if [ -f "$dev_asset" ]; then
    printf '%s\n' "$dev_asset"
    return 0
  fi

  return 1
}

# Resolve the zelligent WASM plugin path, honoring explicit overrides first.
resolve_plugin_path() {
  resolve_shared_asset_path ZELLIGENT_PLUGIN_SRC zelligent-plugin.wasm
}

resolve_default_layout_path() {
  resolve_shared_asset_path ZELLIGENT_DEFAULT_LAYOUT_SRC default-layout.kdl
}

resolve_layout_source() {
  local repo_layout user_layout
  repo_layout="$REPO_ROOT/.zelligent/layout.kdl"
  user_layout="$ZELLIGENT_USER_DIR/layout.kdl"

  if [ -f "$repo_layout" ]; then
    printf '%s\n' "$repo_layout"
    return 0
  fi

  if [ -f "$user_layout" ]; then
    printf '%s\n' "$user_layout"
    return 0
  fi

  return 1
}

count_layout_placeholder() {
  local layout_source="$1"
  local placeholder="$2"
  ZELLIGENT_COUNT_NEEDLE="$placeholder" perl -0ne '
    my $content = $_;
    my $needle = $ENV{ZELLIGENT_COUNT_NEEDLE};
    my $count = 0;
    my $pos = 0;
    my $len = length($content);

    while ($pos < $len) {
      my $char = substr($content, $pos, 1);
      if ($char eq q{"}) {
        $pos++;
        while ($pos < $len) {
          my $string_char = substr($content, $pos, 1);
          if ($string_char eq q{\\}) {
            $pos += 2;
            next;
          }
          $pos++;
          last if $string_char eq q{"};
        }
        next;
      }
      if (substr($content, $pos, 2) eq "//") {
        my $newline = index($content, "\n", $pos + 2);
        $pos = $newline >= 0 ? $newline + 1 : $len;
        next;
      }
      if (substr($content, $pos, 2) eq "/*") {
        my $end = index($content, "*/", $pos + 2);
        if ($end < 0) {
          print STDERR "Error: unterminated block comment in layout file.\n";
          exit 1;
        }
        $pos = $end + 2;
        next;
      }
      if (substr($content, $pos, length($needle)) eq $needle) {
        $count++;
        $pos += length($needle);
        next;
      }
      $pos++;
    }

    print $count;
  ' "$layout_source"
}

validate_layout_source() {
  local layout_source="$1"
  local sidebar_count children_count

  sidebar_count=$(count_layout_placeholder "$layout_source" "{{zelligent_sidebar}}")
  children_count=$(count_layout_placeholder "$layout_source" "{{zelligent_children}}")

  if [ "$sidebar_count" -ne 1 ] || [ "$children_count" -ne 1 ]; then
    echo "Error: layout '$layout_source' must contain {{zelligent_sidebar}} and {{zelligent_children}} exactly once." >&2
    if [ "$layout_source" = "$ZELLIGENT_USER_DIR/layout.kdl" ]; then
      echo "Run 'zelligent doctor' to recreate the default user layout or fix the file manually." >&2
    fi
    return 1
  fi
}

sidebar_plugin_content() {
  local plugin_path="$1"
  local raw_agent_cmd="$2"
  local plugin_path_kdl zelligent_path_cmd zelligent_path_cmd_kdl raw_agent_cmd_kdl

  plugin_path_kdl=$(escape_kdl_string "$plugin_path")
  zelligent_path_cmd=$(command -v zelligent 2>/dev/null || echo "$0")
  zelligent_path_cmd_kdl=$(escape_kdl_string "$zelligent_path_cmd")
  raw_agent_cmd_kdl=$(escape_kdl_string "$raw_agent_cmd")

  cat <<EOF
plugin location="file:$plugin_path_kdl" {
    zelligent_path "$zelligent_path_cmd_kdl"
    agent_cmd "$raw_agent_cmd_kdl"
}
EOF
}

default_tab_children_content() {
  local cwd_value="$1"
  local agent_cmd_kdl="$2"
  local cwd_kdl

  cwd_kdl=$(escape_kdl_string "$cwd_value")

  cat <<EOF
pane {
    pane command="bash" cwd="$cwd_kdl" size="70%" {
        args "-lc" "$agent_cmd_kdl"
    }
    pane command="lazygit" cwd="$cwd_kdl" size="30%"
}
EOF
}

render_layout_fragment() {
  local template_path="$1"
  local output_path="$2"
  local cwd_value="$3"
  local agent_cmd_value="$4"
  local sidebar_value="$5"
  local children_value="$6"

  validate_layout_source "$template_path"

  ZELLIGENT_RENDER_CWD="$(escape_kdl_string "$cwd_value")" \
  ZELLIGENT_RENDER_AGENT_CMD="$agent_cmd_value" \
  ZELLIGENT_RENDER_SIDEBAR="$sidebar_value" \
  ZELLIGENT_RENDER_CHILDREN="$children_value" \
    perl -0ne '
      my $content = $_;
      my $cwd = $ENV{ZELLIGENT_RENDER_CWD};
      my $agent_cmd = $ENV{ZELLIGENT_RENDER_AGENT_CMD};
      my $sidebar = $ENV{ZELLIGENT_RENDER_SIDEBAR};
      my $children = $ENV{ZELLIGENT_RENDER_CHILDREN};
      my $out = "";
      my $pos = 0;
      my $len = length($content);

      while ($pos < $len) {
        if (substr($content, $pos, 2) eq "//") {
          my $newline = index($content, "\n", $pos + 2);
          if ($newline < 0) {
            $out .= substr($content, $pos);
            last;
          }
          $out .= substr($content, $pos, $newline + 1 - $pos);
          $pos = $newline + 1;
          next;
        }
        if (substr($content, $pos, 2) eq "/*") {
          my $end = index($content, "*/", $pos + 2);
          if ($end < 0) {
            print STDERR "Error: unterminated block comment in layout file.\n";
            exit 1;
          }
          $out .= substr($content, $pos, $end + 2 - $pos);
          $pos = $end + 2;
          next;
        }
        if (substr($content, $pos, 7) eq "{{cwd}}") {
          $out .= $cwd;
          $pos += 7;
          next;
        }
        if (substr($content, $pos, 13) eq "{{agent_cmd}}") {
          $out .= $agent_cmd;
          $pos += 13;
          next;
        }
        if (substr($content, $pos, 21) eq "{{zelligent_sidebar}}") {
          $out .= $sidebar;
          $pos += 21;
          next;
        }
        if (substr($content, $pos, 22) eq "{{zelligent_children}}") {
          $out .= $children;
          $pos += 22;
          next;
        }
        if (substr($content, $pos, 1) eq q{"}) {
          $out .= q{"};
          $pos++;
          while ($pos < $len) {
            if (substr($content, $pos, 7) eq "{{cwd}}") {
              $out .= $cwd;
              $pos += 7;
              next;
            }
            if (substr($content, $pos, 13) eq "{{agent_cmd}}") {
              $out .= $agent_cmd;
              $pos += 13;
              next;
            }

            my $string_char = substr($content, $pos, 1);
            $out .= $string_char;
            if ($string_char eq q{\\} && $pos + 1 < $len) {
              $pos++;
              $out .= substr($content, $pos, 1);
            } elsif ($string_char eq q{"}) {
              $pos++;
              last;
            }
            $pos++;
          }
          next;
        }

        $out .= substr($content, $pos, 1);
        $pos++;
      }

      print $out;
    ' "$template_path" > "$output_path"
}

build_agent_command_value() {
  local raw_agent_cmd="$1"
  local session_name="$2"
  local repo_root="$3"
  local worktree_path="$4"
  local setup_script="$5"
  local is_new_worktree="$6"
  local command

  command="export ZELLIGENT_TAB_NAME=$(shell_quote "$session_name"); "
  if [ "$is_new_worktree" = "true" ] && [ -f "$setup_script" ]; then
    command="${command}bash $(shell_quote "$setup_script") $(shell_quote "$repo_root") $(shell_quote "$worktree_path") || { echo 'Setup failed (exit '\$?'). Press Enter to close.'; read; exit 1; }; "
  fi
  command="${command}exec $raw_agent_cmd"
  escape_kdl_string "$command"
}

write_fragment_layout() {
  local output_path="$1"
  local fragment_path="$2"

  {
    echo "layout {"
    sed 's/^/    /' "$fragment_path"
    echo "}"
  } > "$output_path"
}

write_session_layout() {
  local output_path="$1"
  local default_fragment_path="$2"
  local initial_children_path="$3"
  local tab_name="$4"
  local tab_name_kdl

  tab_name_kdl=$(escape_kdl_string "$tab_name")

  {
    echo "layout {"
    echo "    default_tab_template {"
    sed 's/^/        /' "$default_fragment_path"
    echo "    }"
    echo "    tab name=\"$tab_name_kdl\" {"
    sed 's/^/        /' "$initial_children_path"
    echo "    }"
    echo "}"
  } > "$output_path"
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
  SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

  # 1. Check zellij is installed
  if command -v zellij &>/dev/null; then
    echo "  zellij: ok"
  else
    echo "  zellij: not found. Install with: brew install zellij"
    ERRORS=1
  fi

  # 2. Find the Zellij plugin
  PLUGIN_PATH=""
  PLUGIN_MODE=""
  if PLUGIN_PATH=$(resolve_plugin_path); then
    if [ -n "$ZELLIGENT_PLUGIN_SRC" ]; then
      PLUGIN_MODE="custom"
    elif [ "$PLUGIN_PATH" = "$HOME/.local/share/zelligent/zelligent-plugin.wasm" ]; then
      PLUGIN_MODE="dev"
    else
      PLUGIN_MODE="homebrew"
    fi
  elif [ -n "$ZELLIGENT_PLUGIN_SRC" ]; then
    echo "  plugin: source not found ($ZELLIGENT_PLUGIN_SRC)"
    ERRORS=1
  fi

  if [ -z "$PLUGIN_PATH" ]; then
    echo "  plugin: not found"
    echo "          Install with: brew install pcomans/zelligent/zelligent"
    echo "          Or from source: bash dev-install.sh"
    ERRORS=1
  elif [ "$PLUGIN_MODE" = "dev" ]; then
    echo "  plugin: 🔧 dev build ($PLUGIN_PATH)"
  else
    echo "  plugin: ok ($PLUGIN_PATH)"
  fi

  # 3. Check plugin was found before continuing setup
  if [ -z "$PLUGIN_PATH" ]; then
    if [ "$ERRORS" -ne 0 ]; then
      echo ""
      echo "Some checks failed. Fix the issues above and run 'zelligent doctor' again."
      exit 1
    fi
  fi

  DEFAULT_LAYOUT_PATH=""
  if DEFAULT_LAYOUT_PATH=$(resolve_default_layout_path); then
    echo "  default layout: ok ($DEFAULT_LAYOUT_PATH)"
  elif [ -n "$ZELLIGENT_DEFAULT_LAYOUT_SRC" ]; then
    echo "  default layout: source not found ($ZELLIGENT_DEFAULT_LAYOUT_SRC)"
    ERRORS=1
  else
    echo "  default layout: not found"
    echo "                 Install with: brew install pcomans/zelligent/zelligent"
    echo "                 Or from source: bash dev-install.sh"
    ERRORS=1
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

  USER_LAYOUT_PATH="$ZELLIGENT_USER_DIR/layout.kdl"
  mkdir -p "$ZELLIGENT_USER_DIR"
  if [ -n "$DEFAULT_LAYOUT_PATH" ]; then
    if [ ! -f "$USER_LAYOUT_PATH" ]; then
      cp "$DEFAULT_LAYOUT_PATH" "$USER_LAYOUT_PATH"
      echo "  layout: created $USER_LAYOUT_PATH"
    elif cmp -s "$DEFAULT_LAYOUT_PATH" "$USER_LAYOUT_PATH"; then
      echo "  layout: ok"
    else
      echo "  layout: custom user layout differs from shipped default"
      echo "          Overwrite with: cp \"$DEFAULT_LAYOUT_PATH\" \"$USER_LAYOUT_PATH\""
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
    PLUGIN_PATH_STARTUP=""
    if ! PLUGIN_PATH_STARTUP=$(resolve_plugin_path); then
      if [ -n "$ZELLIGENT_PLUGIN_SRC" ]; then
        echo "Plugin source not found: $ZELLIGENT_PLUGIN_SRC" >&2
      else
        echo "Plugin not installed. Run 'zelligent doctor' to set up." >&2
      fi
      exit 1
    fi

    LAYOUT_SOURCE_STARTUP=""
    if ! LAYOUT_SOURCE_STARTUP=$(resolve_layout_source); then
      echo "Error: no layout found. Expected .zelligent/layout.kdl or $ZELLIGENT_USER_DIR/layout.kdl." >&2
      echo "Run 'zelligent doctor' to create the default user layout." >&2
      exit 1
    fi

    mkdir -p "$ZELLIGENT_USER_DIR/tmp"
    RENDERED_STARTUP_TEMPLATE=$(mktemp "$ZELLIGENT_USER_DIR/tmp/layout-startup-template-XXXXXX")
    RENDERED_STARTUP_CHILDREN=$(mktemp "$ZELLIGENT_USER_DIR/tmp/layout-startup-children-XXXXXX")
    STARTUP_LAYOUT=$(mktemp "$ZELLIGENT_USER_DIR/tmp/layout-startup-session-XXXXXX")
    trap 'rm -f "$RENDERED_STARTUP_TEMPLATE" "$RENDERED_STARTUP_CHILDREN" "$STARTUP_LAYOUT"' EXIT

    STARTUP_AGENT_CMD="$SHELL"
    STARTUP_AGENT_RENDER=$(build_agent_command_value "$STARTUP_AGENT_CMD" "$REPO_NAME" "$REPO_ROOT" "$REPO_ROOT" "" "false")
    STARTUP_SIDEBAR=$(sidebar_plugin_content "$PLUGIN_PATH_STARTUP" "$STARTUP_AGENT_CMD")
    STARTUP_CHILDREN=$(default_tab_children_content "$REPO_ROOT" "$STARTUP_AGENT_RENDER")
    render_layout_fragment "$LAYOUT_SOURCE_STARTUP" "$RENDERED_STARTUP_TEMPLATE" "$REPO_ROOT" "$STARTUP_AGENT_RENDER" "$STARTUP_SIDEBAR" "children"
    printf '%s\n' "$STARTUP_CHILDREN" > "$RENDERED_STARTUP_CHILDREN"
    write_session_layout "$STARTUP_LAYOUT" "$RENDERED_STARTUP_TEMPLATE" "$RENDERED_STARTUP_CHILDREN" "$REPO_NAME"

    echo "Creating Zellij session '$REPO_NAME'..."
    zellij --new-session-with-layout "$STARTUP_LAYOUT" --session "$REPO_NAME"
    exit $?
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
# Strip any characters outside the safe set for session/tab names
SESSION_NAME=$(printf '%s' "$SESSION_NAME" | tr -cd 'a-zA-Z0-9_-')

# Detect default base branch
if BASE_REF=$(git symbolic-ref refs/remotes/origin/HEAD 2>/dev/null); then
  BASE_BRANCH="${BASE_REF#refs/remotes/origin/}"
else
  BASE_BRANCH="main"
fi

# Define the new centralized worktree path
WORKTREE_PATH="$WORKTREES_DIR/$BRANCH_NAME"

# Resolve plugin and selected layout source before mutating worktrees.
if ! PLUGIN_PATH_LAYOUT=$(resolve_plugin_path); then
  echo "Error: could not resolve the zelligent sidebar plugin." >&2
  echo "Run 'zelligent doctor' or reinstall the plugin." >&2
  exit 1
fi

if ! LAYOUT_SOURCE=$(resolve_layout_source); then
  echo "Error: no layout found. Expected .zelligent/layout.kdl or $ZELLIGENT_USER_DIR/layout.kdl." >&2
  echo "Run 'zelligent doctor' to create the default user layout." >&2
  exit 1
fi

if ! validate_layout_source "$LAYOUT_SOURCE"; then
  exit 1
fi

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

# Generate temp layout files
mkdir -p "$ZELLIGENT_USER_DIR/tmp"
LAYOUT=$(mktemp "$ZELLIGENT_USER_DIR/tmp/layout-XXXXXX")
RENDERED_TAB_FRAGMENT=$(mktemp "$ZELLIGENT_USER_DIR/tmp/layout-tab-fragment-XXXXXX")
RENDERED_SESSION_TEMPLATE=$(mktemp "$ZELLIGENT_USER_DIR/tmp/layout-session-template-XXXXXX")
trap 'rm -f "$LAYOUT" "$RENDERED_TAB_FRAGMENT" "$RENDERED_SESSION_TEMPLATE"' EXIT

SETUP_SCRIPT="$REPO_ROOT/.zelligent/setup.sh"
AGENT_CMD_RENDER=$(build_agent_command_value "$AGENT_CMD" "$SESSION_NAME" "$REPO_ROOT" "$WORKTREE_PATH" "$SETUP_SCRIPT" "$NEW_WORKTREE")
SESSION_AGENT_RENDER=$(build_agent_command_value "$AGENT_CMD" "$REPO_NAME" "$REPO_ROOT" "$REPO_ROOT" "" "false")
SIDEBAR_RENDER=$(sidebar_plugin_content "$PLUGIN_PATH_LAYOUT" "$AGENT_CMD")
TAB_CHILDREN_RENDER=$(default_tab_children_content "$WORKTREE_PATH" "$AGENT_CMD_RENDER")
render_layout_fragment "$LAYOUT_SOURCE" "$RENDERED_TAB_FRAGMENT" "$WORKTREE_PATH" "$AGENT_CMD_RENDER" "$SIDEBAR_RENDER" "$TAB_CHILDREN_RENDER"
render_layout_fragment "$LAYOUT_SOURCE" "$RENDERED_SESSION_TEMPLATE" "$REPO_ROOT" "$SESSION_AGENT_RENDER" "$SIDEBAR_RENDER" "children"

if [ -n "$ZELLIJ" ]; then
  write_fragment_layout "$LAYOUT" "$RENDERED_TAB_FRAGMENT"
else
  write_session_layout "$LAYOUT" "$RENDERED_SESSION_TEMPLATE" "$RENDERED_TAB_FRAGMENT" "$SESSION_NAME"
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
