// pour PWA service worker
//
// Tasks implemented here (in dependency order):
//   TASK-2.2.1  /sw.js served at root scope with correct headers (Rust side)
//   TASK-2.2.2  App-shell cache: cache-first for shell, network-only for /api/v1/*
//   TASK-2.2.3  Update flow: new SW → postMessage page → user taps banner → skipWaiting
//   TASK-2.1.2  SW intercepts POST /api/v1/submit/* offline → IDB queue → synthetic 202
//   TASK-2.1.3  Background Sync drains the queue (+ window.online fallback for Safari)
//
// ─── CACHE_VERSION ────────────────────────────────────────────────────────────
// Bump this string whenever ANY file under web/ changes. Do NOT auto-bump from
// build metadata — manual is fine for v1. The bump triggers re-install of the
// SW, prunes the old cache, and causes the update banner to appear on open tabs.
//
// Rule: any change to web/ requires a CACHE_VERSION bump before shipping.
//
// ─── Version history ──────────────────────────────────────────────────────────
//   'v1'  2026-04-27  initial shell cache (TASK-2.2.2)
// ─────────────────────────────────────────────────────────────────────────────
const CACHE_VERSION = 'v1';

// Shell assets pre-cached on install.
// Listed explicitly because web/ has no build manifest — any new file added
// to web/ after this task lands must be added here manually along with a
// CACHE_VERSION bump. This is the asset-drift risk documented in TASK-2.2.2.
const SHELL_ASSETS = [
  '/',
  '/app.js',
  '/queue.js',
  '/styles.css',
  '/manifest.json',
  '/static/icon.svg',
];

const CACHE_NAME = 'pour-shell-' + CACHE_VERSION;

// ─── IndexedDB queue (mirrors web/queue.js, inlined for SW context) ──────────
// The SW runs in a separate global context from the page; it cannot import
// queue.js via <script> tags. The IDB primitives are inlined here.
// These functions are identical in contract to web/queue.js exports.
// Contract §9 (round 5): idempotency_key is generated at QUEUE time and
// REUSED on every drain retry — never rotated by the drain loop.

const IDB_DB_NAME = 'pour-queue';
const IDB_DB_VERSION = 1;
const IDB_STORE = 'pending_submits';

function swOpenQueue() {
  return new Promise((resolve, reject) => {
    const req = self.indexedDB.open(IDB_DB_NAME, IDB_DB_VERSION);
    req.onupgradeneeded = event => {
      const db = event.target.result;
      if (!db.objectStoreNames.contains(IDB_STORE)) {
        const store = db.createObjectStore(IDB_STORE, { keyPath: 'id', autoIncrement: true });
        store.createIndex('module_key', 'module_key', { unique: false });
        store.createIndex('queued_at', 'queued_at', { unique: false });
      }
      // DO NOT deleteObjectStore — data loss = capture loss (manifesto).
    };
    req.onsuccess = e => resolve(e.target.result);
    req.onerror = e => reject(e.target.error);
  });
}

async function swEnqueue(record) {
  const db = await swOpenQueue();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(IDB_STORE, 'readwrite');
    const store = tx.objectStore(IDB_STORE);
    const req = store.add(record);
    req.onsuccess = e => resolve(e.target.result);
    req.onerror = e => reject(e.target.error);
    tx.onerror = e => reject(e.target.error);
  });
}

async function swListQueue() {
  const db = await swOpenQueue();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(IDB_STORE, 'readonly');
    const store = tx.objectStore(IDB_STORE);
    const index = store.index('queued_at');
    const req = index.getAll();
    req.onsuccess = e => {
      const records = e.target.result || [];
      // FIFO: queued_at ASC; tiebreak: id ASC (auto-increment is deterministic)
      records.sort((a, b) => {
        const t = a.queued_at < b.queued_at ? -1 : a.queued_at > b.queued_at ? 1 : 0;
        return t !== 0 ? t : (a.id - b.id);
      });
      resolve(records);
    };
    req.onerror = e => reject(e.target.error);
  });
}

async function swRemoveRecord(id) {
  const db = await swOpenQueue();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(IDB_STORE, 'readwrite');
    const store = tx.objectStore(IDB_STORE);
    const req = store.delete(id);
    req.onsuccess = () => resolve();
    req.onerror = e => reject(e.target.error);
    tx.onerror = e => reject(e.target.error);
  });
}

