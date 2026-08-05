# UI Test Harness

Agent-driven UI tests for zelligent using a tmux-based harness and the tmux
skill.

For PR 83 and later persistent-sidebar work, the harness is for visible,
end-to-end behavior:

- startup through `zelligent`
- sidebar-visible session layout
- spawned tab layout
- manual-tab sidebar inheritance

Contract and failure-path coverage such as layout precedence, placeholder
validation, and `doctor` behavior remains in `bash test.sh`.

## Structure

```
tests/harness/
├── plans/          # Test plan markdown files
│   ├── alt-z-focus-keybinding.md
│   ├── empty-repo.md
│   ├── sidebar-mouse-interaction.md
│   ├── sidebar-layout-smoke.md
│   ├── with-worktrees.md
│   └── ui-audit-01..06-*.md   # exhaustive mouse/tab audit suite (2026-07)
├── fixtures/       # Setup scripts (one per scenario)
│   ├── setup-empty-repo.sh
│   ├── setup-with-worktrees.sh
│   ├── setup-many-worktrees.sh
│   └── teardown.sh
└── README.md
```

## How it works

1. Each **test plan** is a markdown file with test steps and frontmatter:
   - `fixture`: setup script
   - `launch`: command to start in the view window, usually `./zelligent.sh`
   - `session_name`: Zellij session name to target from the control window
2. Each **fixture** is a shell script that creates the test repo state
3. The **test-driver** subagent (`.claude/agents/test-driver.md`) reads the plan, runs the fixture, wraps Zellij in a tmux session, executes all test steps by reading `capture-pane` output, and reports results
4. The **tmux skill** (`.claude/skills/tmux/SKILL.md`) is available for manual proofs, inspection, and ad hoc UI interaction outside the automated test-driver flow

### tmux harness architecture

```
tmux session: zt-driver  (isolated socket: zt-driver-test)
├── window 0 "view"  — runs the plan's `launch` command, usually `zelligent`
└── window 1 "ctrl"  — runs shell commands to drive the test
```

## Running a test plan

Ask Claude to run a specific test plan:

```
Run the test plan at tests/harness/plans/with-worktrees.md
```

Claude delegates to the `test-driver` subagent, which handles everything autonomously.

**Important:** Test plans must run sequentially, not in parallel. They share the
same tmux socket (`zt-driver-test`) and test repo path
(`/tmp/zelligent-test-repo`), and each plan controls its own Zellij session, so
concurrent runs will conflict.

Prerequisites:

- `zellij` and `tmux` installed locally
- the build under test installed as the `zelligent` on PATH (CLI + plugin),
  normally via `bash dev-install.sh` from the branch under test — plans launch
  the installed `zelligent`, never the fixture clone's `./zelligent.sh` (see
  the CLI-under-test rule below)

Current plans:

- `sidebar-layout-smoke.md`: PR 83 smoke test for startup, spawned tabs, and
  manual-tab inheritance
- `empty-repo.md`: empty-state sidebar startup in a repo with no worktrees
- `sidebar-mouse-interaction.md`: wheel navigation and click-to-select/open
  behavior in the persistent sidebar
- `with-worktrees.md`: embedded sidebar stability in a repo with seeded
  worktrees
- `ui-audit-01-mouse-core.md` … `ui-audit-05-agent-status-modes.md`: exhaustive
  real-input audit of mouse selection/activation, multi-tab switching, scrolled
  viewports, worktree lifecycle staleness, and agent-status glyphs. Written for
  the 2026-07 UI audit (see `docs/reports/ui-audit-2026-07-05.md` if present);
  each plan carries a "Harness corrections" section with hard-won tmux/SGR
  driving rules — read it before running.
- `ui-audit-06-repro-verification.md`: minimal from-clean-fixture repros for
  every bug found in the audit (Z-1 through Z-8). Use as a regression suite
  when fixing those bugs.
