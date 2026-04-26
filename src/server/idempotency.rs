/// In-memory idempotency cache for `POST /api/v1/submit/{module}`.
///
/// Implements contract §9:
/// - Capacity: 1024 entries (LRU eviction when full).
/// - TTL: 5 minutes from completion.
/// - Thread-safe via `std::sync::Mutex`.
///
/// Lifecycle per key:
/// 1. `get_or_insert_in_flight` → `Fresh` (new key) or `Replay` (cached) or
///    `InFlight` (same key is currently being processed).
/// 2. Handler runs.
/// 3. `complete(key, status, body)` → stores the final response; in-flight
///    marker is replaced with the completed entry.
///
/// On `Fresh`, the caller MUST call `complete()` exactly once. Failing to
/// call `complete()` leaks an in-flight marker, but the entry is evicted
/// after TTL anyway so the impact is bounded.
use axum::http::StatusCode;
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const CAPACITY: usize = 1024;
const TTL: Duration = Duration::from_secs(300); // 5 minutes
/// Maximum time an InFlight entry can remain before it is treated as expired.
/// Protects against wedged in-flight markers from dropped futures (panics,
/// request cancellation, OOM during body parsing). 60 seconds is generous —
/// submit handlers should never legitimately take that long.
const IN_FLIGHT_TTL: Duration = Duration::from_secs(60);

/// The outcome of a `get_or_insert_in_flight` call.
pub enum IdempotencyOutcome {
    /// This is a new key — proceed with the handler; call `complete()` when done.
    Fresh,
    /// A prior completed response exists — serve it byte-for-byte.
    Replay {
        status: StatusCode,
        body: Vec<u8>,
    },
    /// The same key is currently in flight (parallel submit race).
    /// Contract: return 409 `idempotency_replay_in_flight`.
    InFlight,
}

enum Entry {
    /// Chosen approach: TTL-based expiry (simpler than RAII guard).
    ///
    /// `started_at` is recorded when the InFlight marker is inserted. On
    /// subsequent lookups, if more than `IN_FLIGHT_TTL` has elapsed the entry
    /// is treated as expired (removed and re-inserted as fresh). This bounds
    /// the impact of dropped futures to at most `IN_FLIGHT_TTL` seconds of
    /// wedging per key, without requiring a new wrapper type.
    InFlight { started_at: Instant },
    Done { status: StatusCode, body: Vec<u8>, completed_at: Instant },
}

pub struct IdempotencyCache {
    inner: Mutex<Inner>,
}

struct Inner {
    map: HashMap<String, Entry>,
    /// Insertion-order queue for LRU eviction. Entries are pushed on
    /// `get_or_insert_in_flight` and removed on eviction when capacity is hit.
    order: VecDeque<String>,
}

impl IdempotencyCache {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                map: HashMap::with_capacity(CAPACITY + 1),
                order: VecDeque::with_capacity(CAPACITY + 1),
            }),
        }
    }

    /// Look up `key` and either:
    /// - Return `Fresh` (inserting an in-flight marker), or
    /// - Return `Replay` / `InFlight` if the key already exists.
    pub fn get_or_insert_in_flight(&self, key: &str) -> IdempotencyOutcome {
        let mut g = self.inner.lock().unwrap();

        if let Some(entry) = g.map.get(key) {
            let now = Instant::now();
            match entry {
                Entry::InFlight { started_at } => {
                    if now.duration_since(*started_at) < IN_FLIGHT_TTL {
                        // Still within the in-flight window — genuine parallel request.
                        return IdempotencyOutcome::InFlight;
                    }
                    // Expired in-flight marker (dropped future, panic, cancellation).
                    // Fall through and treat as a fresh request.
                }
                Entry::Done { status, body, completed_at } => {
                    if now.duration_since(*completed_at) < TTL {
                        return IdempotencyOutcome::Replay {
                            status: *status,
                            body: body.clone(),
                        };
                    }
                    // Expired — fall through and treat as fresh.
                    // Remove old order entry below.
                }
            }
        }

        // Evict oldest entry if at capacity.
        while g.order.len() >= CAPACITY {
            if let Some(oldest) = g.order.pop_front() {
                g.map.remove(&oldest);
            }
        }

        g.map.insert(key.to_string(), Entry::InFlight { started_at: Instant::now() });
        // Only add to order if this is a genuinely new key (not an expired re-insert).
        if !g.order.contains(&key.to_string()) {
            g.order.push_back(key.to_string());
        }

        IdempotencyOutcome::Fresh
    }

    /// Record the completed response for `key`, replacing the in-flight marker.
    ///
    /// No-op if the key is absent (shouldn't happen in normal usage).
    pub fn complete(&self, key: &str, status: StatusCode, body: Vec<u8>) {
        let mut g = self.inner.lock().unwrap();
        if g.map.contains_key(key) {
            g.map.insert(
                key.to_string(),
                Entry::Done {
                    status,
                    body,
                    completed_at: Instant::now(),
                },
            );
        }
    }
}

impl Default for IdempotencyCache {
    fn default() -> Self {
        Self::new()
    }
}

impl IdempotencyCache {
    /// Test helper: insert an InFlight entry whose `started_at` is backdated
    /// by `age` so that TTL expiry can be verified without sleeping.
    ///
    /// Intended for integration tests only. Not called in production paths.
    pub fn insert_stale_in_flight(&self, key: &str, age: Duration) {
        let mut g = self.inner.lock().unwrap();
        let started_at = Instant::now().checked_sub(age).expect("age fits in Instant");
        g.map.insert(key.to_string(), Entry::InFlight { started_at });
        if !g.order.contains(&key.to_string()) {
            g.order.push_back(key.to_string());
        }
    }
}
