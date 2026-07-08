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

# --- Stale serialized session reconciliation (#155/#157/#158) -------------
#
# Zellij caches a resurrectable copy of every session it has seen
# (session-layout.kdl) under a cache dir named after the running zellij's
# version/contract, e.g. `contract_version_1` on 0.44.x, `0.43.1` on older
# releases — the exact directory name drifts across zellij versions and
# must never be hardcoded (#158). `zellij attach` (including
# `--create-background`) resurrects verbatim from that file whenever the
# named session isn't currently alive, even though `zellij list-sessions
# --short` prints EXITED sessions identically to alive ones. If the
# serialized layout points a plugin at a `file:` path that's since moved,
# been deleted, or no longer holds a valid wasm module, the resurrected
# session shows a broken plugin pane ("magic header not detected") and,
# because resurrection re-serializes, the corruption is sticky. See the
# design doc for the full research: docs/design-docs/session-resurrection.md
# and zelligent issue #155.

# Cache roots to search for serialized session_info dirs. Overridable via
# ZELLIGENT_ZELLIJ_CACHE_ROOTS (colon-separated) so tests never touch the
# real cache.
zellij_cache_roots() {
  if [ -n "$ZELLIGENT_ZELLIJ_CACHE_ROOTS" ]; then
    local root roots
    IFS=':' read -ra roots <<<"$ZELLIGENT_ZELLIJ_CACHE_ROOTS"
    # Print one-per-line with an explicit trailing newline on every entry —
    # a bare `tr ':' '\n'` leaves the last root without one when there's
    # only a single override path (no ':' to convert), and `while read`
    # silently drops a final line with no trailing newline.
    for root in "${roots[@]}"; do
      printf '%s\n' "$root"
    done
  else
    printf '%s\n' \
      "$HOME/Library/Caches/org.Zellij-Contributors.Zellij" \
      "${XDG_CACHE_HOME:-$HOME/.cache}/zellij"
  fi
}

