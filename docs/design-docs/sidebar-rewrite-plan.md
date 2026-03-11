# Persistent Sidebar Rewrite Plan

Clean-room implementation of the persistent sidebar tab navigator.
Replaces the floating Ctrl-Y plugin with an always-visible left sidebar embedded in every zelligent-managed tab.

## Goal

Every zelligent tab has a 24%-width sidebar on the left showing all session tabs. Users navigate between tabs, spawn new worktrees, and remove old ones without leaving the sidebar. Manually created tabs (Ctrl+t n) also inherit the sidebar via `default_tab_template`.

## Hard-Won Lessons (from PR #77)

These are the traps discovered during the failed first attempt. Each PR in this rewrite MUST be validated against them.

### L1: KDL layout nesting — do not nest `split_direction` inside `split_direction`

The failed PR had `pane_content()` call `tab_body_content()`, which injected a second `pane split_direction="horizontal" { ... }` wrapper inside the outer sidebar horizontal split. This created garbled layouts with duplicate tabs.

**Rule:** The layout for a spawned tab must be a flat structure:

```kdl
layout {
    pane split_direction="Vertical" {
        pane size="24%" {
            plugin location="file:..." { ... }
        }
        pane size="70%" {
            // agent pane (inline, no extra split_direction wrapper)
        }
        pane size="30%" command="lazygit" cwd="..."
    }
    pane size=1 borderless=true {
        plugin location="zellij:status-bar"
    }
}
```

Actually — the agent and lazygit need a **vertical** sub-split (top/bottom) inside the remaining 76% horizontal space. The correct nesting is:

```kdl
pane split_direction="Vertical" {
    pane size="24%" { plugin ... }
    pane {
        // defaults to horizontal split (top/bottom) — no split_direction attribute
        pane command="bash" size="70%"    // agent
        pane command="lazygit" size="30%" // lazygit
    }
}
```

**Test:** `dump-layout` on every spawned tab must show exactly this structure. No double `split_direction="horizontal"`.

### L2: `new-tab --layout` does NOT inherit `default_tab_template`

Zelligent spawns tabs with `zellij action new-tab --layout FILE`. The layout file must explicitly include the sidebar plugin pane. The `default_tab_template` only applies to tabs created without `--layout` (e.g., user pressing Ctrl+t n).

**Rule:** Two separate layout functions:
1. `spawned_tab_layout()` — full layout with sidebar + agent + lazygit + status-bar (for `new-tab --layout`)
2. `default_tab_template` — sidebar + `children` placeholder + status-bar (for session startup layout only)

### L3: `new-tab --name` (without `--layout`) DOES inherit `default_tab_template`

Confirmed empirically. When zelligent creates a session with `--new-session-with-layout` and the layout includes a `default_tab_template`, any subsequent `zellij action new-tab --name foo` (no `--layout`) will use that template. This is how user tabs get the sidebar for free.

### L4: The sidebar plugin must NOT call `close_self()`

On main, `SwitchToTab` calls `close_self()` because the plugin is a floating pane that should dismiss. In sidebar mode, the plugin is a persistent layout pane — calling `close_self()` would destroy it. Remove `close_self()` from the `SwitchToTab` handler.

### L5: Tab identity uses names, not indices

Zellij's `TabInfo.position` != tab index. The plugin must use `go_to_tab_name()` for all tab operations. Tab names are the only reliable identifier. See `docs/design-docs/tab-management.md`.

### L6: Duplicate tab names are possible but rare

Two branches that sanitize to the same name (e.g., `feat/cool` and `feat-cool`) produce duplicate tab names. `go_to_tab_name()` always targets the first match. The sidebar should disambiguate display names (e.g., "(1)", "(2)") but this is a known limitation. Don't over-engineer it.

### L7: `kill_sessions` terminates the plugin process

Nothing after `kill_sessions(&[...])` executes. This is documented in `docs/design-docs/tab-management.md`.

### L8: The `q` keybinding changes meaning

In floating mode, `q` closes the plugin (`close_self()`). In sidebar mode, `q` should either do nothing or be removed, since closing the sidebar is destructive. Consider keeping `q` as a no-op or repurposing it.