async function swUpdateRecord(id, patch) {
  const db = await swOpenQueue();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(IDB_STORE, 'readwrite');
    const store = tx.objectStore(IDB_STORE);
    const getReq = store.get(id);
    getReq.onsuccess = e => {
      const record = e.target.result;
      if (!record) { resolve(); return; }
      const updated = Object.assign({}, record, patch);
      const putReq = store.put(updated);
      putReq.onsuccess = () => resolve();
      putReq.onerror = e2 => reject(e2.target.error);
    };
    getReq.onerror = e => reject(e.target.error);
    tx.onerror = e => reject(e.target.error);
  });
}

// ─── postMessage helper — notify all open page clients ───────────────────────
async function notifyClients(payload) {
  const clients = await self.clients.matchAll({ type: 'window' });
  for (const client of clients) {
    client.postMessage(payload);
  }
}

// ─── CRITICAL 4: Fetch current auth token from an open page client ────────────
// NEVER cache the token in SW state or IDB — token rotation must not brick the queue.
// Returns "Bearer <token>" or null if no client is open or none has a token.
// If this returns null, drainQueue defers (does not drain); drain runs again
// when a client opens and fires DRAIN_NOW (the page's online/boot path).
async function getAuthFromClient() {
  const clients = await self.clients.matchAll({ type: 'window', includeUncontrolled: false });
  if (clients.length === 0) return null;

  // Ask the first available client for its current token.
  // The page responds with { type: 'TOKEN_RESPONSE', auth_header: 'Bearer ...' }.
  return new Promise(resolve => {
    const client = clients[0];
    const channel = new MessageChannel();
    const timeout = setTimeout(() => resolve(null), 2000); // 2s timeout
    channel.port1.onmessage = event => {
      clearTimeout(timeout);
      resolve((event.data && event.data.auth_header) || null);
    };
    client.postMessage({ type: 'GET_TOKEN' }, [channel.port2]);
  });
}

// ─── Install ─────────────────────────────────────────────────────────────────
self.addEventListener('install', event => {
  event.waitUntil(
    caches.open(CACHE_NAME)
      .then(cache => cache.addAll(SHELL_ASSETS))
      // Do NOT skipWaiting here — wait for user to tap the update banner
      // (TASK-2.2.3). Auto-claiming mid-form-edit breaks in-flight submits.
  );
});

// ─── Activate ────────────────────────────────────────────────────────────────
self.addEventListener('activate', event => {
  event.waitUntil(
    caches.keys()
      .then(keys => {
        // CRITICAL 3 FIX: detect whether this is a true update (old cache existed)
        // vs a fresh install (no prior pour-shell-* key). Only postMessage SW_UPDATED
        // on a true update — on a fresh install, the update banner is meaningless and
        // confusing ("New version available" with no prior version to compare to).
        const oldCacheKeys = keys.filter(k => k.startsWith('pour-shell-') && k !== CACHE_NAME);
        const isUpdate = oldCacheKeys.length > 0;
        return Promise.all(oldCacheKeys.map(k => caches.delete(k)))
          .then(() => isUpdate);
      })
      .then(isUpdate => {
        // Claim after pruning so this SW controls newly opened tabs.
        // Safe here because activate only fires once the old SW yields.
        return self.clients.claim().then(() => isUpdate);
      })
      .then(isUpdate => {
        // TASK-2.2.3: notify open tabs that a new version is active.
        // Only fires on a true cache-version upgrade, NOT on first install.
        if (isUpdate) {
          notifyClients({ type: 'SW_UPDATED' });
        }
      })
  );
});

