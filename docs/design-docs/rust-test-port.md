# Plan: Port `test.sh` to Rust

## Context

`test.sh` (782 lines) is the CLI integration test suite for `zelligent.sh`. It runs in CI (`ci.yml` -> `bash test.sh`) and contains 119 assertions across 51 logical test scenarios in 13 sections.

### Why port now

The CLI (`zelligent.sh`) is planned for a Rust rewrite. Porting the tests first means:

1. **The test harness carries forward.** `MockBin`, `TempRepo`, and assertion helpers will test the Rust CLI directly once it exists -- no throwaway work.
2. **Structured output.** `cargo test` produces machine-readable output that CI and AI agents can parse reliably. The bash suite's emoji-based output requires redirection workarounds.
3. **KDL parsing.** The `kdl` crate replaces a Python regex extractor for prompt delivery tests. While the extractor has never broken, it simulates KDL parsing with regex -- the Rust version validates the actual KDL structure, which becomes essential when the CLI generates more complex layouts.

### What's wrong with the bash tests

- **No KDL parsing.** Layout validation uses `grep -qF`. The prompt delivery harness uses a Python regex extractor.
- **Real repo contamination.** Tests create worktrees from the real repo, requiring `mv`/`restore` of `.zelligent/setup.sh` and careful branch cleanup.
- **Repetitive mock creation.** Every section creates 2-6 mock binaries with near-identical heredocs.
- **Unstructured output.** Pass/fail is emoji-based text. Long output gets truncated by tooling.

## Accurate test inventory

| Section | Assertions | Scenarios | Notes |
|---------|-----------|-----------|-------|
| Session name | 3 | 3 | Pure string transform, no subprocess |
| Layout generation | 16 | 5 | Mock zellij cats layout file; checks KDL content via string matching |
| Prompt delivery | 7 | 4 | Python regex extractor simulates KDL -> bash -c; gated on python3 |
| Version/help | 8 | 3 | --version, --help, help |
| No-args | 8 | 4 | Non-git, no plugin, inside zellij, outside zellij |
| Stale socket | 7 | 3 | Mock zellij hangs on list-sessions; relies on 3s perl timeout |
| Arg validation | 6 | 3 | Missing args, unknown command |
| Environment | 2 | 1 | Non-git directory |
| Nuke | 7 | 5 | Mock ps/kill/sleep; fake HOME |
| Doctor | 21 | 6 | Config patching, idempotency, XDG, existing keybinds, missing plugin |
| Query | 19 | 9 | show-repo, list-worktrees, remove (mismatched, nonexistent, unmanaged), list-branches |
| Launch mode | 10 | 3 | Inside zellij, outside new, outside existing |
| Integration | 4 | 2 | Requires real zellij; background session + dump-layout |
| **Total** | **119** | **51** | |

## Crate location

New crate at `tests/cli/` as a **workspace member** alongside `plugin/`. The workspace root `Cargo.toml` lists both members. Per-member targets prevent the wasm default from leaking.

```
tests/cli/
  Cargo.toml
  src/
    lib.rs           # shared test harness
  tests/
    session_name.rs       # 3 scenarios
    layout.rs             # 5 scenarios (layout generation + quoted commands)
    prompt_delivery.rs    # 4 scenarios (KDL parsing proof-of-concept)
    version_help.rs       # 3 scenarios
    no_args.rs            # 4 scenarios
    stale_socket.rs       # 3 scenarios
    arg_validation.rs     # 3 scenarios + 1 environment
    nuke.rs               # 5 scenarios
    doctor.rs             # 6 scenarios
    query.rs              # 9 scenarios
    launch_mode.rs        # 3 scenarios
    integration.rs        # 2 scenarios (gated on zellij availability)
```

## Shared harness (`lib.rs`)

### `MockBin` builder

Creates a temp directory with mock shell scripts on PATH.

```rust
let mocks = MockBin::new()
    // Mock zellij that echoes args and cats layout files
    .zellij_echo()
    // Mock lazygit (no-op)
    .lazygit()
    // Mock ps that outputs nothing
    .script("ps", "#!/bin/bash\necho \"\"")
    // Build returns the temp dir (cleaned up on drop) and the PATH prefix
    .build();
```

Internally: `std::fs::write()` + `PermissionsExt::set_permissions(0o755)`. The builder provides preset methods for common mocks (`zellij_echo`, `zellij_hang`, `lazygit`, `noop_ps_kill`) and a generic `script(name, content)` for one-offs.

**PATH construction:** `format!("{}:{}", mocks.path().display(), original_path)` where `original_path` is captured once at harness init from the real environment. Tests need real `git`, `grep`, `basename`, etc.

### `Zelligent` command wrapper

```rust
let result = Zelligent::new(&mocks)
    .env("ZELLIJ", "1")
    .env("ZELLIJ_SESSION_NAME", "fake")
    .args(&["spawn", "test-branch", "claude"])
    .run();

assert_eq!(result.code, 0);
assert_contains!(&result.stdout, "Opening tab");
```

