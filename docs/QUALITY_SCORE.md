# Quality Score

Quick-reference grading of each project area. Reviewed periodically to track known gaps.

**Last reviewed:** 2026-03-11

## CLI (`zelligent.sh`)

| Area | Status | Gap |
|------|--------|-----|
| Test coverage | 🟢 | Comprehensive test.sh with 50+ assertions covering all subcommands |
| Error handling | 🟢 | Validates args, non-git dirs, missing dependencies |
| Documentation | 🟢 | Well-documented in ARCHITECTURE.md and design docs |

## Plugin (`plugin/`)

| Area | Status | Gap |
|------|--------|-----|
| Test coverage | 🟡 | 105 unit + 21 snapshot tests; `execute()`/`fire_*` untestable (Zellij FFI). Fix: extract a `ZellijApi` trait, have `State` hold `Box<dyn ZellijApi>`, inject mock in tests |
| State/UI separation | 🟢 | Render functions are pure (`&impl Write`), handlers return `Action` enums |
| Error handling | 🟢 | Unknown/missing cmd_type, empty stderr, malformed pipe messages all surface user-visible errors |

## Documentation (`docs/`)

| Area | Status | Gap |
|------|--------|-----|
| Coverage | 🟢 | Design docs for all major subsystems |
| Freshness | 🟢 | Audited 2026-03-11: 7/9 docs fully accurate, 2 have minor stale line refs |
| Index completeness | 🟢 | All design docs indexed in design-docs/index.md |

## CI/CD

| Area | Status | Gap |
|------|--------|-----|
| Shell tests | 🟢 | Run on every PR |
| Plugin tests | 🟢 | cargo test + WASM build on every PR |
| UI/harness tests | 🟡 | Exist but run manually (tmux + test-driver agent), not in CI |

## Developer Experience

| Area | Status | Gap |
|------|--------|-----|
| Setup | 🟢 | `zelligent doctor` handles everything |
| Build | 🟡 | Requires PATH workaround for Homebrew Rust (documented but manual) |
