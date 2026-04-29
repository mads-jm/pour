# Pour TUI ↔ Serve Handoff

## Goal

From inside the TUI dashboard, press `s` to suspend the TUI, run the PWA server inline (QR code + URL visible in the terminal), and return to the dashboard when the server stops. One process, one terminal, transient server.

**Use case**: User is at desk in TUI, needs to step away (brew coffee), wants to capture from phone while away. Today they have to spawn a new terminal or quit the TUI. After this change: press `s`, walk away with phone, come back, press Ctrl+C, resume capturing.

## Non-Goals

- Background mode. The server does not survive a TUI quit, and the dashboard is not interactive while the server is running. (If a user needs a long-lived server, they run `pour serve` in a dedicated terminal or container — that path is unchanged.)
- Detached process / daemon supervision. No PID files, no respawn.
- Multi-server-instance coordination. If `pour serve` is already running externally, the TUI handoff detects the conflict and refuses with a clear message.

## UX

**Dashboard**: footer gains `[s] Serve`. Pressing `s` from `Screen::Dashboard` triggers handoff.

**Serve screen** (cooked terminal, not ratatui):
- Same banner that `pour serve` prints today: QR code, URL, transport mode, listening address.
- Plus a hint line: `Press Ctrl+C to stop and return to the dashboard.`
- After Ctrl+C: prints `Stopping…` then `Server stopped` then re-enters the TUI.

**Error paths**:
- Port in use → print one-line error (`port 8421 already in use — is \`pour serve\` running elsewhere?`), stay on dashboard.
- LAN IP detection failure → same warning banner as `pour serve` today (already covers this).
- Server task panic → terminal restoration via existing panic hook; surface error after TUI restored.

## Architecture

### Lifecycle (clean)

```
Dashboard
  ↓ press 's'
Action::Serve
  ↓
ratatui::restore()         ← leave alt-screen, exit raw mode
print_banner(...)          ← QR + URL + token, shared with `pour serve`
bind probe (port-in-use?)  ← fail fast with clear msg if conflict
run_with_shutdown(ctrl_c)  ← graceful axum shutdown on SIGINT
  ↓ user hits Ctrl+C
graceful drain (5s budget) ← bounded; force-exit task on timeout
*terminal = ratatui::init()
terminal.clear()
  ↓
Dashboard (resumed)
```

### Cancellable server (the core refactor)

`src/server/mod.rs` currently has:

```rust
pub async fn run(config: Config, transport: Transport, port: u16, token: String) -> Result<()> {
    // ...
    serve_on_listener(listener, state).await
}
```

`serve_on_listener` calls `axum::serve(listener, app).await` — no shutdown channel. The CLI relies on an external SIGINT to terminate the whole process.

**New shape**:

```rust
pub async fn run_with_shutdown<F>(
    config: Config,
    transport: Transport,
    port: u16,
    token: String,
    shutdown: F,
) -> Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    // ... existing setup ...
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, build_app(state))
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}

pub async fn run(config: Config, transport: Transport, port: u16, token: String) -> Result<()> {
    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    run_with_shutdown(config, transport, port, token, shutdown).await
}
```

The TUI handoff calls `run_with_shutdown` with its own ctrl_c future, wrapped in a 5-second timeout (see "bounded drain" below). The CLI path is unchanged in behavior — `run` is preserved as the public entry and just delegates.

`serve_on_listener` keeps its current shape (no shutdown) for integration tests that already drive ephemeral-port servers; we don't need to touch it.

### Bounded drain

Graceful shutdown can hang if a client holds a connection open. Bound it:

```rust
let server_fut = run_with_shutdown(config, transport, port, token, shutdown);
match tokio::time::timeout(Duration::from_secs(5), server_fut).await {
    Ok(Ok(())) => { /* clean */ }
    Ok(Err(e)) => eprintln!("pour serve: {e}"),
    Err(_) => eprintln!("pour serve: server did not drain within 5s; forcing exit"),
}
```