Wraps `std::process::Command`. Sets PATH from `MockBin`, points at `zelligent.sh` via `env!("CARGO_MANIFEST_DIR")` to find the repo root.

### `TempRepo`

Creates an isolated git repo in a temp dir with an initial commit.

```rust
let repo = TempRepo::new();
// Creates .zelligent/setup.sh if needed
repo.add_setup_sh("#!/bin/bash\necho setup");
// Returns the repo path for use in Zelligent commands
repo.path();
```

**Semantic tradeoff:** The bash tests operate on the real repo. `TempRepo` tests against a throwaway repo with minimal history. This is acceptable because:

- The tests validate `zelligent.sh` behavior (argument parsing, layout generation, subcommand logic), not git history traversal.
- Worktree creation/removal works identically on any valid git repo.
- `TempRepo` eliminates the setup.sh contamination problem entirely -- no move/restore dance, no `trap` cleanup, no race conditions between parallel test runs.
- The integration tests (Phase 4) can still optionally use the real repo if needed.

### `parse_layout_kdl()`

Uses the `kdl` crate to parse layout output, extract `args` nodes, and validate pane structure. Replaces the Python regex extractor for prompt delivery tests.

### Assertion macros

```rust
assert_contains!(haystack, needle);      // panics with both values on failure
assert_not_contains!(haystack, needle);
```

## Key design decisions

| Decision | Rationale |
|----------|-----------|
| Workspace member (not standalone crate) | `cargo test` from repo root runs everything; shared lockfile |
| `TempRepo` over real repo | Eliminates setup.sh contamination; safe for parallel execution |
| `kdl` crate for layout validation | Validates actual KDL structure; prepares for more complex layouts |
| One file per test section | Mirrors bash sections; easy to find and navigate |
| Keep `test.sh` until port is complete | CI stays green throughout |
| Prompt delivery in Phase 1 | Proves out KDL parsing early -- this is the highest-risk part |

## Dependencies

```toml
[dependencies]
kdl = "6"
tempfile = "3"
```

## Migration order

### Phase 1 -- Crate setup + KDL proof-of-concept (14 scenarios)

Create the crate, write `MockBin`, `Zelligent`, `TempRepo`, and assertion macros. Port:

- `session_name` (3) -- simplest, validates the harness works
- `version_help` (3) -- simple subprocess tests
- `arg_validation` (3 + 1 environment) -- simple exit code + output checks
- `prompt_delivery` (4) -- **proves KDL parsing works early**, highest-risk item

The point of this phase is to validate the approach. If `prompt_delivery` works with the `kdl` crate, the rest is mechanical.

### Phase 2 -- Layout tests (8 scenarios)

- `layout` (5) -- layout generation, quoted commands, setup.sh presence/absence
- `launch_mode` (3) -- inside/outside zellij mode selection

### Phase 3 -- Mock-heavy tests (14 scenarios)

- `no_args` (4)
- `stale_socket` (3) -- each test takes ~3s due to perl timeout; run unconditionally
- `nuke` (5)
- `doctor` (6)

### Phase 4 -- Git operations + integration (11 scenarios)

- `query` (9) -- show-repo, list-worktrees, remove, list-branches
- `integration` (2) -- gated on runtime zellij detection

### Phase 5 -- Cleanup

- Update CI: add `cargo test` for `tests/cli/` workspace member
- Remove `bash test.sh` CI step
- Update `AGENTS.md` and `docs/ARCHITECTURE.md`
- Delete `test.sh`

## CI changes

```yaml
test-cli:
  runs-on: macos-latest
  steps:
    - uses: actions/checkout@v4
      with:
        fetch-depth: 0
    - name: Set up remote HEAD
      run: git remote set-head origin --auto
    - name: Run CLI tests
      working-directory: tests/cli
      run: cargo test
```

## Risks and mitigations

| Risk | Mitigation |
|------|------------|
| Stale socket tests take ~3s each (perl timeout) | Run unconditionally; 12s total is acceptable |
| Integration tests require Zellij | Runtime detection with `which zellij`; skip with message if absent |
| Mock scripts are still bash | Unavoidable -- `zelligent.sh` invokes them via PATH |
| `TempRepo` misses real-repo-specific behavior | Integration tests in Phase 4 can use real repo if a scenario demands it |
| KDL parsing approach doesn't work for prompt delivery | Discovered in Phase 1, before investing in the full port |
| Port discovers untranslatable bash-specific test | Keep `test.sh` running in CI; drop individual bash tests only after their Rust equivalent passes |

## Rollback

If Phase 1 reveals that the approach is fundamentally flawed (e.g., `TempRepo` can't reproduce a critical test scenario, or `kdl` parsing doesn't match what Zellij actually does):

- The Rust crate gets deleted
- `test.sh` remains the test suite
- Consider the smaller fix: add TAP output to `test.sh` for better CI/tooling integration