// ─── Fetch ───────────────────────────────────────────────────────────────────
self.addEventListener('fetch', event => {
  const { request } = event;
  const url = new URL(request.url);

  // /sw.js — browser fetches this directly, bypass SW interception.
  if (url.pathname === '/sw.js') {
    event.respondWith(fetch(request));
    return;
  }

  // API calls (/api/v1/*) — routing depends on method and path.
  if (url.pathname.startsWith('/api/v1/')) {
    // TASK-2.1.2: intercept POST /api/v1/submit/* for offline queueing.
    if (request.method === 'POST' && url.pathname.startsWith('/api/v1/submit/')) {
      event.respondWith(handleSubmitRequest(request));
      return;
    }
    // All other API calls: network-only, NO cache fallback (contract §12: no-store).
    // SW MUST NOT log request bodies (contract §14).
    event.respondWith(fetch(request));
    return;
  }

  // Shell assets: cache-first, background-revalidate from network.
  event.respondWith(
    caches.open(CACHE_NAME).then(cache =>
      cache.match(request).then(cached => {
        const networkFetch = fetch(request).then(response => {
          if (response && response.status === 200) {
            cache.put(request, response.clone());
          }
          return response;
        }).catch(() => null);
        return cached || networkFetch;
      })
    )
  );
});

// ─── TASK-2.1.2: Submit intercept ────────────────────────────────────────────
// Called for every POST /api/v1/submit/{module} fetch.
// Network reachable + 2xx/4xx: pass through unchanged.
// Network reachable + 5xx: queue and return synthetic 202.
// Network unreachable (fetch throws): queue and return synthetic 202.
//
// Idempotency-Key discipline (contract §9 round 5):
//   The key is read from the request header (page generates it at queue time).
//   It is stored in the IDB record and REUSED on every drain retry without rotation.
//   Rotation happens only on 2xx success (the page rotates it after seeing 201/202-drained).
//
// captured_at (contract §10):
//   Read from the request body and stored verbatim in the IDB record.
//   Replayed unchanged on drain. Never replaced with drain time.
//   The festival case (capture Friday, sync Monday) is the whole point.
//
// 4xx gating (critical for correctness):
//   NEVER queue a 4xx response — those are client-fixable errors (validation_failed,
//   not_found, etc.). Queueing a 4xx would cause the drain to replay a broken submit
//   forever, jamming the queue. Only network failure and 5xx are queueable.
async function handleSubmitRequest(request) {
  // Extract the module key from the URL path for the IDB record.
  const url = new URL(request.url);
  const moduleKey = url.pathname.replace('/api/v1/submit/', '').split('/')[0];

  // Extract the Idempotency-Key from the request headers.
  // Idempotency-Key: generated by the page at queue time; reused on every retry.
  //
  // CRITICAL 4 FIX: Authorization is intentionally NOT stored in IDB.
  // Storing the auth token at queue time means rotated tokens brick all queued
  // records — the drain would replay with a stale Bearer token and get 401s.
  // Instead, drain fetches a fresh token from the open page client at drain time
  // (see drainQueue). If no client is open, drain is deferred.
  const idempotencyKey = request.headers.get('Idempotency-Key') || '';

  // Clone the request body before reading it (Request body can only be consumed once).
  // SW MUST NOT log the body (contract §14).
  let bodyText;
  try {
    bodyText = await request.clone().text();
  } catch (_e) {
    // Body read failed — treat as a transient error, queue with empty captured_at.
    bodyText = '{}';
  }

  let parsedBody;
  try {
    parsedBody = JSON.parse(bodyText);
  } catch (_e) {
    parsedBody = {};
  }

  // captured_at from the request body — preserved for drain replay (contract §10).
  const capturedAt = parsedBody.captured_at || null;

  // Attempt the real network request.
  let response;
  let networkFailed = false;

  try {
    response = await fetch(request.clone());
  } catch (_e) {
    // fetch() rejected — offline or DNS failure.
    networkFailed = true;
  }

  // If we got a response and it's NOT a 5xx, pass it through.
  if (!networkFailed && response) {
    const status = response.status;

    // 2xx or 4xx: pass through to the page unchanged.
    // 4xx MUST NOT be queued — they are client errors the user must fix.
    if (status < 500) {
      return response;
    }
    // 5xx: fall through to queue logic below.
  }

  // ─── Queue the submit ─────────────────────────────────────────────────────
  // NOTE: auth_header is NOT stored — drain fetches a fresh token at drain time
  // to survive token rotation. See drainQueue() for the token-acquisition logic.
  const queuedAt = new Date().toISOString();
  const record = {
    module_key: moduleKey,
    body: parsedBody,            // full submit payload; replayed unchanged on drain
    idempotency_key: idempotencyKey,
    queued_at: queuedAt,
    attempt_count: 0,
    last_error: null,
  };

  let queueId;
  try {
    queueId = await swEnqueue(record);
  } catch (err) {
    // QuotaExceededError: IDB storage full.
    // Return a synthetic error response that the page can detect.
    if (err && err.name === 'QuotaExceededError') {
      const queueFullBody = JSON.stringify({
        error: { code: 'queue_full', message: 'Offline queue is full — clear storage to continue.' }
      });
      return new Response(queueFullBody, {
        status: 507, // Insufficient Storage
        headers: { 'Content-Type': 'application/json; charset=utf-8' },
      });
    }
    // Other IDB error: surface as a generic network error so the page retries.
    throw err;
  }

  // Register Background Sync so the queue drains when network returns.
  // Feature-detect: Safari does not support Background Sync.
  // The page-side window.online fallback (wired in app.js TASK-2.1.3) covers Safari.
  if ('sync' in self.registration) {
    try {
      await self.registration.sync.register('pour-queue-drain');
    } catch (_e) {
      // Sync registration failure is non-fatal; the online event fallback covers it.
    }
  }

  // Notify page clients that a new record was queued (updates badge count).
  notifyClients({ type: 'QUEUED', queue_id: queueId, module_key: moduleKey });

  // Return synthetic 202 to the page (contract §6.4 round-6 amendment).
  // The page treats 202 as "Queued — will sync when online", NOT "Saved".
  // Body shape: { queued: true, queue_id, captured_at }
  const synthetic202 = JSON.stringify({
    queued: true,
    queue_id: queueId,
    captured_at: capturedAt,
  });
  return new Response(synthetic202, {
    status: 202,
    statusText: 'Queued',
    headers: { 'Content-Type': 'application/json; charset=utf-8' },
  });
}

