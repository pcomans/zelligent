---
name: tmux
description: Drive tmux sessions, windows, panes, and commands directly via the tmux CLI.
allowed-tools: Bash
---

Use `tmux` CLI commands directly.

## Sessions

```bash
# create-session  (detached, optional start-directory and name)
tmux new-session -d -s <name> [-c <start-dir>]

# kill-session
tmux kill-session -t <name>

# list-sessions  (machine-readable)
tmux list-sessions -F '#{session_name} #{session_windows} #{session_created}'

# rename-session
tmux rename-session -t <old> <new>

# get current session (from inside a tmux env)
echo "$TMUX_PANE"   # pane id
tmux display-message -p '#S'  # session name
```

## Windows

```bash
# list-windows for a session
tmux list-windows -t <session> -F '#{window_index} #{window_name} #{window_active}'

# create-window
tmux new-window -t <session> -n <name> [-c <start-dir>]

# select-window
tmux select-window -t <session>:<index-or-name>

# rename-window
tmux rename-window -t <session>:<index> <new-name>

# kill-window
tmux kill-window -t <session>:<index>
```

## Panes

```bash
# list-panes for a session (all windows)
tmux list-panes -s -t <session> -F '#{pane_id} #{window_index} #{pane_index} #{pane_active} #{pane_current_command}'

# list-panes for a specific window
tmux list-panes -t <session>:<window> -F '#{pane_id} #{pane_index} #{pane_active}'

# split-pane  (-h = horizontal split i.e. side by side, -v = vertical i.e. stacked)
tmux split-window -t <pane-id> [-h|-v] [-c <start-dir>]

# select-pane
tmux select-pane -t <pane-id>

# kill-pane
tmux kill-pane -t <pane-id>

# resize-pane  (by N cells)
tmux resize-pane -t <pane-id> [-U|-D|-L|-R] <N>

# zoom pane (toggle)
tmux resize-pane -t <pane-id> -Z
```

Pane IDs look like `%0`, `%1`, etc. Target syntax: `session:window.pane` or bare pane id `%N`.

## Sending keys

```bash
# send-keys (raw text, no Enter)
tmux send-keys -t <pane-id> '<text>'

# send-keys + Enter  (most common: run a shell command)
tmux send-keys -t <pane-id> '<command>' Enter

# send-enter
tmux send-keys -t <pane-id> '' Enter

# send-escape
tmux send-keys -t <pane-id> '' Escape

# send-keys literal (no special key processing)
tmux send-keys -t <pane-id> -l '<text>'

# special keys: Tab, BSpace, Up, Down, Left, Right, PPage, NPage, Home, End, DC (Delete), F1-F12
tmux send-keys -t <pane-id> '' Tab
tmux send-keys -t <pane-id> '' Up
# etc.

# send Ctrl+C (cancel / SIGINT)
tmux send-keys -t <pane-id> '' C-c
```

## Capturing pane output

```bash
# capture-pane  (print to stdout, last N lines)
tmux capture-pane -t <pane-id> -p [-S -<N>]

# capture with history (e.g. last 500 lines)
tmux capture-pane -t <pane-id> -p -S -500

# capture with ANSI color codes preserved — required when reading TUI state
# (e.g. to detect which item is selected/highlighted via escape sequences like \e[7m)
tmux capture-pane -t <pane-id> -p -e -J
```

> Use `-e -J` whenever you need to verify visual state (selections, focus, colors). Plain `-p` strips all ANSI codes, making it impossible to distinguish selected from unselected items.

## Execute a command and reliably capture its exit code

Wrap commands in unique markers to extract output and exit code from captured pane content:

```bash
# 1. Generate a unique command ID
CMD_ID="tmux_$(date +%s%N | sha256sum | head -c 8)"

# 2. Send the wrapped command
tmux send-keys -t <pane-id> \
  "echo TMUX_START_${CMD_ID}; <your-command>; echo TMUX_DONE_${CMD_ID}_\$?" \
  Enter

# 3. Wait for completion, then capture and parse
sleep <estimated-duration>   # or poll (see below)
OUTPUT=$(tmux capture-pane -t <pane-id> -p -S -2000)

# 4. Extract output between markers
RESULT=$(echo "$OUTPUT" \
  | awk "/TMUX_START_${CMD_ID}/{found=1; next} /TMUX_DONE_${CMD_ID}_/{print; found=0; exit} found")
EXIT_CODE=$(echo "$OUTPUT" | grep -oP "TMUX_DONE_${CMD_ID}_\K[0-9]+")
```

