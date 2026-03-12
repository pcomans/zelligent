# Quality Score

Quick-reference grading of each project area. Reviewed periodically to track known gaps.

**Last reviewed:** 2026-03-12

## CLI (`zelligent.sh`)

| Area | Status | Gap |
|------|--------|-----|
| Test coverage | 🟢 | Comprehensive test.sh with 50+ assertions covering all subcommands |
| Error handling | 🟢 | Validates args, missing layout/sidebar assets, non-git dirs, and invalid custom layouts |
| Documentation | 🟢 | Layout contract and sidebar behavior are documented in ARCHITECTURE.md and references |

## Plugin (`plugin/`)

| Area | Status | Gap |
|------|--------|-----|
| Test coverage | 🟡 | 105 unit + 21 snapshot tests; `execute()`/`fire_*` untestable (Zellij FFI). Fix: extract a `ZellijApi` trait, have `State` hold `Box<dyn ZellijApi>`, inject mock in tests |
| State/UI separation | 🟢 | Render functions are pure (`&impl Write`), handlers return `Action` enums |
| Error handling | 🟢 | Unknown/missing cmd_type, empty stderr, malformed pipe messages all surface user-visible errors |

## Documentation (`docs/`)

| Area | Status | Gap |
|------|--------|-----|
| Coverage | 🟢 | Design docs cover session layout, tab behavior, resurrection, and notifications |
| Freshness | 🟢 | Sidebar/layout docs refreshed for the persistent-sidebar model on 2026-03-12 |
| Index completeness | 🟢 | All current design docs are indexed in design-docs/index.md |

## CI/CD

| Area | Status | Gap |
|------|--------|-----|
| Shell tests | 🟢 | Run on every PR |
| Plugin tests | 🟢 | cargo test + WASM build on every PR |
| UI/harness tests | 🟡 | Exist but run manually (tmux + test-driver agent), not in CI |

## Developer Experience

| Area | Status | Gap |
|------|--------|-----|
| Setup | 🟢 | `zelligent doctor` installs permissions, config defaults, and the canonical user layout |
| Build | 🟡 | Requires PATH workaround for Homebrew Rust (documented but manual) |
