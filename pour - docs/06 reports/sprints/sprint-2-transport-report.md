---
tags:
  - sprint-report
  - v1
date created: Tuesday, March 31st 2026, 5:17:56 am
sprint: 2
status: complete
date modified: Friday, April 3rd 2026, 4:11:44 am
---

# Sprint 2 Report: Transport Layer

## Summary

Sprint 2 implemented the hybrid transport layer: an API client for the Obsidian Local REST API, a filesystem writer for direct vault access, and a unified dispatcher that tries the API first and falls back to filesystem. All three tasks passed inspection with `cargo check`, `cargo test` (34 tests), `cargo clippy`, and `cargo fmt -- --check` all clean.

## What Was Built

### TASK-004: API Client (`src/transport/api.rs`)

`ApiClient` struct wrapping `reqwest::Client` with:

- Constructor `new(port, api_key)` building an HTTPS client that accepts self-signed certificates with a 5-second timeout
- `check_connection()` -- GET `/` with Bearer auth, returns `bool` (catches all errors, never propagates)
- `create_file(vault_path, content)` -- PUT to `/vault/{path}` with `Content-Type: text/markdown`
- `append_under_heading(vault_path, heading, content)` -- PATCH to `/vault/{path}` using the v3 header-based API (`Operation: append`, `Target-Type: heading`, `Target: {heading}`)
- `list_directory(vault_dir_path)` -- GET `/vault/{dir}/` (trailing slash), deserializes `{"files": […]}` JSON response

All methods async, returning `anyhow::Result` with contextual error messages.

5 tests: HTTPS scheme validation, port embedding, default port, multi-port sweep, unreachable server returns false.

### TASK-005: Filesystem Writer (`src/transport/fs.rs`)

`FsWriter` struct holding a vault `base_path: PathBuf` with:

- `create_file(relative_path, content)` -- writes file, creates parent directories, errors if file already exists
- `append_to_file(relative_path, content)` -- appends to existing file, errors if file not found
- `list_directory(relative_dir_path)` -- lists `.md` files, returns sorted stem names, excludes non-`.md` files and subdirectories

All methods synchronous, returning `anyhow::Result` with contextual error messages.

10 tests: content writing, parent dir creation, duplicate file rejection, append behavior, missing file error, sorted stem listing, subdirectory exclusion, non-directory error, empty directory handling, base_path getter.

### TASK-006: Transport Dispatcher (`src/transport/mod.rs`)

- `TransportMode` enum (`Api` / `FileSystem`) with `Display` impl for TUI status display
- `Transport` enum wrapping `ApiClient` and `FsWriter`
- `Transport::connect(config)` -- attempts API connection when both `api_port` and `api_key` are configured and `check_connection()` succeeds; falls back to filesystem otherwise
- Unified `create_file`, `append_under_heading`, `list_directory` methods that delegate to the active backend
- Filesystem fallback for `append_under_heading` does a plain append (heading-aware insertion deferred to future sprint)
- `src/lib.rs` updated with `pub mod transport;`

6 tests: no-API-key fallback, unreachable-API fallback, create/append/list delegation through FS backend, TransportMode Display formatting.

## Issues Found and Fixed During Inspection

1. __Clippy collapsible_if (TASK-005)__: Nested `if path.is_file() { if let Some(ext) … { if ext == "md" { … }}}` flagged by clippy. Collapsed into a single `if` with let-chains (Rust 2024 edition).

2. __Formatting (TASK-005)__: Several `anyhow::bail!` and `.with_context()` calls had formatting that didn't match `cargo fmt` expectations. Reformatted.

3. __Module wiring__: Integration tests in `tests/transport/` require the module to be exported from `lib.rs`. Added minimal `pub mod transport;` to `lib.rs` and `pub mod api;` / `pub mod fs;` to `transport/mod.rs` as part of the build-up, rather than deferring all wiring to TASK-006.

4. __Test crate structure__: `tests/transport/` uses `main.rs` as the crate root (not `mod.rs`), following Rust's convention for directory-based integration test crates.

## Current State of the Codebase

### Files Modified/Created

| File | Status |
|------|--------|
| `Cargo.toml` | Modified - added `[dev-dependencies] tempfile = "3"` |
| `src/lib.rs` | Modified - added `pub mod transport;` |
| `src/transport/mod.rs` | New - Transport enum, TransportMode, connect/dispatch logic |
| `src/transport/api.rs` | New - ApiClient struct, all API methods |
| `src/transport/fs.rs` | New - FsWriter struct, all filesystem methods |
| `tests/transport/main.rs` | New - test crate root, wires api/fs/dispatcher modules |
| `tests/transport/api.rs` | New - 5 API client tests |
| `tests/transport/fs.rs` | New - 10 filesystem writer tests |
| `tests/transport/dispatcher.rs` | New - 6 dispatcher tests |

### Test Results

```ts
running 34 tests (13 config + 21 transport)

config: 13 passed
transport::api: 5 passed
transport::fs: 10 passed
transport::dispatcher: 6 passed

test result: ok. 34 passed; 0 failed
```

### Quality Gates

| Gate | Result |
|------|--------|
| `cargo check` | Clean |
| `cargo test -- --test-threads=1` | 34 passed, 0 failed |
| `cargo clippy` | Clean (no warnings) |
| `cargo fmt -- --check` | Clean (no diffs) |

## Design Decisions

1. __`base_url` is `pub`__ on `ApiClient` to allow test assertions on URL construction without needing a getter method.

2. __`list_directory` return type differences__: The API backend returns raw filenames (e.g., `"latte.md"`, `"subdir/"`), while the filesystem backend returns `.md` stems only (e.g., `"latte"`). This asymmetry is documented; callers will need to normalize. A normalization layer can be added in a future sprint.

3. __Filesystem `append_under_heading` fallback__: The FS backend cannot do heading-aware insertion without a markdown parser. It falls back to plain `append_to_file`. This is documented in the code and acceptable for v1.

4. __`tempfile` crate__: Added as a dev-dependency for filesystem tests. Creates OS-managed temporary directories that auto-clean on drop.

## Readiness for Sprint 3

The transport layer is complete and ready to be consumed. Sprint 3 (Output Rendering) can begin:

- Output modules can use `Transport::create_file` and `Transport::append_under_heading` to write to the vault
- Data fetching modules can use `Transport::list_directory` to populate dynamic dropdowns
- The TUI can display `Transport::mode()` to show the user which backend is active

No blockers. The transport foundation is in place.