### L9: Plugin path resolution must be centralized

The failed PR had `resolve_plugin_path()` which was good, but the path leaked into tab names due to a variable scoping bug. Keep the resolver, but make sure it's only used in layout generation — never in session/tab name construction.

### L10: `exec zellij` replaces the shell — no cleanup runs after it

When starting a new session with `exec zellij --new-session-with-layout`, the shell process is replaced. Any `trap ... EXIT` or cleanup code after it will never run. Temp layout files created before `exec` must either be cleaned up before exec (background subshell) or accepted as harmless leaks.

### L11: Doctor should stop installing the Ctrl-Y keybinding

Since the plugin is now layout-embedded, the floating keybinding is unnecessary. Doctor should skip keybinding setup and print a message explaining why.

### L12: Test the inside-Zellij AND outside-Zellij spawn paths separately

The spawn command has two code paths:
- **Inside Zellij** (`$ZELLIJ` is set): uses `zellij action new-tab --layout FILE`
- **Outside Zellij**: creates a session with `--new-session-with-layout` or attaches and adds a tab

Both must produce correct layouts. The failed PR only got the outside path right.

---

## Implementation Stack

### PR 1: CLI layout refactor — sidebar-aware layout generation

**Branch:** `sidebar/cli-layout`
**Base:** `main`
**Scope:** `zelligent.sh` only (no plugin changes)

Changes:
1. Add `resolve_plugin_path()` function (lines 32-55 of failed PR — this part was correct)
2. Refactor layout generation into clear functions:
   - `sidebar_pane()` — the 24% plugin pane KDL fragment
   - `spawned_tab_layout()` — full flat layout for `new-tab --layout` (sidebar + agent/lazygit vertical split + status-bar)
   - `session_startup_layout()` — layout with `default_tab_template` (sidebar + `children` + status-bar) plus initial tab
3. Remove `zellij:tab-bar` from all zelligent layouts (sidebar replaces it)
4. Update the inside-Zellij and outside-Zellij code paths to use the new functions
5. Keep the `pane_content()` / `tab_body_content()` pattern for custom layouts (`.zelligent/layout.kdl`) unchanged for now

**Validation:**
- `bash test.sh` passes (update assertions: tab-bar -> sidebar plugin, add sidebar structure checks)
- Headless session test: create session, `dump-layout`, verify `default_tab_template` has sidebar
- Headless spawn test (inside Zellij): spawn from inside, `dump-layout`, verify tab has sidebar at 24%
- Headless spawn test (outside Zellij): spawn from outside, verify session + tab layout

**Test plan:** `tests/harness/plans/sidebar-layout-smoke.md`
- Test 1: `zelligent` (no args) from test repo → session starts with sidebar visible
- Test 2: `zelligent spawn feature-a bash` from inside the session → new tab has sidebar
- Test 3: Create manual tab (Ctrl+t, n, type name, Enter) → tab inherits sidebar from `default_tab_template`
- Test 4: Verify `dump-layout` shows correct structure for all three tab types

---

### PR 2: Plugin state model — SidebarItem and tab-aware browsing

**Branch:** `sidebar/plugin-model`
**Base:** `sidebar/cli-layout`
**Scope:** `plugin/src/lib.rs`, `plugin/tests/`

Changes:
1. Add `SidebarItem` struct:
   ```rust
   pub struct SidebarItem {
       pub tab_name: String,
       pub display_name: String,
       pub matched_branch: Option<String>,
   }
   ```
2. Add `sidebar_items: Vec<SidebarItem>` to `State`
3. Add `recompute_sidebar_items()` — builds sidebar items from `self.tabs` + `self.worktrees`, with duplicate name disambiguation
4. Call `recompute_sidebar_items()` in `handle_tab_update()` and `handle_list_worktrees()`
5. Change `BrowseWorktrees` Enter action: use `Action::SwitchToTab(item.tab_name)` instead of `spawn_or_switch`
6. Remove `close_self()` from `SwitchToTab` handler (L4)
7. Remove `has_loaded` flag — `recompute_sidebar_items` handles cursor restoration by tab name
8. Change `q` to no-op in browse mode (L8)