// ─── TASK-2.1.3: Background Sync drain ───────────────────────────────────────
self.addEventListener('sync', event => {
  if (event.tag === 'pour-queue-drain') {
    event.waitUntil(drainQueue());
  }
});

// drainQueue — called on Background Sync event and optionally from the page
// via postMessage when window.online fires (Safari fallback — see app.js).
//
// Drain order: FIFO by queued_at ASC; tiebreak by id ASC (auto-increment).
// Per contract §9 (round 5): Idempotency-Key is REUSED on every retry.
// Per contract §10: captured_at comes from the original body, never drain time.
// Per contract §14: server log for a drained submit is identical to a fresh
//   submit — no "drain" marker. The server does not know this is a drain.
//
// CRITICAL 4: Auth token is obtained fresh from an open page client at drain time.
// If no client is open, drain is deferred — the page will fire DRAIN_NOW when it
// opens (both the online event and the DOMContentLoaded boot path trigger it).
// This means drain-with-no-open-client is intentionally a no-op; the records stay
// in IDB until a client reconnects. This is the correct trade-off: a deferred drain
// is recoverable; a 401 from a stale token bricks every record permanently.
async function drainQueue() {
  // CRITICAL 4: Fetch a fresh auth token before touching any records.
  // If no client is open we cannot get a token — defer until a client opens.
  const authHeader = await getAuthFromClient();
  if (!authHeader) {
    // No open client — do not drain now. The page's DOMContentLoaded and online
    // event handlers will postMessage DRAIN_NOW when a client becomes available.
    return;
  }

  let records;
  try {
    records = await swListQueue();
  } catch (_e) {
    return; // IDB unavailable — nothing to drain
  }

  if (records.length === 0) return;

  // Announce drain start so the page can show "Syncing…" in the queue panel.
  notifyClients({ type: 'DRAIN_STARTED', count: records.length });

  for (const record of records) {
    const { id, module_key, body, idempotency_key, attempt_count } = record;

    let response;
    let networkFailed = false;

    try {
      // Replay the original body verbatim. captured_at is already in body
      // from the original submit — never replaced with drain time (contract §10).
      // The server sees this as an ordinary submit; it does not know it's a drain.
      // authHeader is fresh from the page — NOT the stale value stored at queue time.
      response = await fetch('/api/v1/submit/' + module_key, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json; charset=utf-8',
          'Authorization': authHeader, // CRITICAL 4: fresh token from current page client
          'Idempotency-Key': idempotency_key, // REUSED — never rotated by drain
        },
        body: JSON.stringify(body),
      });
    } catch (_e) {
      networkFailed = true;
    }

    if (networkFailed) {
      // Network error: keep the record, increment attempt count, reschedule.
      await swUpdateRecord(id, { attempt_count: attempt_count + 1 });
      // Re-register sync so the browser tries again when network returns.
      if ('sync' in self.registration) {
        try { await self.registration.sync.register('pour-queue-drain'); } catch (_e) {}
      }
      continue; // try next record; this one stays in the queue
    }

    const status = response.status;

    if (status >= 200 && status < 300) {
      // 2xx success: delete the record and notify the page.
      let responseBody = {};
      try { responseBody = await response.json(); } catch (_e) {}
      await swRemoveRecord(id);
      notifyClients({
        type: 'DRAINED',
        queue_id: id,
        module_key,
        history_id: responseBody.history_id || null,
        vault_path: responseBody.vault_path || null,
      });

    } else if (status >= 400 && status < 500) {
      // 4xx: client-fixable error (validation_failed, not_found, etc.).
      // Keep the record, increment attempt_count, store error code only (§14).
      let errorCode = 'unknown';
      try {
        const errBody = await response.json();
        errorCode = (errBody.error && errBody.error.code) || 'unknown';
      } catch (_e) {}
      await swUpdateRecord(id, {
        attempt_count: attempt_count + 1,
        last_error: errorCode, // CODE only — no user content, no message (§14)
      });
      notifyClients({ type: 'DRAIN_ERROR', queue_id: id, module_key, code: errorCode });
      // Do NOT reschedule: 4xx requires user action (discard or edit via TASK-2.1.5).

    } else {
      // 5xx: server error — keep record, reschedule.
      let errorCode = 'server_error';
      try {
        const errBody = await response.json();
        errorCode = (errBody.error && errBody.error.code) || 'server_error';
      } catch (_e) {}
      await swUpdateRecord(id, {
        attempt_count: attempt_count + 1,
        last_error: errorCode,
      });
      if ('sync' in self.registration) {
        try { await self.registration.sync.register('pour-queue-drain'); } catch (_e) {}
      }
    }
  }

  // CRITICAL 5: Announce drain completion so the page can update sync status.
  // The page's handleSwMessage wires DRAIN_FINISHED to the #queue-sync-status element.
  notifyClients({ type: 'DRAIN_FINISHED' });
}