### Polling until done

```bash
for i in $(seq 1 30); do
  OUTPUT=$(tmux capture-pane -t <pane-id> -p -S -2000)
  if echo "$OUTPUT" | grep -q "TMUX_DONE_${CMD_ID}_"; then
    EXIT_CODE=$(echo "$OUTPUT" | grep -oP "TMUX_DONE_${CMD_ID}_\K[0-9]+")
    RESULT=$(echo "$OUTPUT" \
      | awk "/TMUX_START_${CMD_ID}/{found=1; next} /TMUX_DONE_${CMD_ID}_/{found=0; exit} found")
    break
  fi
  sleep 1
done
```

## Simulating mouse clicks

tmux passes SGR mouse escape sequences directly to pane applications:

```
ESC[<Pb;Px;PyM   = button press   (Pb=button, Px=col, Py=row, 1-indexed)
ESC[<Pb;Px;Pym   = button release
```
Button codes: `0`=left, `1`=middle, `2`=right, `64`=wheel-up, `65`=wheel-down

```bash
# Enable mouse mode so the pane application receives mouse events
tmux set-option -t <session> mouse on

# Left-click at column 10, row 5 (press + release)
tmux send-keys -t <pane-id> $'\x1b[<0;10;5M'$'\x1b[<0;10;5m'

# Right-click at col 20, row 3
tmux send-keys -t <pane-id> $'\x1b[<2;20;3M'$'\x1b[<2;20;3m'

# Wheel-down / wheel-up at col 1, row 1
tmux send-keys -t <pane-id> $'\x1b[<65;1;1M'
tmux send-keys -t <pane-id> $'\x1b[<64;1;1M'
```

### Helper function

```bash
# tmux_click <pane-id> <col> <row> [button=0]
tmux_click() {
  local pane=$1 col=$2 row=$3 btn=${4:-0}
  tmux set-option -t "$pane" mouse on 2>/dev/null || true
  tmux send-keys -t "$pane" $'\x1b'"[<${btn};${col};${row}M"$'\x1b'"[<${btn};${col};${row}m"
}
# tmux_click %1 10 5      # left-click at col 10, row 5
# tmux_click %1 10 5 2    # right-click
```

> Mouse events only reach applications that have enabled mouse reporting (e.g. vim, less, ncurses TUIs, Zellij plugin panes). If the application hasn't enabled mouse mode, use keyboard navigation instead.

> **Coordinates are terminal-absolute.** For split-pane TUIs (e.g. Zellij with a sidebar + main pane), pass the full terminal row/col — the application translates internally. Do not subtract anything for pane borders or pane position. Row 1 = top of terminal, col 1 = left edge. The pane border itself occupies row 1 (and col 1) of the pane's screen area; the first clickable content row is row 2.

## Working with isolated sockets

```bash
# All commands accept -L <socket-name> or -S <socket-path>
tmux -L agent-socket new-session -d -s main
tmux -L agent-socket send-keys -t main 'echo hello' Enter
tmux -L agent-socket capture-pane -t main -p
tmux -L agent-socket kill-server
```

## Common patterns

**Spawn a shell, run a command, capture result:**
```bash
tmux new-session -d -s work -c /tmp
CMD_ID="job_$(date +%s)"
tmux send-keys -t work "echo START_${CMD_ID}; ls -la; echo DONE_${CMD_ID}_\$?" Enter
sleep 0.5
tmux capture-pane -t work -p -S -100
```

**Check if a session exists:**
```bash
tmux has-session -t <name> 2>/dev/null && echo "exists" || echo "missing"
```

**Wait for a pane's command to become shell (idle):**
```bash
while [[ "$(tmux display-message -t <pane-id> -p '#{pane_current_command}')" != "zsh" ]]; do
  sleep 0.2
done
```
