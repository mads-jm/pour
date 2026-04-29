// pour PWA — offline submit queue (TASK-2.1.1)
//
// IndexedDB schema for the offline submit queue. Exposes a clean async API
// over the native IDB API so sw.js and app.js never touch IDB primitives directly.
//
// ─── Database identity ────────────────────────────────────────────────────────
//   Name:    pour-queue
//   Version: 1
//   Store:   pending_submits
//   Key:     id (auto-increment integer)
//
// ─── Schema versioning rule (DO NOT DELETE pending_submits on upgrade) ────────
// Future schema bumps MUST add new indexes or stores via additive migrations in
// onupgradeneeded. NEVER call deleteObjectStore('pending_submits') or destroy
// existing records. A queued submit is a capture the user made while offline;
// deleting it means data loss — the entire point of the queue is "no capture lost"
// (the Pour manifesto). If the schema must change incompatibly, migrate records
// to the new shape before deleting the old store.
//
// ─── Record shape ─────────────────────────────────────────────────────────────
//   id               auto-increment integer — IDB-assigned, used as tiebreak in drain order
//   module_key       string  — e.g. "coffee"
//   body             object  — the full submit payload (field_values, composite_data,
//                              auto_create_inputs, callout_overrides, callout_titles,
//                              captured_at, client_id). Stored intact; replayed unchanged on drain.
//   idempotency_key  string  — UUIDv4, generated at QUEUE time, REUSED on every drain retry.
//                              Contract §9 (round 5): same key + recoverable error = re-execute.
//                              Rotated only on 2xx success. Never rotated by drain logic.
//   queued_at        string  — ISO 8601 wall-clock timestamp at queue time.
//                              Distinct from body.captured_at (the moment the user tapped Submit).
//   attempt_count    integer — starts at 0; incremented on each failed drain attempt.
//   last_error       string|null — error CODE (not message) from most recent failed drain.
//                              Per contract §14: only machine-readable codes in logs/storage.
//
// ─── Drain order ──────────────────────────────────────────────────────────────
// Primary sort: queued_at ASC (earliest first — FIFO, preserves capture order).
// Tiebreak:     id ASC (auto-increment is deterministic; same-millisecond records
//               are drained in insertion order).
//
// ─── Site-data loss warning ───────────────────────────────────────────────────
// If the user clears site data in the browser, this IDB database is destroyed
// along with any pending records. This is acceptable for v1 (the user chose to
// clear data); a future "warn before clear" treatment is flagged but deferred.

const DB_NAME = 'pour-queue';
const DB_VERSION = 1;
const STORE_NAME = 'pending_submits';

/** Open (or create) the IDB database. Returns a Promise<IDBDatabase>. */
function openQueue() {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION);

    req.onupgradeneeded = event => {
      const db = event.target.result;

      // Create the store only if it doesn't exist (additive — do not drop it).
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        const store = db.createObjectStore(STORE_NAME, {
          keyPath: 'id',
          autoIncrement: true,
        });
        // Index on module_key for per-module filtering (e.g. queue panel).
        store.createIndex('module_key', 'module_key', { unique: false });
        // Index on queued_at for chronological drain order.
        store.createIndex('queued_at', 'queued_at', { unique: false });
      }
      // Future schema bumps: add new indexes/stores HERE. Never delete STORE_NAME.
    };

    req.onsuccess = event => resolve(event.target.result);
    req.onerror = event => reject(event.target.error);
  });
}

/**
 * Enqueue a submit record. Returns the new record's IDB auto-increment id.
 *
 * @param {object} record — Must include: module_key, body, idempotency_key, queued_at.
 *   attempt_count and last_error are defaulted here if absent.
 * @returns {Promise<number>} The auto-assigned id.
 * @throws {DOMException} with name 'QuotaExceededError' if IDB storage is full.
 *   Callers MUST handle this and surface a "queue full" UI state.
 */
async function enqueue(record) {
  const db = await openQueue();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readwrite');
    const store = tx.objectStore(STORE_NAME);
    const entry = {
      module_key: record.module_key,
      body: record.body,
      idempotency_key: record.idempotency_key,
      queued_at: record.queued_at,
      attempt_count: record.attempt_count || 0,
      last_error: record.last_error || null,
    };
    const req = store.add(entry);
    req.onsuccess = event => resolve(event.target.result); // IDB-assigned id
    req.onerror = event => reject(event.target.error);
    tx.onerror = event => reject(event.target.error);
  });
}

/**
 * List all pending records, sorted by queued_at ASC then id ASC (drain order).
 * No field values are exposed in any log line — callers MUST not log record.body.
 *
 * @returns {Promise<Array>} Array of queue record objects (including id).
 */
async function listQueue() {
  const db = await openQueue();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readonly');
    const store = tx.objectStore(STORE_NAME);
    const index = store.index('queued_at');
    const req = index.getAll();
    req.onsuccess = event => {
      const records = event.target.result || [];
      // Primary sort: queued_at ASC. Tiebreak: id ASC.
      records.sort((a, b) => {
        const timeDiff = a.queued_at < b.queued_at ? -1 : a.queued_at > b.queued_at ? 1 : 0;
        return timeDiff !== 0 ? timeDiff : (a.id - b.id);
      });
      resolve(records);
    };
    req.onerror = event => reject(event.target.error);
  });
}

/**
 * Get a single queue record by its IDB id.
 *
 * @param {number} id
 * @returns {Promise<object|undefined>}
 */
async function getQueueRecord(id) {
  const db = await openQueue();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readonly');
    const store = tx.objectStore(STORE_NAME);
    const req = store.get(id);
    req.onsuccess = event => resolve(event.target.result);
    req.onerror = event => reject(event.target.error);
  });
}

/**
 * Remove a queue record by id. Called after a successful 2xx drain.
 *
 * @param {number} id
 * @returns {Promise<void>}
 */
async function removeQueueRecord(id) {
  const db = await openQueue();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readwrite');
    const store = tx.objectStore(STORE_NAME);
    const req = store.delete(id);
    req.onsuccess = () => resolve();
    req.onerror = event => reject(event.target.error);
    tx.onerror = event => reject(event.target.error);
  });
}

/**
 * Update fields on a queue record (used by drain to increment attempt_count
 * and record last_error). Only merges the provided patch keys.
 *
 * @param {number} id
 * @param {object} patch — e.g. { attempt_count: 3, last_error: 'write_error' }
 * @returns {Promise<void>}
 */
async function updateQueueRecord(id, patch) {
  const db = await openQueue();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readwrite');
    const store = tx.objectStore(STORE_NAME);
    const getReq = store.get(id);
    getReq.onsuccess = event => {
      const record = event.target.result;
      if (!record) {
        resolve(); // already removed — no-op
        return;
      }
      const updated = Object.assign({}, record, patch);
      const putReq = store.put(updated);
      putReq.onsuccess = () => resolve();
      putReq.onerror = event => reject(event.target.error);
    };
    getReq.onerror = event => reject(event.target.error);
    tx.onerror = event => reject(event.target.error);
  });
}

/**
 * Count pending records. Exposed so the badge can re-read count without
 * pulling all records (cheap IDB count query).
 *
 * @returns {Promise<number>}
 */
async function countQueue() {
  const db = await openQueue();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readonly');
    const store = tx.objectStore(STORE_NAME);
    const req = store.count();
    req.onsuccess = event => resolve(event.target.result);
    req.onerror = event => reject(event.target.error);
  });
}
