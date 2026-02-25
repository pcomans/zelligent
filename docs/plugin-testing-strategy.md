# Automated UI Testing Strategy for Zelligent Plugin

## Context

Currently, testing the plugin's UI requires manually starting Zellij, loading the plugin, granting permissions, and stepping through interactions by hand. The plugin already has 59 unit tests covering pure state/logic, but **zero tests for rendered output or end-to-end interaction flows** (key press → state change → rendered UI).

Zellij's internal plugin test infrastructure (`create_plugin_thread()` + mock channels) is tightly coupled to `zellij-server` internals and **cannot be used by external plugins**. We need our own approach.

The good news: the plugin's architecture is already well-suited for render testing — `ui.rs` contains **pure functions** that just `println!()` formatted ANSI strings, and all state handlers are pure functions returning `Action` enums.

## Strategy: Two Layers

### Layer 1: Render Snapshot Tests (fast, in `cargo test`)

**Goal:** Capture and snapshot-test the actual ANSI output from render functions.

**Change required:** Refactor `ui.rs` functions from `println!()` to `writeln!(writer, ...)` so tests can capture output into a `Vec<u8>` instead of stdout.

```rust
// Before
pub fn render_header(repo_name: &str, cols: usize) {
    let title = format!(" zelligent: {} ", repo_name);
    let pad = cols.saturating_sub(title.len());
    println!("{BOLD}{CYAN}{title}{}{RESET}", "─".repeat(pad));
}

// After
pub fn render_header(w: &mut impl Write, repo_name: &str, cols: usize) {
    let title = format!(" zelligent: {} ", repo_name);
    let pad = cols.saturating_sub(title.len());
    writeln!(w, "{BOLD}{CYAN}{title}{}{RESET}", "─".repeat(pad)).unwrap();
}
```

The `render()` method in `main.rs` passes `&mut std::io::stdout()`. Tests pass `&mut Vec<u8>`.

**What this enables:**
- Snapshot every UI mode with `insta::assert_snapshot!()`
- Test scrolling/pagination (worktree list with 20 items in 10-row viewport)
- Test empty states (no worktrees → ASCII art)
- Test error/success status message rendering
- Test the full `render()` orchestration (mode → correct UI components)

**Files to modify:**
- `plugin/src/ui.rs` — add `w: &mut impl Write` parameter to all render functions
- `plugin/src/main.rs` — update `render()` to pass `&mut std::io::stdout()`, add render snapshot tests
- `plugin/Cargo.toml` — add `insta` dev-dependency

### Layer 2: Interaction Flow Tests (fast, in `cargo test`)

**Goal:** Test full user interaction sequences: key press → state mutation → rendered output.

These tests combine the existing pure key handlers with the new render capture:

```rust
#[test]
fn browse_then_select_branch_then_spawn() {
    let mut state = state_with_worktrees();
    let mut buf = Vec::new();

    // User sees worktree list
    state.render_to(&mut buf, 20, 80);
    insta::assert_snapshot!("initial_browse", String::from_utf8_lossy(&buf));

    // User presses 'n' to open branch picker
    buf.clear();
    let action = state.handle_key_browse(&key(BareKey::Char('n')));
    assert_eq!(state.mode, Mode::SelectBranch);
    state.render_to(&mut buf, 20, 80);
    insta::assert_snapshot!("branch_picker", String::from_utf8_lossy(&buf));

    // User presses 'j' to move down, then Enter to spawn
    state.handle_key_select_branch(&key(BareKey::Char('j')));
    buf.clear();
    state.render_to(&mut buf, 20, 80);
    insta::assert_snapshot!("branch_picker_moved", String::from_utf8_lossy(&buf));
}
```

This tests the **full interaction loop** without needing Zellij running. The `render_to()` method is identical to `render()` but writes to the provided buffer instead of stdout.

**Scenarios to cover:**
- Empty state → press `n` → branch picker → select → spawns
- Empty state → press `i` → type branch name → Enter → spawns
- Worktree list → navigate → press `d` → confirm dialog → `y` → removes
- Error states (spawn failure, remove failure) → error message appears
- Status messages clear on next action
- Scrolling through long lists

### What We Explicitly Don't Do

- **No WASM-level integration tests** — Zellij's `create_plugin_thread()` is internal; reimplementing it would be fragile and high-maintenance
- **No Docker/SSH E2E tests** — overkill for a plugin of this size
- **No headless Zellij E2E** — `test.sh` already covers the shell CLI; the plugin layer is better tested at the unit level with the approach above

## Implementation Steps

1. Add `insta` to `plugin/Cargo.toml` as a dev-dependency
2. Refactor `ui.rs`: add `w: &mut impl Write` to all 7 render functions, change `println!()` → `writeln!(w, ...).unwrap()`
3. Add `render_to(&mut self, w: &mut impl Write, rows: usize, cols: usize)` method to `State`. Note: the `Loading` branch in `render()` has bare `println!()` calls (not routed through `ui.rs`) — these must also become `writeln!(w, ...).unwrap()` in `render_to()`
4. Update `render()` to call `self.render_to(&mut std::io::stdout(), rows, cols)`
5. Write render snapshot tests for each mode/state combination
6. Write interaction flow tests covering key user journeys
7. Run `cargo insta review` to accept initial snapshots

## Verification

Because `.cargo/config.toml` defaults to `wasm32-wasip1`, tests must target the native host explicitly (this matches what CI does):

```bash
cd plugin
HOST=$(rustc -vV | awk '/^host:/ {print $2}')
cargo test --target "$HOST"              # all tests pass (existing + new)
cargo insta test --target "$HOST" --review   # review and accept snapshots
```

Snapshot files will live in `plugin/src/snapshots/` and are committed to git, so regressions in rendering are caught automatically in CI.

## Notes

- `insta` is a dev-dependency, so it is only compiled for the native host target (via `cargo test --target <host>`), never for wasm. The production `cargo build --target wasm32-wasip1` ignores dev-dependencies entirely.
- Render functions use `.unwrap()` on write errors to match `println!()`'s existing panic-on-failure behavior.
