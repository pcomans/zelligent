# Recording demo videos

How the `demo.mp4` / `demo.gif` shipped at the repo root were produced, with the
exact tooling and rough recipes. The script that drives the recording itself
lives in a local `DEMO_SCRIPT.md` at the repo root (kept per-recording, not
checked in) — this doc covers the
post-production side only.

## Tools we actually used

- **macOS QuickTime Player** (`⌘⇧5` → "Record Selected Portion") — primary
  screen recording with mic audio. Produces the 1734×1080 30fps H.264 + AAC
  `demo.mp4`. Pick the Zellij window region; uncheck "Show Floating Thumbnail";
  set the mic input. Output lands on the desktop.
- **`ffmpeg`** (`brew install ffmpeg`) — every cut, mute, crop, scale, and
  encode below.
- **`agg`** (`brew install agg`) — only used if you go down the
  asciinema route (see "Alternate: asciinema → mp4" below).

You don't need `vhs` / `terminalizer` for the current demos. The 100fps
`demo-silent.mp4` in the tree was an asciinema export experiment from a
different run; it isn't the source of the shipped clips.

## Pipeline for the shipped clips

### 1. Record

QuickTime, region capture over the Zellij window. Follow the beats in
your local `DEMO_SCRIPT.md`. Stop on `⌘⌃Esc`. Save as `demo-raw.mov`.

### 2. Trim + mute (if needed)

```bash
# Trim to the on-script range, drop audio (for the silent variant)
ffmpeg -ss 00:00:02 -to 00:02:18 -i demo-raw.mov \
  -an -c:v copy demo-silent.mp4

# Keep audio
ffmpeg -ss 00:00:02 -to 00:02:18 -i demo-raw.mov \
  -c copy demo.mp4
```

`-c copy` preserves the original codec (no re-encode). For frame-accurate
trims drop `-c copy` and let ffmpeg re-encode.

### 3. Make the GIF

The shipped `demo.gif` is a 8-second 900×561 10fps clip — a short loop, not
the whole video. Two-pass palette flow gives much smaller files than a
naive single-pass:

```bash
# Pass 1 — extract palette from the chosen window
ffmpeg -ss 00:00:35 -t 8 -i demo.mp4 \
  -vf "fps=10,scale=900:-1:flags=lanczos,palettegen=stats_mode=diff" \
  -y palette.png

# Pass 2 — apply the palette
ffmpeg -ss 00:00:35 -t 8 -i demo.mp4 -i palette.png \
  -lavfi "fps=10,scale=900:-1:flags=lanczos[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle" \
  -y demo.gif
```

Tune knobs:

- `fps=10` — drop to 8 if file size is still too big for GitHub's ~10MB
  inline-render cap; 15 if motion feels choppy.
- `scale=900:-1` — match the README column. Bigger = readable text, bigger
  file. Below 720 the terminal text gets hard to read.
- `bayer_scale` — 5 is the dithering sweet spot for terminal output; lower
  values produce more banding, higher values produce file-size bloat.

### 4. Sanity check

```bash
ffprobe -v error -show_entries format=duration,size:stream=width,height,r_frame_rate,codec_name demo.gif
du -h demo.mp4 demo.gif
```

GitHub inline-renders GIFs up to ~10MB; the shipped `demo.gif` lands at ~1MB.
For longer or higher-fidelity demos prefer uploading the MP4 as a GitHub
attachment instead — drag it into a PR or issue body and GitHub plays it
inline.

## Alternate: asciinema → mp4 (the `demo-silent.mp4` route)

If you'd rather have a deterministic, scriptable terminal recording with no
window chrome:

```bash
brew install asciinema agg

# Record (Ctrl-D to stop)
asciinema rec demo.cast

# Render to gif (small)
agg --speed 1.0 --font-size 14 demo.cast demo.gif

# Or to mp4 via ffmpeg (gif → mp4)
ffmpeg -i demo.gif -movflags faststart -pix_fmt yuv420p \
  -vf "scale=trunc(iw/2)*2:trunc(ih/2)*2" demo.mp4
```

Trade-offs:

- Pros: no mouse, no window borders, font-controlled, replays at exact
  timing, can be re-rendered after the recording (font, speed, theme).
- Cons: can't capture the Zellij sidebar transitions that live above the
  pane content reliably — asciinema only sees what the underlying terminal
  emulator sees, so any cursor positioning issues in your terminal show up
  in the recording too. The QuickTime route is more faithful to "what the
  user actually sees".

## Where the artifacts live

`DEMO_SCRIPT.md` at the repo root holds the narration script and pre-record
checklist for the currently-being-recorded demo. It's kept untracked because
it changes with every recording session — copy the previous one if you want
a starting point.

The `demo*.mp4` and `demo*.gif` files at the repo root are **rendered
artifacts** — they're outputs of the pipeline above. Don't commit them;
they're large binaries and they change every recording session. The
`.gitignore` doesn't currently exclude them by name; if you want
belt-and-braces protection add a line like `demo*.mp4` and `demo*.gif`.