The TUI handoff applies this wrapping; the `pour serve` CLI path keeps blocking indefinitely on Ctrl+C (current behavior).

### Banner extraction

Move `main.rs:117–185` into `src/server/startup.rs`:

```rust
pub struct StartupContext {
    pub port: u16,
    pub token: String,
    pub transport_mode: TransportMode,
}

/// Resolve or generate the mobile token, persisting if newly generated.
/// Same precedence as today: POUR_MOBILE_TOKEN env > secrets.toml > generate.
pub fn resolve_token() -> String { /* ... */ }

/// Print the startup banner (QR + URL + transport) to stdout/stderr.
/// Behavior matches `pour serve` today — same warning banner on LAN-IP
/// detection failure.
pub fn print_banner(ctx: &StartupContext) { /* ... */ }
```

Both `pour serve` (CLI) and the TUI handoff call these. `pour serve` continues to print to its own stdout; the TUI calls them after `ratatui::restore()` so output goes to the cooked terminal.

### Port collision detection

Before kicking off the banner + server in the TUI handoff:

```rust
match tokio::net::TcpListener::bind(("0.0.0.0", port)).await {
    Ok(listener) => drop(listener), // close immediately; server will rebind
    Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
        // surface to dashboard, stay in TUI
        return ServeHandoffResult::PortInUse(port);
    }
    Err(e) => return ServeHandoffResult::BindError(e),
}
```

There is a tiny TOCTOU window between the probe drop and the server's rebind. In practice this is fine — port collisions on this machine come from a long-lived `pour serve` elsewhere, not a race. If we want to eliminate the gap, we can pass the probed listener directly into `serve_on_listener` instead. **Decision**: pass the listener through to remove the race; minor signature tweak.

### Terminal handoff

`run_loop` owns `terminal: &mut ratatui::DefaultTerminal`. We can replace through the mut ref:

```rust
tui::Action::Serve => {
    ratatui::restore();
    let result = serve_handoff(/* ... */).await;
    *terminal = ratatui::init();
    terminal.clear()?;
    if let Err(msg) = result {
        // surface a transient overlay, or stash in app.deferred_stderr
        app.deferred_stderr.push(format!("Serve: {msg}"));
    }
}
```

The existing panic hook (`main.rs:251–255`) calls `ratatui::restore()`, which is idempotent — safe whether we're inside ratatui or not at the moment of panic. No new safety code needed.

## Files Changed

- `src/server/mod.rs` — add `run_with_shutdown`; `run` delegates to it.
- `src/server/startup.rs` — **new file**. `StartupContext`, `resolve_token`, `print_banner`. Re-exported from `src/server/mod.rs`.
- `src/main.rs`:
  - `pour serve` branch (lines 50–192) shrinks to: parse port → load config → connect transport → `print_banner` → `server::run`. Token resolution moves to `startup::resolve_token`.
  - `run_loop` gains an `Action::Serve` arm that performs the handoff.
- `src/tui/mod.rs` — add `Action::Serve` variant; wire dashboard arm.
- `src/tui/dashboard.rs`:
  - Add `DashboardAction::Serve`.
  - Bind `Char('s')` → `DashboardAction::Serve`.
  - Update footer hint string to include `[s] Serve`.
- `tests/server_shutdown.rs` — **new test file**. Verify `run_with_shutdown` exits cleanly when the shutdown future fires; verify the listener is freed afterward.

## Tests

- **Unit / integration**: `run_with_shutdown` on a port-0 listener exits within 1s when the shutdown oneshot fires.
- **Unit**: `startup::resolve_token` precedence (env > secrets.toml > generate) — token persistence path covered by existing tests if any; add coverage if missing.
- **Manual** (TUI handoff cannot be unit-tested cleanly):
  1. `cargo run` → press `s` → confirm QR appears, terminal is cooked.
  2. Submit a capture from the PWA on phone → confirm it lands in the vault.
  3. Press Ctrl+C → confirm "Stopping…" → "Server stopped" → dashboard reappears.
  4. Press `s` again → confirm same token (reused from secrets.toml).
  5. In a second terminal, `pour serve` → in TUI, press `s` → confirm clean error, dashboard stays.
  6. Quit TUI with `q` while server running (separate flow): not applicable — the dashboard is suspended during serve, so this can't happen by design.