# All existing `<cache_root>/*/session_info/<name>` directories across every
# cache root, tolerant of the version/contract dir name drifting (#158).
serialized_session_dirs() {
  local name="$1" root dir
  while IFS= read -r root; do
    [ -n "$root" ] && [ -d "$root" ] || continue
    for dir in "$root"/*/session_info/"$name"; do
      [ -d "$dir" ] && printf '%s\n' "$dir"
    done
  done < <(zellij_cache_roots)
}

# `session-layout.kdl` files for one session name, across cache roots.
serialized_layout_files() {
  local name="$1" dir
  while IFS= read -r dir; do
    [ -n "$dir" ] || continue
    [ -f "$dir/session-layout.kdl" ] && printf '%s\n' "$dir/session-layout.kdl"
  done < <(serialized_session_dirs "$name")
}

# `session-layout.kdl` files for EVERY serialized session, across cache
# roots — used by `zelligent doctor`'s sweep.
all_serialized_layout_files() {
  local root f
  while IFS= read -r root; do
    [ -n "$root" ] && [ -d "$root" ] || continue
    for f in "$root"/*/session_info/*/session-layout.kdl; do
      [ -f "$f" ] && printf '%s\n' "$f"
    done
  done < <(zellij_cache_roots)
}

# Long-form `zellij list-sessions`, timeout-guarded like zellij_list_sessions.
zellij_list_sessions_long() {
  run_with_timeout 3 zellij list-sessions --no-formatting 2>/dev/null || true
}

# alive | exited | none for a session name, parsed from the long-form
# listing. `--short` can't distinguish EXITED from alive (#155 finding 1.4);
# the long form marks EXITED sessions with a "(EXITED" suffix on their line.
# Fails open to "none" on any list-sessions error/timeout — the guard must
# never block startup.
session_state() {
  local name="$1" output line candidate
  output=$(zellij_list_sessions_long)
  [ -n "$output" ] || { printf 'none\n'; return 0; }
  while IFS= read -r line; do
    [ -n "$line" ] || continue
    candidate="${line%% \[Created*}"
    if [ "$candidate" = "$name" ]; then
      if printf '%s\n' "$line" | grep -qF '(EXITED'; then
        printf 'exited\n'
      else
        printf 'alive\n'
      fi
      return 0
    fi
  done <<<"$output"
  printf 'none\n'
}

# Candidate plugin `file:` URLs referenced by a serialized layout. Uses grep
# rather than a KDL parser — robust to formatting drift; a stray non-plugin
# match just fails validation, which triggers a fresh session (the safe
# direction). URL-decoding isn't needed for zelligent's own paths.
extract_plugin_file_urls() {
  local file="$1"
  grep -o 'file:[^"]*' "$file" 2>/dev/null || true
}

# Validate one `file:` plugin URL. Returns 0 (valid) or 1 (stale).
#   - missing file, or first 4 bytes aren't the wasm magic number  -> stale
#   - basename is zelligent-plugin.wasm but path != current_plugin_path
#     (install moved) -> stale
#   - URL-encoded paths (contain a %XX escape) are not decoded; treated as
#     "cannot validate safely" and pass.
validate_plugin_url() {
  local url="$1" current_plugin_path="$2" path magic
  path="${url#file:}"
  case "$path" in
    *%[0-9A-Fa-f][0-9A-Fa-f]*) return 0 ;;
  esac
  [ -f "$path" ] || return 1
  # Compare via hex (not a raw string) because bash can't hold the NUL byte
  # that starts the wasm magic number (\0asm) in a variable.
  magic=$(head -c4 "$path" 2>/dev/null | od -An -tx1 | tr -d ' \n')
  [ "$magic" = "0061736d" ] || return 1
  if [ "$(basename "$path")" = "zelligent-plugin.wasm" ] && [ -n "$current_plugin_path" ] && [ "$path" != "$current_plugin_path" ]; then
    return 1
  fi
  return 0
}

# Classify a layout file's staleness: prints "<kind>\t<bad_path>" where kind
# is one of none|zelligent|other. "zelligent" wins over "other" (a stale
# zelligent URL is always auto-fixable; a stale third-party URL alone is
# not) — see reconcile_serialized_session / doctor for how each is used.
layout_stale_kind() {
  local file="$1" current_plugin_path="$2" url path kind="none" bad_path=""
  while IFS= read -r url; do
    [ -n "$url" ] || continue
    if ! validate_plugin_url "$url" "$current_plugin_path"; then
      path="${url#file:}"
      if [ "$(basename "$path")" = "zelligent-plugin.wasm" ]; then
        kind="zelligent"
        bad_path="$path"
        break
      elif [ "$kind" != "zelligent" ]; then
        kind="other"
        bad_path="$path"
      fi
    fi
  done < <(extract_plugin_file_urls "$file")
  printf '%s\t%s\n' "$kind" "$bad_path"
}

# Drop a session's cache dirs and print the standard stale-session message.
# Guards itself: re-checks the session is still EXITED immediately before
# deleting, so no caller can race an exited->alive transition (a second
# `zelligent`/attach elsewhere) into `delete-session --force` killing a
# live session out from under its user. Callers must still pre-filter on
# exited state for correct messaging; this check is the last line of
# defense, not the primary gate.
drop_stale_session() {
  local name="$1" bad_path="$2" dir
  if [ "$(session_state "$name")" != "exited" ]; then
    echo "ℹ️  Skipped dropping saved session '$name' — it came alive while being checked."
    return 0
  fi
  zellij delete-session --force "$name" 2>/dev/null || true
  while IFS= read -r dir; do
    [ -n "$dir" ] && rm -rf "$dir" 2>/dev/null || true
  done < <(serialized_session_dirs "$name")
  echo "ℹ️  Dropped stale saved session '$name' (serialized plugin path no longer valid: $bad_path); starting fresh."
}

# The core guard (#155/#157): call before any flow that can attach to a
# session by name (`zellij list-sessions --short` doesn't distinguish
# EXITED from alive, so a naive existence probe can walk into resurrecting
# a broken layout). Never touches a live session — its server holds the
# plugin in memory and will re-serialize on its own. Fails open: any
# ambiguity (list-sessions timeout, unreadable/missing layout file, no
# file: URLs at all) leaves the session untouched.
reconcile_serialized_session() {
  local name="$1" current_plugin_path="$2" file kind bad_path

  [ "$(session_state "$name")" = "exited" ] || return 0

  while IFS= read -r file; do
    [ -n "$file" ] || continue
    IFS=$'\t' read -r kind bad_path < <(layout_stale_kind "$file" "$current_plugin_path")
    if [ "$kind" != "none" ]; then
      # Re-check immediately before deleting: the session could have
      # transitioned exited -> alive between the check above and here (a
      # second `zelligent` launch racing this one). delete-session --force
      # on a now-alive session would kill it out from under its user.
      if [ "$(session_state "$name")" = "exited" ]; then
        drop_stale_session "$name" "$bad_path"
      fi
      return 0
    fi
  done < <(serialized_layout_files "$name")
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
  local cwd_value="$3"
  local plugin_path_kdl zelligent_path_cmd zelligent_path_cmd_kdl raw_agent_cmd_kdl cwd_kdl

  plugin_path_kdl=$(escape_kdl_string "$plugin_path")
  zelligent_path_cmd=$(command -v zelligent 2>/dev/null || echo "$0")
  zelligent_path_cmd_kdl=$(escape_kdl_string "$zelligent_path_cmd")
  raw_agent_cmd_kdl=$(escape_kdl_string "$raw_agent_cmd")
  cwd_kdl=$(escape_kdl_string "$cwd_value")

  # `cwd` on the plugin block sets `RunPlugin.initial_cwd` for the live
  # session. Zellij's resurrection serializer drops the plugin cwd, though,
  # so we ALSO pass `repo_root` inside the user-config block — that field
  # IS preserved verbatim across resurrection. The plugin reads `repo_root`
  # in load() and prefers it whenever the resolved cwd looks bogus (e.g.
  # `/` after a resurrect). See docs/design-docs/session-resurrection.md.
  cat <<EOF
plugin location="file:$plugin_path_kdl" cwd="$cwd_kdl" {
    zelligent_path "$zelligent_path_cmd_kdl"
    agent_cmd "$raw_agent_cmd_kdl"
    repo_root "$cwd_kdl"
}
EOF
}

pane_name_for_agent_cmd() {
  # Derive a short, stable pane title from the user's agent command.
  # For shell-like invocations there's no useful command name to surface, so
  # fall back to the tab/session name when one is provided (keeps the pane
  # frame consistent with the sidebar and tab title) — or to literal "shell"
  # as a last resort.
  local agent_cmd="$1"
  local session_name="${2:-}"
  local first_word base

  shell_fallback() {
    if [ -n "$session_name" ]; then
      echo "$session_name"
    else
      echo "shell"
    fi
  }

  if [ -z "$agent_cmd" ]; then
    shell_fallback
    return
  fi

  # First whitespace-separated token, then strip a directory component.
  first_word="${agent_cmd%% *}"
  base="${first_word##*/}"

  case "$base" in
    "" | sh | bash | zsh | fish | dash | ksh | tcsh)
      shell_fallback
      ;;
    *)
      echo "$base"
      ;;
  esac
}

