# Plugin Testing Guide

## Architecture

The plugin uses a **lib/bin split** so tests run on the native host without WASM:

- `src/lib.rs` — all logic (State, handlers, rendering, ui). Tested natively.
- `src/main.rs` — just `register_plugin!(State)` + a linker stub. Never tested directly.
- `src/ui.rs` — render functions accept `&mut impl Write` so tests capture output into a `Vec<u8>`.

The `register_plugin!()` macro generates `#[no_mangle]` symbols (including `pipe`) that collide with libc on native builds. Keeping it isolated in the bin target prevents this.

## Running tests

```bash
# .cargo/config.toml defaults to wasm32-wasip1, so always specify the host target:
cargo test --target "$(rustc -vV | awk '/^host:/ {print $2}')"

# Update snapshots after intentional UI changes:
INSTA_UPDATE=always cargo test --target "$(rustc -vV | awk '/^host:/ {print $2}')"

# Verify WASM still builds (requires: rustup target add wasm32-wasip1):
PATH="$HOME/.rustup/toolchains/stable-$(rustc -vV | grep host | cut -d' ' -f2)/bin:$PATH" \
  cargo build --target wasm32-wasip1 --release
```

## Test types

### Unit tests (`src/lib.rs` — `mod tests`)

Test pure state logic: key handlers, parsers, navigation, command result handling.

Handlers return an `Action` enum (no side effects), so tests just assert on state mutations and the returned action:

```rust
#[test]
fn browse_j_moves_down() {
    let mut s = state_with_worktrees();
    s.handle_key_browse(&key(BareKey::Char('j')));
    assert_eq!(s.selected_index, 1);
}
```

### Render snapshot tests (`tests/render_snapshots.rs`)

Capture full ANSI output via `render_to()` into a `Vec<u8>` and compare with [insta](https://insta.rs) snapshots stored in `tests/snapshots/`.

```rust
#[test]
fn render_browse_with_worktrees() {
    let s = state_with_worktrees();
    insta::assert_snapshot!(render_to_string(&s, 20, 80));
}
```

### Interaction flow tests (`tests/render_snapshots.rs`)

Chain key presses with render snapshots to test full user journeys:

```rust
#[test]
fn flow_browse_navigate_and_render() {
    let mut s = state_with_worktrees();
    insta::assert_snapshot!("initial", render_to_string(&s, 20, 80));

    s.handle_key_browse(&key(BareKey::Char('j')));
    insta::assert_snapshot!("after_j", render_to_string(&s, 20, 80));
}
```

### Shared helpers (`tests/common/mod.rs`)

```rust
key(BareKey)              // KeyWithModifier with no modifiers
state_with_worktrees()    // State in BrowseWorktrees mode with 3 worktrees
render_to_string(&s, rows, cols)  // Render state to String via Vec<u8>
```

The unit tests in `lib.rs` have their own copies of `key()` and `state_with_worktrees()` (plus `key_shift()` and `make_tab()`), since `mod tests` can't import from integration test helpers.

## Adding new tests

1. **New handler/parser logic** — add a unit test in `src/lib.rs` `mod tests`.
2. **New UI mode or visual change** — add a snapshot test in `tests/render_snapshots.rs`, run with `INSTA_UPDATE=always`, review the generated `.snap` file.
3. **New user journey** — add a flow test that chains key presses with snapshots at each step.

## Key design decisions

- **`Action` enum** — handlers are pure functions that return actions. Side effects (Zellij host calls) happen in `execute()`. This makes handlers fully testable on native.
- **`render_to(&self, w: &mut impl Write, ...)`** — production calls `render()` which passes `stdout`; tests pass `Vec<u8>`.
- **`host_run_plugin_command` stub** — `main.rs` provides a no-op `#[cfg(not(target_arch = "wasm32"))]` stub for a WASM host import that zellij-tile references. Without it, native linking fails.