// ─── Message channel ──────────────────────────────────────────────────────────
// Page → SW messages:
//   { type: 'SKIP_WAITING' }  — user tapped the update banner; activate the new SW.
//   { type: 'DRAIN_NOW' }     — page online event (Safari Background Sync fallback).
//   { type: 'GET_TOKEN' }     — CRITICAL 4: SW asks page for its current auth token;
//                               page replies on event.ports[0] with { auth_header }.
self.addEventListener('message', event => {
  if (!event.data) return;

  if (event.data.type === 'SKIP_WAITING') {
    // TASK-2.2.3: user tapped "New version available — tap to refresh" banner.
    // Only call skipWaiting here (user-initiated), never on install.
    self.skipWaiting();
    return;
  }

  if (event.data.type === 'DRAIN_NOW') {
    // Safari/iOS fallback: page fires this on window.online event.
    // Runs the same drainQueue() as the Background Sync handler.
    // Both paths share the identical function so behavior is identical
    // regardless of whether Background Sync is available.
    // Note: iOS Safari may evict the SW between sessions; the online event
    // is the only reliable signal that the network has returned on that platform.
    // Note: MessageEvent does not have waitUntil() — drainQueue is fire-and-forget
    // from the message handler. The sync event handler uses waitUntil() correctly.
    drainQueue().catch(() => {}); // non-fatal if drain fails; browser will retry on next sync
  }

  // CRITICAL 4: The SW uses a MessageChannel to ask an open page for its current
  // auth token. The page handles GET_TOKEN in handleSwMessage and replies via
  // event.ports[0]. This message is sent by getAuthFromClient() above — the SW
  // does NOT emit GET_TOKEN itself here; this entry point is for completeness only.
  // (The actual GET_TOKEN send is via client.postMessage in getAuthFromClient.)
});