default_tab_children_content() {
  # Flat-siblings form. Used to substitute {{zelligent_children}} in the
  # sidebar layout fragment, where the outer wrapper is already
  # `pane split_direction="Vertical" { sidebar, ... }` — so emitting two
  # bare panes here makes them direct siblings of the sidebar pane and
  # inherit its left/right split, putting lazygit on the right of the
  # agent pane.
  #
  # Note: this form only works inside that outer Vertical wrapper.
  # `default_tab_body_content` is the form used inside a `tab { }` body
  # (session-layout mode), where panes need their own explicit wrapper.
  local cwd_value="$1"
  local agent_cmd_kdl="$2"
  local pane_name="$3"
  local cwd_kdl pane_name_kdl

  cwd_kdl=$(escape_kdl_string "$cwd_value")
  if [ -z "$pane_name" ]; then
    pane_name="shell"
  fi
  pane_name_kdl=$(escape_kdl_string "$pane_name")

  cat <<EOF
pane name="$pane_name_kdl" command="bash" cwd="$cwd_kdl" size="70%" {
    args "-lc" "$agent_cmd_kdl"
}
pane name="lazygit" command="lazygit" cwd="$cwd_kdl" size="30%"
EOF
}

default_tab_body_content() {
  # Wrapped form for use inside a `tab { }` block. zellij auto-wraps
  # multi-pane tab bodies with a horizontal-split wrapper (lazygit ends up
  # below the agent), so we emit our own vertical-split wrapper.
  local cwd_value="$1"
  local agent_cmd_kdl="$2"
  local pane_name="$3"
  local cwd_kdl pane_name_kdl

  cwd_kdl=$(escape_kdl_string "$cwd_value")
  if [ -z "$pane_name" ]; then
    pane_name="shell"
  fi
  pane_name_kdl=$(escape_kdl_string "$pane_name")

  cat <<EOF
pane split_direction="vertical" {
    pane name="$pane_name_kdl" command="bash" cwd="$cwd_kdl" size="70%" {
        args "-lc" "$agent_cmd_kdl"
    }
    pane name="lazygit" command="lazygit" cwd="$cwd_kdl" size="30%"
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
  local new_tab_fragment_path="$5"
  local tab_name_kdl

  tab_name_kdl=$(escape_kdl_string "$tab_name")

  {
    echo "layout {"
    echo "    default_tab_template {"
    sed 's/^/        /' "$default_fragment_path"
    echo "    }"
    # `default_tab_template`'s {{zelligent_children}} substitution ("children",
    # a bare keyword) is only filled in when Zellij merges an EXPLICIT tab body
    # into the template at layout-parse time — which is how the `tab { }`
    # block below gets its shell+lazygit panes. A tab created later via `zellij
    # action new-tab --name X` with no --layout has no explicit body to merge,
    # and Zellij's fallback fill for that case does not recurse into nested
    # panes to find the children marker, so it silently resolves to nothing —
    # leaving only the sidebar, full width, with no shell pane (issue #139).
    # `new_tab_template` is a distinct KDL node, parsed like a literal `tab { }`
    # (no children-marker merge at all), and Zellij prefers it over
    # `default_tab_template` specifically for that no-layout new-tab case. Give
    # it real, literal content so manual tabs get a usable shell pane too.
    if [ -n "$new_tab_fragment_path" ]; then
      echo "    new_tab_template {"
      sed 's/^/        /' "$new_tab_fragment_path"
      echo "    }"
    fi
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

  # Zellij can persist a *blocked* prompt as an entry with an empty body
  # (`"…wasm" { }`), which the previous grep-only check treated as already
  # configured and skipped. Rewrite a present-but-empty/incomplete block in
  # place, append a fresh block when no entry exists.
  if grep -qF "\"$PLUGIN_PATH\"" "$PERM_FILE"; then
    PLUGIN_PATH="$PLUGIN_PATH" perl -i -0pe '
      my $path = quotemeta($ENV{PLUGIN_PATH});
      my $block = qq{"$ENV{PLUGIN_PATH}" \{\n    ChangeApplicationState\n    ReadApplicationState\n    RunCommands\n    ReadCliPipes\n\}\n};
      if (/"$path"\s*\{([^}]*)\}/s) {
        my $body = $1;
        my $needs = $body !~ /ChangeApplicationState/
                 || $body !~ /ReadApplicationState/
                 || $body !~ /RunCommands/
                 || $body !~ /ReadCliPipes/;
        s/"$path"\s*\{[^}]*\}\s*\n?/$block/s if $needs;
      }
    ' "$PERM_FILE"
  else
    cat >> "$PERM_FILE" <<PERMS
"$PLUGIN_PATH" {
    ChangeApplicationState
    ReadApplicationState
    RunCommands
    ReadCliPipes
}
PERMS
  fi
  echo "  permissions: granted for $PLUGIN_PATH"

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
      # known_marketplaces.json is keyed by name, so a stale "zelligent"
      # entry (e.g. left over from a dev install after switching to
      # Homebrew, or vice versa) makes `marketplace add` fail on a name
      # collision — previously swallowed silently, leaving `plugin update`
      # reading a path that may no longer exist. Repair it by removing the
      # stale registration before re-adding. When the registered path
      # already matches, skip `marketplace add` entirely: re-adding an
      # identical registration is a redundant, and possibly erroring,
      # no-op we don't want to have to distinguish from a real failure.
      # `marketplace add` fails on a name collision whether the existing
      # registration points at THIS path (healthy, idempotent re-run) or a
      # stale one (e.g. an old dev install). Doctor deliberately does NOT
      # introspect or mutate Claude Code's registration files to tell the
      # difference — production code stays out of dev-environment hygiene
      # (that's `bash dev-install.sh --uninstall`). If the plugin then
      # installs/updates fine the user is served; if not, the fix is one
      # command, printed in the failure line.
      claude plugin marketplace add "$PLUGIN_MARKETPLACE" 2>/dev/null || true
      if claude plugin list 2>/dev/null | grep -qF 'zelligent@zelligent'; then
        if claude plugin update zelligent@zelligent 2>/dev/null; then
          echo "  claude plugin: updated"
          echo "  claude plugin: restart running Claude Code sessions to pick up hook changes"
        else
          echo "  claude plugin: ok (update check failed)"
        fi
      else
        if claude plugin install zelligent@zelligent 2>/dev/null; then
          echo "  claude plugin: installed"
          echo "  claude plugin: restart running Claude Code sessions to pick up hook changes"
        else
          echo "  claude plugin: failed to install — if a stale 'zelligent' marketplace is registered from an old install, run: claude plugin marketplace remove zelligent && zelligent doctor"
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

  # 8. Sweep serialized (resurrectable) sessions for stale plugin URLs
  # (#155/#157). Startup and spawn only reconcile the current repo's own
  # session; this is the only place ALL cached sessions get checked,
  # including other repos' and alive-but-stale ones (which the startup
  # guard deliberately never touches).
  echo ""
  echo "  Serialized sessions:"
  DOCTOR_SWEEP_COUNT=0
  while IFS= read -r DOCTOR_LAYOUT_FILE; do
    [ -n "$DOCTOR_LAYOUT_FILE" ] || continue
    DOCTOR_SWEEP_COUNT=$((DOCTOR_SWEEP_COUNT + 1))
    DOCTOR_SESSION_DIR=$(dirname "$DOCTOR_LAYOUT_FILE")
    DOCTOR_SESSION_NAME=$(basename "$DOCTOR_SESSION_DIR")
    DOCTOR_SESSION_STATE=$(session_state "$DOCTOR_SESSION_NAME")
    IFS=$'\t' read -r DOCTOR_STALE_KIND DOCTOR_BAD_PATH < <(layout_stale_kind "$DOCTOR_LAYOUT_FILE" "$PLUGIN_PATH")
    if [ "$DOCTOR_STALE_KIND" = "none" ]; then
      echo "    $DOCTOR_SESSION_NAME ($DOCTOR_SESSION_STATE): ok"
    elif [ "$DOCTOR_SESSION_STATE" = "exited" ] && [ "$DOCTOR_STALE_KIND" = "zelligent" ]; then
      # Auto-fixable: exited, and the zelligent sidebar's own URL is stale.
      drop_stale_session "$DOCTOR_SESSION_NAME" "$DOCTOR_BAD_PATH" | sed 's/^/    /'
    else
      # Warn-only: either the session is still alive (never delete it out
      # from under its user) or it's exited but only a third-party plugin's
      # URL is stale (not ours to fix on the user's behalf).
      echo "    $DOCTOR_SESSION_NAME ($DOCTOR_SESSION_STATE): stale — plugin path no longer valid: $DOCTOR_BAD_PATH"
      echo "        Fix: zellij delete-session --force '$DOCTOR_SESSION_NAME'"
    fi
  done < <(all_serialized_layout_files)
  if [ "$DOCTOR_SWEEP_COUNT" -eq 0 ]; then
    echo "    none found"
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

# Best-effort: tell every sidebar plugin instance in the repo's session that
# the worktree list just changed (spawn/remove). CLI pipes are the ONLY
# channel that reaches plugin instances in hidden tabs — Zellij Events
# (TabUpdate etc.) are only delivered to the visible tab, so without this
# pipe a hidden sidebar can keep a stale worktree row indefinitely. See
# issues #138/#140 and docs/references/zellij-plugin-api.md ("Event delivery
# and hidden panes"). Mirrors how the Claude Code status hooks invoke
# `zellij pipe`. Guarded so it can never fail the calling command.
#
# Fire-and-forget (#167): `zellij pipe` BLOCKS until a plugin consumes the
# message — up to zellij's ~1s CliPipe dispatch timeout with sidebars
# loaded, and indefinitely in a session with no zelligent plugin at all
# (measured: the pipe was the entire >1s of perceived spawn latency; the
# tab itself is ready in ~150ms). The caller never uses the result, so run
# it in the background under a hard timeout; delivery still happens
# milliseconds later, and the no-consumer case is bounded instead of
# wedging the CLI.
pipe_invalidate() {
  command -v zellij &>/dev/null || return 0
  if [ -n "$ZELLIJ" ]; then
    run_with_timeout 5 zellij pipe --name zelligent-invalidate >/dev/null 2>&1 &
  else
    run_with_timeout 5 zellij --session "$REPO_NAME" pipe --name zelligent-invalidate >/dev/null 2>&1 &
  fi
}

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
  #   macOS: ~/Library/Caches/org.Zellij-Contributors.Zellij/<version-or-contract-dir>/session_info/SESSION/
  #   Linux: ~/.cache/zellij/<version-or-contract-dir>/session_info/SESSION/
  # The version-dir NAME drifts across zellij releases — 0.43.1 used the bare
  # version string, 0.44.x uses `contract_version_N`, and it can drift again
  # on future releases. Hardcoding `$zellij_version` here silently no-ops on
  # any version that doesn't match (#158); glob for any dir that owns a
  # `session_info/<name>` entry instead.
  while IFS= read -r cache_dir; do
    [ -n "$cache_dir" ] && rm -rf "$cache_dir" 2>/dev/null || true
  done < <(serialized_session_dirs "$REPO_NAME")
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

  # Resolve the plugin path up front (best-effort) so the reconciliation
  # guard (#155/#157) can validate a resurrectable session's serialized
  # plugin URL BEFORE the existence probe below decides to `zellij attach`
  # into it. `zellij list-sessions --short` prints EXITED sessions
  # identically to alive ones, so this probe is itself the vulnerable path.
  # Tolerate resolve failure here — the guard still catches missing-file and
  # bad-magic staleness without a current path to compare against; the
  # "not installed" error below still fires from the real resolve attempt.
  PLUGIN_PATH_STARTUP=$(resolve_plugin_path 2>/dev/null || true)
  reconcile_serialized_session "$REPO_NAME" "$PLUGIN_PATH_STARTUP"

  if zellij_list_sessions | grep -qxF "$REPO_NAME"; then
    echo "Attaching to session '$REPO_NAME'..."
    exec zellij attach "$REPO_NAME"
  else
    if [ -z "$PLUGIN_PATH_STARTUP" ]; then
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
    RENDERED_STARTUP_NEW_TAB_TEMPLATE=$(mktemp "$ZELLIGENT_USER_DIR/tmp/layout-startup-new-tab-template-XXXXXX")
    # `zellij --new-session-with-layout` treats an extension-less argument as a
    # layout NAME (looked up against built-ins) rather than a path, and silently
    # falls back to the default built-in layout on miss. Force a `.kdl` suffix
    # so zellij parses the file as a path.
    STARTUP_LAYOUT_RAW=$(mktemp "$ZELLIGENT_USER_DIR/tmp/layout-startup-session-XXXXXX")
    STARTUP_LAYOUT="${STARTUP_LAYOUT_RAW}.kdl"
    mv "$STARTUP_LAYOUT_RAW" "$STARTUP_LAYOUT"
    trap 'rm -f "$RENDERED_STARTUP_TEMPLATE" "$RENDERED_STARTUP_CHILDREN" "$RENDERED_STARTUP_NEW_TAB_TEMPLATE" "$STARTUP_LAYOUT"' EXIT

    STARTUP_AGENT_CMD="$SHELL"
    STARTUP_AGENT_RENDER=$(build_agent_command_value "$STARTUP_AGENT_CMD" "$REPO_NAME" "$REPO_ROOT" "$REPO_ROOT" "" "false")
    STARTUP_SIDEBAR=$(sidebar_plugin_content "$PLUGIN_PATH_STARTUP" "$STARTUP_AGENT_CMD" "$REPO_ROOT")
    STARTUP_PANE_NAME=$(pane_name_for_agent_cmd "$STARTUP_AGENT_CMD" "$REPO_NAME")
    # Startup tab body: bare shell+lazygit go directly into `tab { … }`
    # without an outer Vertical wrapper, so use the wrapped body form.
    STARTUP_CHILDREN=$(default_tab_body_content "$REPO_ROOT" "$STARTUP_AGENT_RENDER" "$STARTUP_PANE_NAME")
    render_layout_fragment "$LAYOUT_SOURCE_STARTUP" "$RENDERED_STARTUP_TEMPLATE" "$REPO_ROOT" "$STARTUP_AGENT_RENDER" "$STARTUP_SIDEBAR" "children"
    printf '%s\n' "$STARTUP_CHILDREN" > "$RENDERED_STARTUP_CHILDREN"
    # `new_tab_template` content for manual tabs (`zellij action new-tab
    # --name X` with no --layout, see #139): no worktree/agent context exists
    # for a tab created later on, so fall back to a plain shell — same
    # wrapped body shape, just without the worktree cwd/agent command.
    STARTUP_MANUAL_AGENT_RENDER=$(escape_kdl_string "exec $STARTUP_AGENT_CMD")
    STARTUP_NEW_TAB_CHILDREN=$(default_tab_body_content "$REPO_ROOT" "$STARTUP_MANUAL_AGENT_RENDER" "shell")
    render_layout_fragment "$LAYOUT_SOURCE_STARTUP" "$RENDERED_STARTUP_NEW_TAB_TEMPLATE" "$REPO_ROOT" "$STARTUP_MANUAL_AGENT_RENDER" "$STARTUP_SIDEBAR" "$STARTUP_NEW_TAB_CHILDREN"
    write_session_layout "$STARTUP_LAYOUT" "$RENDERED_STARTUP_TEMPLATE" "$RENDERED_STARTUP_CHILDREN" "$REPO_NAME" "$RENDERED_STARTUP_NEW_TAB_TEMPLATE"

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
  shift
  # --plugin-driven is set ONLY by the sidebar plugin's `fire_remove`. It
  # tells us to skip the auto-close block below because the plugin will close
  # the worktree's tab itself via `Action::CloseTabAndRefresh` once we
  # return. An env var was tried first (see issue #121 review) but is too
  # easy for a user to leak into their shell, silently breaking manual
  # `zelligent remove`. A CLI flag has tighter blast radius.
  PLUGIN_DRIVEN=
  if [ "${1:-}" = "--plugin-driven" ]; then
    PLUGIN_DRIVEN=1
    shift
  fi
  if [ -z "${1:-}" ]; then
    echo "Usage: zelligent remove [--plugin-driven] <branch-name>"
    exit 1
  fi
  BRANCH_NAME=$1
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
  # Invalidate every sidebar instance's worktree cache — including hidden
  # ones, which only a pipe can reach. See issues #138/#140.
  pipe_invalidate
  # When running inside Zellij, also close the worktree's tab so the sidebar
  # plugin doesn't show an orphaned tab labeled "user tab" (the worktree is
  # gone but Zellij still holds the tab). Return the user to the tab they
  # came from after closing.
  if [ -n "$ZELLIJ" ] && command -v zellij &>/dev/null; then
    # When the plugin invoked us with `--plugin-driven`, it will close the
    # tab itself via `Action::CloseTabAndRefresh` after this CLI returns.
    # Doing it twice races: the CLI's `go-to-tab-name` + `close-tab` runs
    # first, the plugin's `close_focused_tab` then lands on whatever Zellij
    # focused next — potentially the user's main repo tab. Defer to the
    # plugin in that case. See issue #121.
    if [ -z "$PLUGIN_DRIVEN" ]; then
      # Capture the full remainder of the `name:` line — tab names can be
      # user-renamed (Ctrl-t r) to include `: `, in which case `-F': '` would
      # truncate at the second separator. Use `sed` to strip only the leading
      # `name: ` and keep everything else verbatim.
      ORIGIN_TAB=$(zellij action current-tab-info 2>/dev/null | sed -n '1{s/^name: //p;}')
      if zellij action go-to-tab-name "$SESSION_NAME" 2>/dev/null; then
        zellij action close-tab 2>/dev/null || true
        if [ -n "$ORIGIN_TAB" ] && [ "$ORIGIN_TAB" != "$SESSION_NAME" ]; then
          zellij action go-to-tab-name "$ORIGIN_TAB" 2>/dev/null || true
        fi
      fi
    fi
  else
    echo "ℹ️  Close the '$SESSION_NAME' tab manually if still open."
  fi
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

# When invoked outside a running Zellij, the spawn ultimately calls
# `zellij attach` or `zellij --new-session-with-layout`, both of which need a
# controlling terminal. Refuse early — before we create the worktree on disk
# — so a non-interactive caller doesn't leave orphan worktrees behind.
# `ZELLIGENT_SKIP_TTY_CHECK=1` is honored for test harnesses that stub zellij.
require_tty_or_die() {
  if [ -n "${ZELLIGENT_SKIP_TTY_CHECK:-}" ]; then
    return 0
  fi
  if [ ! -t 0 ] && [ ! -t 1 ]; then
    echo "Error: 'zelligent spawn' must run from a TTY when not already inside" >&2
    echo "       a Zellij session. Attach to the session first (e.g. 'zelligent')," >&2
    echo "       then run 'zelligent spawn $BRANCH_NAME' from inside it." >&2
    exit 1
  fi
}
if [ -z "$ZELLIJ" ]; then
  require_tty_or_die
fi

# Check zellij is available before creating any worktrees
if ! command -v zellij &>/dev/null; then
  echo "Error: zellij not found. Run 'zelligent doctor' to set up." >&2
  exit 1
fi

SESSION_NAME="${BRANCH_NAME//\//-}"
# Strip any characters outside the safe set for session/tab names
SESSION_NAME=$(printf '%s' "$SESSION_NAME" | tr -cd 'a-zA-Z0-9_-')

# Pick the base branch for the new worktree.
#
# Branch off the caller's CURRENT branch — that's typically what you want
# when you spawn from inside an existing worktree (continuing on top of work
# in progress). This works for both invocation paths:
#   - From a worktree's shell (typical CLI use): cwd is the worktree, so
#     HEAD points at that worktree's branch.
#   - From the persistent sidebar plugin: it runs the spawn command from
#     the main repo root, so HEAD points at the main branch — still a
#     sensible default.
#
# Fallbacks: detached HEAD or unresolvable HEAD → origin/HEAD's target →
# `main`.
BASE_BRANCH=""
if CURRENT_REF=$(git symbolic-ref --quiet --short HEAD 2>/dev/null); then
  BASE_BRANCH="$CURRENT_REF"
elif BASE_REF=$(git symbolic-ref refs/remotes/origin/HEAD 2>/dev/null); then
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
# `zellij --new-session-with-layout` treats an extension-less argument as a
# layout NAME and silently falls back to the built-in default on miss. Force a
# `.kdl` suffix so the path is parsed as a file. (`new-tab --layout` accepts
# either, but we keep both consistent.)
LAYOUT_RAW=$(mktemp "$ZELLIGENT_USER_DIR/tmp/layout-XXXXXX")
LAYOUT="${LAYOUT_RAW}.kdl"
mv "$LAYOUT_RAW" "$LAYOUT"
RENDERED_TAB_FRAGMENT=$(mktemp "$ZELLIGENT_USER_DIR/tmp/layout-tab-fragment-XXXXXX")
RENDERED_TAB_BODY=$(mktemp "$ZELLIGENT_USER_DIR/tmp/layout-tab-body-XXXXXX")
RENDERED_SESSION_TEMPLATE=$(mktemp "$ZELLIGENT_USER_DIR/tmp/layout-session-template-XXXXXX")
RENDERED_NEW_TAB_TEMPLATE=$(mktemp "$ZELLIGENT_USER_DIR/tmp/layout-new-tab-template-XXXXXX")
trap 'rm -f "$LAYOUT" "$RENDERED_TAB_FRAGMENT" "$RENDERED_TAB_BODY" "$RENDERED_SESSION_TEMPLATE" "$RENDERED_NEW_TAB_TEMPLATE"' EXIT

SETUP_SCRIPT="$REPO_ROOT/.zelligent/setup.sh"
AGENT_CMD_RENDER=$(build_agent_command_value "$AGENT_CMD" "$SESSION_NAME" "$REPO_ROOT" "$WORKTREE_PATH" "$SETUP_SCRIPT" "$NEW_WORKTREE")
SESSION_AGENT_RENDER=$(build_agent_command_value "$AGENT_CMD" "$REPO_NAME" "$REPO_ROOT" "$REPO_ROOT" "" "false")
SIDEBAR_RENDER=$(sidebar_plugin_content "$PLUGIN_PATH_LAYOUT" "$AGENT_CMD" "$REPO_ROOT")
TAB_PANE_NAME=$(pane_name_for_agent_cmd "$AGENT_CMD" "$SESSION_NAME")
TAB_CHILDREN_RENDER=$(default_tab_children_content "$WORKTREE_PATH" "$AGENT_CMD_RENDER" "$TAB_PANE_NAME")
render_layout_fragment "$LAYOUT_SOURCE" "$RENDERED_TAB_FRAGMENT" "$WORKTREE_PATH" "$AGENT_CMD_RENDER" "$SIDEBAR_RENDER" "$TAB_CHILDREN_RENDER"
render_layout_fragment "$LAYOUT_SOURCE" "$RENDERED_SESSION_TEMPLATE" "$REPO_ROOT" "$SESSION_AGENT_RENDER" "$SIDEBAR_RENDER" "children"
# Content-only body for the new-session mode's explicit `tab { }` block (issue
# #163): the session template already wraps every tab in the sidebar, so the
# tab body must carry ONLY the agent+lazygit panes — embedding the full
# sidebar-bearing RENDERED_TAB_FRAGMENT there gets merged INTO the template's
# children slot and renders a second, nested sidebar. Same wrapped-body form
# as the no-arg startup path uses for its initial tab.
printf '%s\n' "$(default_tab_body_content "$WORKTREE_PATH" "$AGENT_CMD_RENDER" "$TAB_PANE_NAME")" > "$RENDERED_TAB_BODY"
# `new_tab_template` content for manual tabs (`zellij action new-tab --name X`
# with no --layout, see #139): no worktree/agent context exists for a tab
# created later on, so fall back to a plain shell — same wrapped body shape
# default_tab_body_content produces for the session's own initial tab, just
# without the worktree cwd/agent command. See write_session_layout for why
# `default_tab_template`'s "children" alone isn't enough for these tabs.
MANUAL_AGENT_RENDER=$(escape_kdl_string "exec $SHELL")
NEW_TAB_CHILDREN_RENDER=$(default_tab_body_content "$REPO_ROOT" "$MANUAL_AGENT_RENDER" "shell")
render_layout_fragment "$LAYOUT_SOURCE" "$RENDERED_NEW_TAB_TEMPLATE" "$REPO_ROOT" "$MANUAL_AGENT_RENDER" "$SIDEBAR_RENDER" "$NEW_TAB_CHILDREN_RENDER"

# Decide spawn mode ONCE. We used to call `zellij_list_sessions` twice — once
# to choose the layout shape, once to choose the launch command — and a
# session that exited between those probes (or a stale-socket recovery)
# could feed a fragment to `--new-session-with-layout` or a full session
# layout to `new-tab --layout`. That race lands AFTER `git worktree add` has
# already mutated disk, so the user is left with an orphan worktree and a
# malformed tab. Cache the decision and use it for both the layout writer
# and the launch command.
#
# Modes:
#   inside-zellij       — already attached, use `action new-tab` with fragment
#   attach-session      — outside zellij, repo session exists: `action new-tab` then `attach`
#   new-session         — outside zellij, no session: `--new-session-with-layout`
if [ -n "$ZELLIJ" ]; then
  SPAWN_MODE="inside-zellij"
else
  # Same guard as the no-arg startup path (#155/#157): this probe is the
  # other flow that can walk into resurrecting a broken EXITED session
  # (`attach-session` mode below calls `zellij attach "$REPO_NAME"`).
  # PLUGIN_PATH_LAYOUT is already resolved above (line ~1270).
  reconcile_serialized_session "$REPO_NAME" "$PLUGIN_PATH_LAYOUT"
  if zellij_list_sessions | grep -qxF "$REPO_NAME"; then
    SPAWN_MODE="attach-session"
  else
    SPAWN_MODE="new-session"
  fi
fi

# Inside Zellij and attach-session both want a fragment layout (panes at
# root) for `new-tab --layout`. Only new-session wants the full session
# layout for `--new-session-with-layout`.
case "$SPAWN_MODE" in
  inside-zellij | attach-session)
    write_fragment_layout "$LAYOUT" "$RENDERED_TAB_FRAGMENT"
    ;;
  new-session)
    write_session_layout "$LAYOUT" "$RENDERED_SESSION_TEMPLATE" "$RENDERED_TAB_BODY" "$SESSION_NAME" "$RENDERED_NEW_TAB_TEMPLATE"
    ;;
esac

case "$SPAWN_MODE" in
  inside-zellij)
    echo "🪟 Opening tab '$SESSION_NAME'..."
    zellij action new-tab --layout "$LAYOUT" --name "$SESSION_NAME"
    # Invalidate every sidebar instance's worktree cache — including hidden
    # ones, which only a pipe can reach. See issues #138/#140.
    pipe_invalidate
    ;;
  attach-session)
    echo "🪟 Attaching to session '$REPO_NAME', opening tab '$SESSION_NAME'..."
    ZELLIJ_SESSION_NAME="$REPO_NAME" zellij action new-tab --layout "$LAYOUT" --name "$SESSION_NAME"
    # Fire before the blocking `attach` below, so existing instances heal
    # even if the user later detaches without interacting. See #138/#140.
    pipe_invalidate
    zellij attach "$REPO_NAME"
    ;;
  new-session)
    # No pipe_invalidate: the session is brand new, so every plugin
    # instance in it bootstraps a fresh worktree list anyway.
    echo "🪟 Creating Zellij session '$REPO_NAME'..."
    zellij --new-session-with-layout "$LAYOUT" --session "$REPO_NAME"
    ;;
esac