**Validation:**
- `cd plugin && cargo test --target "$(rustc -vV | awk '/^host:/ {print $2}')"` — all unit tests pass
- Snapshot tests updated for any changed render output
- Test that cursor restoration works: select tab B, trigger TabUpdate, verify cursor stays on B

**No harness test yet** — the rendering changes come in PR 3.

---

### PR 3: Plugin rendering — two-line sidebar rows with powerline styling

**Branch:** `sidebar/plugin-render`
**Base:** `sidebar/plugin-model`
**Scope:** `plugin/src/ui.rs`, `plugin/tests/`

Changes:
1. Add `unicode-width` dependency to `Cargo.toml`
2. Add text helpers: `visible_width()`, `clip_to_width()`, `fit_text()`
3. Replace `render_worktree_list()` with `render_sidebar_list()`:
   - Two-line rows: display name (with kind icon) + subtitle ("branch: X" or "user tab")
   - Powerline-style selected chip (with ASCII fallback via `supports_arrow_fonts`)
   - Scrolling via `visible_sidebar_window()`
4. Add `render_status_strip()` for "ready" / status messages
5. Update `render_header()` to show tab count
6. Update `render_footer()` to accept `cols` for width-aware layout
7. Rename `AgentStatus::NeedsInput` -> `AgentStatus::Awaiting`

**Validation:**
- `INSTA_UPDATE=always cargo test` — update all snapshots, review each one
- Harness test: `tests/harness/plans/sidebar-render-check.md`
  - Verify two-line rows visible in sidebar
  - Verify branch subtitles correct
  - Verify user tabs show "user tab" subtitle

---

### PR 4: Mouse support and interaction polish

**Branch:** `sidebar/mouse-support`
**Base:** `sidebar/plugin-render`
**Scope:** `plugin/src/lib.rs`, `plugin/src/ui.rs`

Changes:
1. Subscribe to `EventType::Mouse` and `EventType::ModeUpdate`
2. Add `last_rows: usize` field for click-to-line mapping
3. Add `handle_mouse_browse()`: scroll up/down, click to select, double-click to switch
4. Add `sidebar_index_at_line()` and `visible_sidebar_window()` helpers
5. Add `supports_arrow_fonts: Option<bool>` from ModeUpdate for render detection

**Validation:**
- Unit tests for `sidebar_index_at_line` with various viewport sizes
- Harness test: `tests/harness/plans/sidebar-mouse-interaction.md`
  - Click on non-selected row → selection changes
  - Click again → tab switches
  - Scroll wheel → list scrolls

---

### PR 5: Doctor and docs update

**Branch:** `sidebar/doctor-docs`
**Base:** `sidebar/mouse-support`
**Scope:** `zelligent.sh` (doctor), `docs/`, `README.md`, `AGENTS.md`

Changes:
1. Doctor: skip Ctrl-Y keybinding installation, print "keybinding: skipped (persistent sidebar)"
2. Doctor: keep permissions check (plugin still needs RunCommand, etc.)
3. Update `docs/ARCHITECTURE.md` — floating -> sidebar
4. Update `docs/PRODUCT_SENSE.md` — layout spec, remove Ctrl-Y references
5. Update `docs/design-docs/tab-management.md` — sidebar-based navigation
6. Update `docs/references/zellij-kdl-layout.md` — sidebar layout examples
7. Update `README.md` with new session startup behavior
8. Update `tests/harness/plans/` — existing plans adapted for sidebar
9. Add `tests/harness/plans/mixed-three-tabs-switching.md` (from failed PR, already written)

**Validation:**
- `bash test.sh` fully green
- All harness tests pass
- `DOCS_VERIFIED=1` ready for push

---

## Agent Team Roles

### Agent 1: Implementer

Works through PRs 1-5 sequentially. Each PR is a separate branch stacked on the previous. Creates the branch, writes the code, runs `bash test.sh`, and opens the PR.

**Instructions:**
- Follow this plan exactly — do not add features not listed
- Run `bash test.sh > /tmp/test-output.txt 2>&1` after every change and fix failures before committing
- For plugin changes, run `cd plugin && cargo test --target "$(rustc -vV | awk '/^host:/ {print $2}')"` first
- Each PR should be independently reviewable — no forward references
- Commit messages: `feat(scope): description` or `fix(scope): description`