## Risks & Mitigations

| Risk | Mitigation |
|---|---|
| Server task hangs in graceful shutdown | 5s timeout, then drop the future. Connection drop is acceptable for transient mode. |
| Port-in-use TOCTOU between probe and bind | Pass the probed listener directly into the server (eliminates the race). |
| Terminal left in raw mode on panic during serve | Existing panic hook calls `ratatui::restore()`; serve runs in cooked mode so no extra hook needed. |
| Ctrl+C in serve mode also kills the TUI process | Use `with_graceful_shutdown` instead of letting the default tokio signal handler propagate. The shutdown future awaits `ctrl_c()` and resolves; signal does not exit the process. |
| Token churn between CLI and TUI | Both paths call `startup::resolve_token`; same secrets.toml semantics. No churn. |
| `serve_on_listener` divergence from `run_with_shutdown` | Keep `serve_on_listener` for tests; `run_with_shutdown` builds its own listener. Both call `build_app(state)` — single source of truth for the router. |

## Out of Scope (follow-ups, not blocking)

- Auto-reconnect / "keep server warm" if the dashboard exits to fast-path.
- Showing live request log inside the serve screen.
- A `pour serve --tui` mode that boots straight into serve (probably not needed).

## Open Questions

None blocking. Resolved during planning:

- **Stop key**: Ctrl+C (cooked mode, natural fit). No competing key needed.
- **Token reuse**: Yes — same `read_mobile_token` / `write_mobile_token` precedence as `pour serve`.
- **Drain timeout**: 5s. Below user-perceptible irritation, above a typical request lifetime.
- **TOCTOU on port probe**: Eliminate by passing the probed listener through.

## Implementation Order

1. Server refactor: `run_with_shutdown` + `run` delegate. Test it.
2. Banner extraction: `src/server/startup.rs`. CLI path uses it; behavior unchanged.
3. TUI wiring: `Action::Serve`, `DashboardAction::Serve`, `Char('s')`, footer hint.
4. Handoff in `run_loop`: restore → probe → serve → re-init.
5. Bounded drain timeout.
6. Manual test pass.
7. Doc updates: `04 architecture/System-Architecture-Overview.md` (note the suspend/resume flow), `README.md` (mention `s` keybinding), this file marked as implemented.

## Implementation Notes

**Status: Implemented 2026-04-29.**

### Divergences from plan

1. **`run_with_shutdown` signature**: The plan showed `run_with_shutdown` building the listener internally. The implemented signature accepts a pre-bound `TcpListener` from the caller — this is actually the design the plan recommended in "Port collision detection" (pass the probed listener through to remove the TOCTOU race). The plan's code snippet was aspirational; the implementation follows the prose recommendation.

2. **`run` delegate shutdown signal**: The plan showed `tokio::signal::ctrl_c()` in the `run` delegate. The `signal` tokio feature was not previously enabled. Rather than add the feature just for the CLI path (which already worked via OS-level SIGINT), `run` delegates with `std::future::pending()` — the server blocks forever and the OS kills the process on Ctrl+C, which is identical to the original behavior. The `signal` feature was added to support `ctrl_c()` in the TUI handoff path.

3. **"Stopping…" timing**: The plan says "prints `Stopping…`" after Ctrl+C. Because `run_with_shutdown` is a single await that resolves only after the drain completes, there is no hook point between the signal firing and the drain completing. "Stopping…" is printed as the first line of the match arm after the timeout block resolves (i.e., after the drain), immediately before returning to the TUI. Functionally equivalent from the user's perspective.

