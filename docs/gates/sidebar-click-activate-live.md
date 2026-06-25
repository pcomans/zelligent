# Gate G4 — live-path validation (issue #132)

> Architect integration gate. NOT run by the build lane — the in-flight builder
> neither sees nor can affect it. Run by an independent cheap-model judge against
> the BUILT + INSTALLED plugin on the integration branch, then architect spot-check.
> Added by the architect after freezing G1–G3, before any result was judged: the
> original gates were unit-only and could not prove a real click activates a tab.

Unit gates (G1–G3) prove `handle_mouse_browse` *returns* the activation Action.
They do NOT prove that a real left-click in a running Zellij session switches the
active tab. #132 is a live mouse-interaction defect, so merge requires a live
exercise of the actual rendered plugin.

## Setup (judge performs)

1. Build + install the plugin from the integration branch:
   ```
   PATH="$HOME/.rustup/toolchains/stable-$(rustc -vV | grep host | cut -d' ' -f2)/bin:$PATH" bash dev-install.sh
   ```
   (rustup toolchain in this devcontainer; the PATH line is the documented incantation.)
2. Seed the worktree fixture and launch under tmux with mouse reporting on, per
   `tests/harness/plans/sidebar-mouse-interaction.md` front-matter
   (fixture `setup-with-worktrees.sh`, launch `./zelligent.sh`, session
   `zelligent-test-repo`). Mouse sequences per `.claude/skills/tmux/SKILL.md`.

## G4 assertions (verbatim pass criteria — measured from capture-pane)

- G4a — SINGLE-CLICK ACTIVATION: with `zelligent-test-repo` the active tab and a
  non-selected worktree row (`feature-b`) visible, send exactly ONE left-click
  (SGR press+release) on the `feature-b` title line. PASS iff after that SINGLE
  click a `feature-b` tab is opened/active (the row's worktree is activated) AND
  the sidebar selection is on `feature-b`. FAIL if `feature-b` is merely selected
  while the active tab is unchanged (i.e. a second click would be required — the
  #132 bug).
- G4b — NO-OP ON BLANK: one left-click on a blank sidebar/footer line leaves the
  active tab and selection unchanged.
- Evidence: before/after `capture-pane` snapshots (ANSI-aware) for G4a and G4b
  pasted raw into the lane report. Each PASS/FAIL backed by a capture.

PASS threshold: G4a and G4b both PASS, evidenced by captures.

Judge returns an independent **SHIP | DO-NOT-SHIP** with the captures — no
knowledge of the architect's expectation. Architect then spot-checks the captures
for off-gate regressions (sidebar still renders, no stray lines) before the final
verdict.