### Agent 2: Reviewer

Reviews each PR after Agent 1 opens it. Checks:
- Does the change violate any lesson L1-L12?
- Does `dump-layout` show the correct structure?
- Are there nested `split_direction` bugs?
- Does `close_self()` appear where it shouldn't?
- Are tab names correct (no plugin path leaking)?
- Do tests cover both inside-Zellij and outside-Zellij paths?

**Instructions:**
- Read the "Hard-Won Lessons" section before each review
- Run `bash test.sh` independently
- Check `dump-layout` output for every layout change
- Flag any deviation from lessons L1-L12 as blocking

### Agent 3: Test Driver

Writes and executes harness test plans for each PR. Creates the test plan markdown, then executes it via Chrome DevTools MCP.

**Instructions:**
- Write test plans in `tests/harness/plans/` following the existing format
- Each test plan covers ONE PR's changes
- Execute via the test-driver agent protocol (setup, authenticate, create session, run steps, teardown)
- Take screenshots at every step
- Report PASS/FAIL with actual observations
- Sidebar must be visually confirmed — not just dump-layout assertions

### Sequencing

```
PR 1 (CLI layout) → Agent 1 implements → Agent 2 reviews → Agent 3 tests layout
PR 2 (plugin model) → Agent 1 implements → Agent 2 reviews → (unit tests only)
PR 3 (rendering) → Agent 1 implements → Agent 2 reviews → Agent 3 tests rendering
PR 4 (mouse) → Agent 1 implements → Agent 2 reviews → Agent 3 tests interaction
PR 5 (docs) → Agent 1 implements → Agent 2 reviews → Agent 3 runs full regression
```

## Test Plans to Write

### `sidebar-layout-smoke.md` (PR 1)

```markdown
---
fixture: setup-with-worktrees.sh
---

# Sidebar Layout Smoke Test

## Test 1: Session starts with sidebar
- Action: Run `zelligent` from /tmp/zelligent-test-repo (outside Zellij)
- Expected: Session opens with sidebar plugin visible on the left (~24% width)

## Test 2: Spawned tab has sidebar
- Action: From inside the session, run `zelligent spawn feature-a bash`
- Expected: New tab opens with sidebar on left, bash pane in center, lazygit on right

## Test 3: Manual tab inherits sidebar
- Action: Create a new tab via Zellij (Ctrl+t, then type a name, Enter)
- Expected: New tab has sidebar pane on the left (from default_tab_template)

## Test 4: Layout structure is correct
- Action: Run `zellij action dump-layout` from inside the session
- Expected: Each zelligent tab shows `pane split_direction="horizontal"` with plugin at 24%, no nested horizontal splits
```

### `sidebar-render-check.md` (PR 3)

```markdown
---
fixture: setup-with-worktrees.sh
---

# Sidebar Render Check

## Test 1: Spawn two worktree tabs
- Action: Run `zelligent spawn feature-a bash`, then `zelligent spawn feature-b bash`
- Expected: Sidebar shows two-line rows for each tab with "branch: feature-a" and "branch: feature-b" subtitles

## Test 2: Tab switching via Enter
- Action: Press j to move to the other tab, press Enter
- Expected: Focus switches to the other tab, sidebar remains visible

## Test 3: Status line shows "ready"
- Action: Look at sidebar bottom area
- Expected: Status line shows "ready" text
```

### `sidebar-mouse-interaction.md` (PR 4)

```markdown
---
fixture: setup-with-worktrees.sh
---

# Sidebar Mouse Interaction

## Test 1: Click to select
- Action: Spawn two tabs, click on the non-selected row in the sidebar
- Expected: Selection highlight moves to clicked row without switching tabs

## Test 2: Click again to switch
- Action: Click the same row again
- Expected: Tab switches to the selected tab

## Test 3: Scroll navigation
- Action: Spawn enough tabs to overflow, scroll in sidebar
- Expected: List scrolls to reveal hidden tabs
```