4. **Banner styling**: The URL line in the styled box uses `{url:<63}` padding. URLs longer than 63 chars (e.g. long LAN IPs + long tokens) will overflow the box. This is the same trade-off the original code accepted; a `--host` flag (noted as TODO) is the long-term fix.

5. **`tokio::time` feature**: Added alongside `signal` — `tokio::time::timeout` was already used in `server_shutdown` tests and implied by the plan's 5s drain timeout, but the feature was not previously declared.

---

### Adversarial audit fixes (2026-04-29)

**C1 — `deferred_stderr` dead drop fixed**: Serve-handoff errors (port-in-use, bind error, drain timeout, server error) are now pushed to `app.startup_warnings` instead of `app.deferred_stderr`. `startup_warnings` is rendered as a dismissable overlay on every dashboard draw, so the user sees the error immediately on return.

**C2 — `ctrl_c()` error swallowing fixed (TUI path)**: The TUI handoff no longer calls `ctrl_c()` inside a `let _ = …` future. Instead, the shutdown signal is delivered via a `tokio::sync::oneshot` channel fed by a spawned task (see C3). If the signal task fails, the server runs until the 5s drain timeout, which is the documented safe fallback. The CLI `run` path was already safe (uses `std::future::pending()`).

**C3 — Pre-poll Ctrl+C window closed**: The `tokio::spawn` that registers `tokio::signal::ctrl_c()` is called at the very top of the `Action::Serve` arm, before `ratatui::restore()` or any setup work. The unprotected window between "user presses `s`" and "signal handler installed" now spans only the microseconds it takes for `tokio::spawn` to enqueue the task, not the hundreds of milliseconds of banner print + TCP bind.

**C4 — Banner widths fixed**: `src/server/startup.rs` introduces `BANNER_WIDTH = 70` and `BANNER_INNER = 68`. All box lines are constructed via the new `build_banner_box` function which pads each line exactly to `BANNER_INNER` visible chars (including the two `│` borders = 70 total). URL truncation uses a `truncate_url` helper that counts by `chars()` and appends a single-char `…` (U+2026) on overflow, avoiding the byte-length padding bug. Regression test: `tests/server_startup_banner.rs`.

**H1 — Config/transport reuse**: `app.config.clone()` is used instead of `Config::load()` for the serve handoff. `Config`, `VaultConfig`, `ModuleConfig`, `WriteMode`, `FieldConfig`, and `FieldType` all gained `#[derive(Clone)]`. A fresh `Transport::connect` is called from the cloned config (cannot move `app.transport` while `app` is mutably borrowed), but it uses the same config snapshot as the TUI — no more divergence.

**H2 — `run_with_shutdown` log line fixed**: The `tracing::info!` call now uses `listener.local_addr()` to derive the actual bound address, so port-0 tests log the correct OS-assigned address instead of `0.0.0.0:0`.

**H3 — Drain timeout tests added**: `tests/server_shutdown.rs` gains two new tests:
- `shutdown_with_no_connections` — fires shutdown immediately, asserts exit within 500ms.
- `shutdown_drains_in_flight_request` — fires shutdown 100ms into an active request, asserts the request completes (200) and the server exits within 600ms total.

**H4 — Keypress buffer race fixed**: The `Action::Serve` arm now ends with `break;` to discard any remaining keys in the current poll batch. A comment explains why: keys buffered between `s` and the batch boundary would otherwise be applied against the freshly re-initialised dashboard.

### Manual test checklist (for Joseph)

From the plan §"Tests":
1. `cargo run` → press `s` → confirm QR appears, terminal is cooked.
2. Submit a capture from the PWA on phone → confirm it lands in the vault.
3. Press Ctrl+C → confirm "Stopping…\nServer stopped." → dashboard reappears.
4. Press `s` again → confirm same token (reused from secrets.toml).
5. In a second terminal, `pour serve` → in TUI, press `s` → confirm port-in-use warning overlay on dashboard (not a lost error).
