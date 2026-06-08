# Recording demo videos

How the `demo.mp4` / `demo.gif` shipped at the repo root were produced.
Reconstructed from the session that actually recorded them
(2026-05-27 / 2026-05-28).

The recipe is **scripted, not interactive** — there is no mouse, no
QuickTime, no manual key-pressing. A shell script drives a real Zellij
session from outside via tmux while `asciinema` captures the inner
session's terminal output, then `agg` and `ffmpeg` convert the cast to
GIF and MP4.

## Tools

```bash
brew install asciinema agg tmux ffmpeg
```

- `asciinema` (3.2+) — records terminal output to a `.cast` file. No
  mouse, no chrome, no host font dependency.
- `agg` (1.8+) — renders an asciicast to a GIF. Theme- and font-aware.
- `tmux` — used twice, nested. See the architecture note below.
- `ffmpeg` — final GIF → MP4 step.

## The double-tmux architecture (this is the load-bearing trick)

`asciinema rec` needs a foreground pty to record. If you launch it
inline, the very terminal you're driving from is the one being
captured — you can't send keystrokes from outside. Workaround:

- **`demo-rec`** — inner tmux session, 220×60, status bar off. This is
  the shell where `zelligent` runs and where Claude agents live. The
  *contents* of this session are what gets recorded.
- **`rec-host`** — outer tmux session, same size. It runs
  `asciinema rec --overwrite -c 'tmux attach -t demo-rec' …`. We never
  view `rec-host`; it exists so asciinema gets a pty without us holding
  the foreground.

Drive the demo by sending keys to `demo-rec` from the host shell:

```bash
tmux send-keys -t demo-rec -l "zelligent spawn agent/foo 'claude \"…\"'"
sleep 1.8
tmux send-keys -t demo-rec Enter
```

Stop the recording by killing `demo-rec`. That makes the attached
client (inside `rec-host`) exit, which makes `asciinema` flush the cast
file. Killing `rec-host` directly leaves a half-written cast — go
through `demo-rec` first.

## The actual recording script

The script that produced the shipped clips lived at
`/tmp/record-zelligent-demo.sh`. It's not committed; it was per-take.
Skeleton:

```bash
#!/bin/bash
set -u

CAST=/tmp/zelligent-demo.cast
GIF=/Users/philipp/code/zelligent/demo.gif
MP4=/Users/philipp/code/zelligent/demo.mp4
REPO=/tmp/httpie-demo
COLS=220
ROWS=60

# Clean slate
tmux kill-session -t demo-rec 2>/dev/null
tmux kill-session -t rec-host 2>/dev/null
zellij delete-session httpie-demo --force 2>/dev/null
rm -f "$CAST"

# Inner: the session being recorded
tmux new -d -s demo-rec -x "$COLS" -y "$ROWS" -c "$REPO"
tmux set-option -t demo-rec status off
sleep 0.5

# Outer: asciinema wrapping `tmux attach -t demo-rec`
tmux new -d -s rec-host -x "$COLS" -y "$ROWS" \
  "asciinema rec --overwrite -c 'tmux attach -t demo-rec' $CAST"
tmux set-option -t rec-host status off
sleep 2

# Helpers
type_line() {
  tmux send-keys -t demo-rec -l "$1"
  sleep "${2:-1.8}"
  tmux send-keys -t demo-rec Enter
}
tab() {                          # zellij tab leader + number
  tmux send-keys -t demo-rec C-t
  sleep 0.3
  tmux send-keys -t demo-rec "$1"
  sleep "${2:-4}"
}

# --- Demo beats ----------------------------------------------------------

sleep 3                          # cold open: prompt visible for a beat

type_line "zelligent" 2
sleep 9                          # layout boots, sidebar + lazygit render

# Spawn agent A — real Claude, --dangerously-skip-permissions so the
# script doesn't have to handle the trust prompt
SPAWN_A='zelligent spawn agent/fix-auth-decode '"'"'claude --dangerously-skip-permissions "Fix httpie issue 1623: …"'"'"
type_line "$SPAWN_A" 3
sleep 36                         # tab opens, Claude reads + produces output

tab 1 5                          # back to main shell
type_line "vim httpie/cli/argparser.py +282" 2
sleep 10                         # let viewer see the TODO + bug site
tmux send-keys -t demo-rec Escape; sleep 0.3
tmux send-keys -t demo-rec ":q" Enter
sleep 3

# Spawn agent B
SPAWN_B='zelligent spawn agent/refactor-process-auth '"'"'claude --dangerously-skip-permissions "Refactor …"'"'"
type_line "$SPAWN_B" 3
sleep 31

tab 1 6                          # parallelism reveal
tab 2 7
tab 3 7
tab 1 6                          # rest on the sidebar

# --- Stop + convert ------------------------------------------------------
tmux kill-session -t demo-rec
sleep 2
tmux kill-session -t rec-host 2>/dev/null

agg --idle-time-limit 1.5 --theme dracula --font-size 14 "$CAST" "$GIF"

ffmpeg -y -i "$GIF" \
  -movflags faststart -pix_fmt yuv420p \
  -vf "scale=1920:-2:flags=lanczos" "$MP4"

zellij delete-session httpie-demo --force 2>/dev/null
```

## Pacing rules that mattered

- **Human eyes need 2–3s minimum** to register a state change, longer
  on busy frames. Most `sleep` values in the script are 2–5s for short
  actions and 7–36s when Claude is doing real work.
- `agg --idle-time-limit 1.5` collapses any idle gap longer than 1.5s
  down to 1.5s. Without this the cast is mostly empty time during
  Claude's thinking. With this, the final GIF reads as continuous
  motion.
- Use `tmux send-keys -l` (literal mode) for the spawn commands.
  Without `-l`, escape sequences in the prompt string get interpreted
  as tmux key syntax.

## Pre-record checklist

1. `cd` to a fresh clone of the target repo (the shipped demo uses
   `/tmp/httpie-demo` — `git clone https://github.com/httpie/cli.git
   /tmp/httpie-demo`).
2. `zellij delete-session httpie-demo --force` to ensure no leftover
   serialized layout interferes.
3. `which asciinema agg tmux ffmpeg` — all four present.
4. Smoke test the inner/outer tmux pattern with a trivial command
   sequence before doing the full ~4-minute take.
5. Decide on `--dangerously-skip-permissions` for Claude. Without it,
   the trust prompt blocks the script and you have to handle it
   interactively — defeats the point.

## Why not QuickTime / vhs

- **QuickTime**: works, but captures the entire window including font
  rendering, mouse cursor, focus rings — and it's not scriptable, so
  every retake means manually nailing the timing of every keystroke.
- **vhs (charmbracelet)**: nice for short tape-defined demos. Doesn't
  fit here because the demo's whole point is showing *real* Claude
  agents running in parallel, not synthesized terminal output. vhs
  also doesn't drive a host tmux/zellij session well.

The double-tmux + asciinema + agg approach is the only one that gets
all three of: scriptable retakes, real Claude agents in real Zellij,
and clean output without mouse/chrome.

## Where the artifacts live

`DEMO_SCRIPT.md` at the repo root is the human-readable narration
script and pre-record checklist for the currently-being-recorded
demo. Kept untracked because it changes per take.

`demo*.mp4` and `demo*.gif` at the repo root are the rendered
artifacts. Don't commit them; they're large binaries that change
every recording. The `.gitignore` doesn't exclude them by name
currently — add them if you want belt-and-braces protection.
