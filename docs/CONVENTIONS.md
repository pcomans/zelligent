# Conventions

When in doubt, follow these rules.

## Code — Plugin (`plugin/`)

- **Render functions** (`ui.rs`): must be pure. Take `&impl Write` + data, return nothing. No side effects, no Zellij API calls.
- **Key/event handlers** (`lib.rs`): `handle_*` methods return `Action` enums. Never call Zellij APIs directly.
- **Side effects**: all Zellij API calls go through `execute()` and `fire_*` methods only.

## Code — CLI (`zelligent.sh`)

- Error messages must name the subcommand: `"spawn: not inside a git repository"`, not `"not inside a git repository"`.

## Code — Tests (`test.sh`)

- Structure: `echo "Section name:"` followed by `check`/`contains`/`excludes` assertions.
- Always redirect output: `bash test.sh > /tmp/test-output.txt 2>&1; echo "EXIT: $?"` — long output gets swallowed otherwise.
- Snapshot tests: run `INSTA_UPDATE=always cargo test --target "$(rustc -vV | awk '/^host:/ {print $2}')"` in `plugin/`, review `.snap` files before committing.

## Zellij Gotchas

- `TabInfo.position` != tab index. Always use name-based tab operations (`go_to_tab_name`), never index-based.
- `split_direction="Vertical"` = left/right (sidebar). `"Horizontal"` = top/bottom. This is counterintuitive.
- `dump-layout` normalizes `split_direction` to **lowercase**. Test assertions must use lowercase.
- `kill_sessions(&[&name])` terminates the plugin's own process. Nothing after it runs.
- WASM plugins CAN read host env vars via `std::env::var()` (Zellij calls `builder.inherit_env()`).
- KDL layout templates are full `layout { ... }` files, not fragments.
- `.zelligent/layout.kdl` must contain `{{zelligent_sidebar}}` exactly once.
- `{{cwd}}` and `{{agent_cmd}}` are plain text placeholders. `{{agent_cmd}}` is a shell fragment intended for `bash -c`.

## Build

- Always use the PATH workaround for Homebrew Rust when building or testing the plugin:
  ```
  PATH="$HOME/.rustup/toolchains/stable-$(rustc -vV | grep host | cut -d' ' -f2)/bin:$PATH"
  ```
- Plugin unit tests require `--target "$(rustc -vV | awk '/^host:/ {print $2}')"` (host target, not wasm).