- `session-resurrection.md`: serialized-session lifecycle — clean resurrection
  after a hard server kill, the stale-plugin-path footgun (#155/#157), the
  spawn-flow guard, and nuke recovery (#158).
- `refresh-failure-staleness.md`: reworked refresh lifecycle (#216/#219) — a
  failed `list-worktrees` keeps the last known list, raises a persistent
  `stale · retrying` marker that outlives the 8s status TTL, recovers the full
  error via `e`, does not spin, and clears on a timer-driven or manual retry.
  Simulates failure by moving the installed `zelligent` CLI aside (restore it
  if the run aborts mid-plan).

**CLI-under-test rule:** fixtures clone the source repo's checked-out branch
(usually `main`) into `/tmp/zelligent-test-repo`, so `./zelligent.sh` inside the
test repo is the OLD CLI. Plugin changes are injected via
`ZELLIGENT_PLUGIN_SRC`, but to test CLI changes you must invoke the installed
`zelligent` (from `bash dev-install.sh` of the branch under test) — verify with
`command -v zelligent` plus a `grep` for the change. A prior #140 verification
produced a false FAILED verdict by running the fixture clone's script.

## Driving rules (the playbook)

Hard-won across the 2026-07 audit, fix verification, integration, and release
qualification runs (~30 driver sessions). Every rule below exists because its
absence produced a wasted run, a false verdict, or a wedged environment.

### Identity first — prove WHAT you are testing

- **Verify the build before the first test step.** The sidebar footer shows the
  plugin version (`0.2.X+<sha>` for dev-install, `0.0.0-dev+<sha>` for a plain
  cargo build); `zelligent --version` shows the CLI stamp. Both must match the
  build under test, or STOP — a wrong-artifact run produces confident garbage.
- **CLI-under-test rule** (burned twice): fixtures clone the source repo's
  checked-out branch, so `./zelligent.sh` inside the test repo is the OLD CLI.
  Always launch the installed `zelligent`.
- **Build-identity integrity**: `plugin/build.rs` bakes `git rev-parse HEAD`
  with NO dirty marker, so a build of uncommitted changes wears its parent's
  sha. Verification artifacts must be built from committed trees; a plain
  `cargo build` footer (`0.0.0-dev+<sha>`) cannot be produced by dev-install
  and is therefore un-fakeable. Cross-check wasm byte size when in doubt.

### Mouse and keyboard input

- `tmux set-option -g mouse on` on the harness socket before any click; send
  SGR press and release as SEPARATE `send-keys` calls.
- Take a FRESH capture and locate the target row before every click — never
  trust coordinates written in a plan.
- **The focus-claim click**: a sidebar pane that is not click-focused eats
  exactly one click (Zellij's click-to-focus), with zero state change. This
  recurs after EVERY cross-tab landing, not just at startup. Count clicks from
  the first one the plugin receives.
- **Keyboard focus follows the new tab's main pane** after a click-driven
  spawn/switch. Re-click the sidebar once before sending it keys, or the keys
  land in the shell (a literal `dy` in a worktree prompt was the incident).

### Reading state

- There is no tab bar. Verify the active tab via the main pane's frame title,
  the sidebar's bold-cyan row, and `zellij action query-tab-names` from ctrl.
- **Two-axis cursor contract**: `▌` (spanning BOTH lines of the selected item)
  is the browsing cursor and follows the active tab on reveal/switches;
  bold-cyan marks the active tab and must ALWAYS be correct. Judge the axes
  separately.
- Capture plain (`capture-pane -p`) AND ANSI (`-p -e`) per step; glyph and
  color assertions need the ANSI bytes.
- **Zellij's alt-screen wipes tmux scrollback** — CLI stdout printed before an
  attach (e.g. guard messages) is unrecoverable from capture-pane. Pipe the
  CLI through `tee` to a log when its stdout is evidence.
- Status messages self-clear after ~8s (#152). Timing-sensitive checks must
  timestamp captures (`date +%s.%N`) and batch the whole sequence in one shell
  call; tool-call overhead alone can exceed the TTL.
- For structural claims (pane trees, duplicate panes), capture
  `zellij action dump-layout` — screen captures can hide nesting.

### What must NEVER run

- **Never `pkill zellij`** — it kills the driver's own shell. Use
  `zellij kill-session` / `zellij delete-session --force`. A hard
  `kill -9 <server pid>` is legitimate ONLY when a plan prescribes simulating
  a crash (leaves EXITED serialized state for resurrection tests).
- **Never run `zelligent spawn` from the ctrl window.** Outside Zellij it ends
  in `exec zellij attach`, turning ctrl into a second mirrored client whose
  keystrokes leak into the live session's focused pane. Spawn via the sidebar
  UI (`i` flow or clicks). Pipes (`zellij pipe`) and `zelligent remove` are
  non-attaching and safe from ctrl.
- **Never run `bash test.sh` concurrently with a harness driver** (or two
  drivers in parallel). Both create real Zellij sessions and fixtures; the
  collisions produce hangs and phantom failures (two incidents).

### Hygiene and evidence

- One Bash call per plan step: action + sleep + capture batched. Drivers have
  a hard turn budget (75); per-keystroke calls exhaust it mid-plan.
- ~8s wait after spawns/removes, ~1s after input, before capturing.
- Archive every capture under `/tmp/zelligent-ui-run/<run-name>/` with
  step-named files; quote evidence verbatim in reports.
- Diff `zellij.log` per phase (count `magic header` / panic lines before and
  after) when testing lifecycle/resurrection paths.
- Tear down completely: kill/delete every test session, `tmux kill-server` on
  the harness socket, run `fixtures/teardown.sh`. An EXITED serialized session
  left behind can resurrect into a later run.
- Known environmental noise (not product findings): `lazygit` missing in
  containers (`Command not found` pane), zellij's "non-fatal" pty-resize log
  lines, and devcontainer login banners in fresh shells.
- `Action CliPipe did not complete within 1s timeout` is NOT noise — it
  means some process synchronously waited ~1s on a `zellij pipe` call
  (#167's root cause hid behind exactly this line for weeks, misfiled as
  environmental). Around hard server kills it's expected; anywhere else,
  find the caller and background it.

## Writing a new test plan

1. Create a fixture script in `fixtures/` if you need a new repo state
2. Create a plan in `plans/` with frontmatter pointing to the fixture:

```markdown
---
fixture: setup-my-scenario.sh
launch: zelligent  # INSTALLED CLI — never the fixture clone's ./zelligent.sh
session_name: zelligent-test-repo
---

# My Test Scenario

## Test 1: Something works
- Action: Press a key or run a shell command described in the plan
- Expected: The resulting UI state matches the plan
```

## Fixture scripts

Fixture scripts must:
- Create the test repo at `/tmp/zelligent-test-repo`
- Seed any repo-local layout files the plan depends on
- Print `REPO_DIR=/tmp/zelligent-test-repo` to stdout
- Be idempotent (clean up before setting up)

The `teardown.sh` script kills the isolated tmux socket, removes the known test
Zellij sessions, clears stale session resurrection state, and removes the
temporary repo and worktrees.

## Harness Learnings

- Test setup must be boring and deterministic. If teardown "usually" works, it is broken.
- Treat session isolation as part of the test contract. If stale tabs or stale session state leak across runs, the result is invalid.
- Prefer absolute paths when driving tmux on macOS. `/tmp` often resolves to `/private/tmp`, and getting that wrong creates fake failures.
- Run harness setup serially. Parallel setup made failures harder to trust than the product itself.
