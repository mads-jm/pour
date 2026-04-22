---
date created: Tuesday, April 21st 2026
date modified: Tuesday, April 21st 2026
---

# Clippy / CI Parity

CI treats clippy warnings as errors (`-D warnings`), so any lint the local workflow misses becomes a hard build failure on push. Plain `cargo clippy` only warns, which lets drift slip through.

## Run locally what CI runs

```bash
cargo clippy --all-targets -- -D warnings
cargo fmt -- --check
cargo test
```

`--all-targets` covers lib, bins, tests, and examples — matching CI's surface area. Without it, test-only code (which is most of `tests/`) goes unchecked.

## Lints that have bitten us

- **`collapsible_match`** — `match X { Variant => { if cond { ... } } }` must be written as `Variant if cond => { ... }` when there's a catchall arm. Hit in `src/tui/form.rs` callout title edit handler (4 arms: `Backspace`, `Left`, `Right`, `Char(c)` each wrapping an `if` around the body).
- **`unused_mut` across cfg boundaries** — a `let mut x` is fine on Windows (where a `#[cfg(target_os = "windows")]` block mutates it) but flags as unused on Linux CI. Scope the allow to the platforms where it's genuinely unused: `#[cfg_attr(not(target_os = "windows"), allow(unused_mut))]`. Don't blanket-allow. Hit in `src/init.rs::detect_obsidian_vaults` — local Windows build was happy, Linux CI was not.

## Rule of thumb

If rustc is happy but clippy isn't, CI will reject the push. Pre-push check should be the three-command block above, not just `cargo build`.

## Test flakes from shared mutexes

Tests that mutate process-wide state (env vars, CWD, etc.) commonly serialize via a `static Mutex`. Two gotchas the suite has hit:

1. **Every test touching the shared state must acquire the mutex**, not just the ones clustered near its definition. Missing just one creates a race that Ubuntu's scheduler may hide but Windows' will expose. Hit when `load_with_pour_config_env_var_nonexistent_file` and `load_from_pour_config_env_var` mutated `POUR_CONFIG` without `SECRETS_ENV_LOCK`, racing the guarded secrets tests.
2. **Prefer `.lock().unwrap_or_else(|e| e.into_inner())` over `.lock().unwrap()`**. A plain `unwrap` panics on a poisoned mutex — which happens any time another test panics mid-critical-section — so one genuine failure cascades into N false PoisonError failures, burying the real signal. Using `into_inner` keeps the real failure visible and lets the rest of the suite run.

Integration test isolation note: each file under `tests/` compiles to a separate binary and runs as its own process, so env vars don't leak across files. Within a single file, though, tests share a process and run in parallel threads.
