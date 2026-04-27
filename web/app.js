// Pour PWA — vanilla JS, no framework, no build step.
// ~420 lines. Modules via plain functions.
//
// SECURITY CONVENTION: All user-derived strings injected via innerHTML must pass
// through escapeHtml(). Prefer textContent / createElement for new code.
//
// ---------------------------------------------------------------------------
// Phase 2 Stream D — History view + heatmap (TASK-2.5.1 through 2.5.6)
// ---------------------------------------------------------------------------
//
// HEATMAP DATA SOURCE DECISION (TASK-2.5.5):
//   Path (a) — client-side rollup from /api/v1/history.
//   Rationale: zero contract cost. No new endpoint needed. The summary object
//   returned by the first (no-cursor) call covers today/week/streak. The
//   heatmap rollup fetches up to HEATMAP_DAYS of entries paginated at
//   HEATMAP_FETCH_LIMIT per page to build a date→count map client-side.
//
// CURSOR PAGINATION (contract §6.5 — millisecond-collision warning):
//   We ALWAYS use the server's `next_cursor` field for pagination.
//   We NEVER derive a cursor from `entries[last].timestamp`.
//   Reason: offline-queue replay can produce same-millisecond entries. A
//   timestamp-only cursor would silently drop them at the page boundary.
//   The id-based cursor is exact and immune to millisecond collisions.
//
// HEATMAP LOOP TERMINATION:
//   The heatmap fetch loop runs until:
//     (a) has_more === false (no more pages), OR
//     (b) the oldest entry on the last page is beyond the HEATMAP_DAYS window
//         (all remaining entries are older than the window — no point fetching), OR
//     (c) a fetch error occurs (loop aborts, renders with partial data), OR
//     (d) CRITICAL-1 guard: entries.length === 0 on a page that claims has_more
//         (malformed server response — empty page + has_more=true would loop
//         forever; we abort and render with partial data), OR
//     (e) CRITICAL-1 guard: iteration counter exceeds HEATMAP_MAX_PAGES.
//         50 pages × 1000 entries = 50 000 captures; ample for any realistic vault.
//   Each page uses ?limit=HEATMAP_FETCH_LIMIT (1000). Covers ~90 days of
//   10 captures/day in a single request for typical usage.
//
// HEATMAP_DAYS is a module-level constant. NOT user-configurable in v1.
const HEATMAP_DAYS = 90;
// Limit per page for the heatmap rollup fetch.
const HEATMAP_FETCH_LIMIT = 1000;
// Hard cap on heatmap pagination iterations (CRITICAL-1 fix).
// 50 × 1000 = 50 000 captures — sufficient for any realistic vault.
const HEATMAP_MAX_PAGES = 50;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

let _token = null;
let _config = null;          // full /api/v1/config response
let _currentModule = null;   // module key string
let _currentModuleData = null; // module object from config
let _optionsCache = {};      // { "module:field": [...] }
let _presetsCache = [];      // preset objects for current module
let _activePreset = null;    // name of currently applied preset, or null

// idempotency-key persists across retries within a form session.
// Generated once on first submit; reused on any retry (5xx / network error).
// Rotated to a fresh key only after a 2xx success OR explicit form reset
// (navigating away, opening a new module, "Pour Another" tap).
let _pendingIdempotencyKey = null;

// ---------------------------------------------------------------------------
// Phase 2 Stream A — Offline queue + service worker state
// ---------------------------------------------------------------------------

// Count of pending IDB records, drives the "Queued (n)" badge.
// Refreshed on init (reads IDB) and updated via SW postMessages so the badge
// stays live without polling.
let _queueCount = 0;

// Whether the queue panel is currently expanded.
let _queuePanelOpen = false;

// ---------------------------------------------------------------------------
// Phase 2 Stream B — Sub-form overlay state
// ---------------------------------------------------------------------------

// Map of parent-field-name → { template-field-name: value, ... }
// Populated by overlay confirm; cleared on form reset or after 2xx.
// On parent submit, included as auto_create_inputs when non-empty.
let _pendingAutoCreateInputs = {};

// Overlay context — set when openSubformOverlay() is called.
let _overlayContext = null;
// {
//   fieldName: string,          parent dynamic_select field name
//   templateName: string,       template key from config
//   novelValue: string,         the typed parent value at open-time
//   parentValueAtOpen: string,  parent input value snapshot for cancel-revert
//   focusReturnEl: HTMLElement, element to focus on close
// }

// ---------------------------------------------------------------------------
// Token bootstrap (contract §3)
// ---------------------------------------------------------------------------

function getToken() {
  if (_token) return _token;

  // 1. Check URL ?token= (QR-code first-visit bootstrap)
  const params = new URLSearchParams(window.location.search);
  const urlToken = params.get("token");
  if (urlToken) {
    localStorage.setItem("pour_token", urlToken);
    // Remove token from URL so it doesn't linger in browser history
    params.delete("token");
    const newSearch = params.toString();
    const newUrl = window.location.pathname + (newSearch ? "?" + newSearch : "");
    history.replaceState(null, "", newUrl);
    _token = urlToken;
    return _token;
  }

  // 2. Check localStorage
  const stored = localStorage.getItem("pour_token");
  if (stored) { _token = stored; return _token; }

  return null;
}

// ---------------------------------------------------------------------------
// API helpers
// ---------------------------------------------------------------------------

async function apiFetch(path, opts = {}) {
  const token = getToken();
  if (!token) {
    showView("token-gate");
    throw new Error("No auth token");
  }
  const headers = Object.assign({}, opts.headers || {}, {
    "Authorization": "Bearer " + token,
  });
  if (opts.body) headers["Content-Type"] = "application/json; charset=utf-8";
  const resp = await fetch(path, { ...opts, headers });
  // 401 anywhere → token is stale; clear and force re-bootstrap
  if (resp.status === 401) {
    _token = null;
    localStorage.removeItem("pour_token");
    showView("token-gate");
    throw new Error("Unauthorized — re-scan QR code");
  }
  return resp;
}

// ---------------------------------------------------------------------------
// View switching
// ---------------------------------------------------------------------------

// Views where the bottom-tab nav must be HIDDEN:
//   - form: sticky submit bar owns the bottom
//   - summary: no navigation needed mid-flow
//   - token-gate: no auth yet
// Views where bottom-tab nav is SHOWN:
//   - dashboard (capture tab)
//   - history (history tab)
const VIEWS_WITHOUT_TAB_NAV = new Set(["form", "summary", "token-gate"]);

function showView(id) {
  for (const v of ["token-gate", "dashboard", "form", "summary", "history"]) {
    const el = document.getElementById(v);
    if (el) el.hidden = (v !== id);
  }

  // Bottom-tab nav visibility
  const nav = document.getElementById("bottom-tab-nav");
  if (nav) {
    if (VIEWS_WITHOUT_TAB_NAV.has(id)) {
      nav.classList.add("tab-nav--hidden");
    } else {
      nav.classList.remove("tab-nav--hidden");
    }
  }

  // Update aria-selected on tab buttons to match the active view.
  // dashboard ↔ capture tab; history ↔ history tab.
  const tabCapture = document.getElementById("tab-capture");
  const tabHistory = document.getElementById("tab-history");
  if (tabCapture) tabCapture.setAttribute("aria-selected", id === "dashboard" ? "true" : "false");
  if (tabHistory) tabHistory.setAttribute("aria-selected", id === "history" ? "true" : "false");
}

// ---------------------------------------------------------------------------
// Phase 2 Stream B — Sub-form overlay (TASK-2.3.1 through TASK-2.3.5)
// ---------------------------------------------------------------------------

// isNovelValue: client-side mirror of src/autocreate.rs is_existing_option().
//
// Server logic (autocreate.rs line 81-84):
//   pub fn is_existing_option(value: &str, options: &[String]) -> bool {
//     let lower = value.trim().to_lowercase();
//     options.iter().any(|o| o.trim().to_lowercase() == lower)
//   }
//
// The server uses CASE-INSENSITIVE comparison after trim(). This function
// mirrors that exactly. NOTE: the locked backlog spec said "case-sensitive",
// but the server implementation is case-insensitive — we mirror the server
// to avoid a mismatched-novelty check that would open duplicate-write hazards.
// Flagged in submission for review.
function isNovelValue(value, options) {
  const lower = value.trim().toLowerCase();
  if (lower === "") return false; // empty is not a novel value — it's just empty
  return !options.some(o => o.trim().toLowerCase() === lower);
}

// Open the sub-form overlay for a parent dynamic_select field that has
// create_template set and whose current typed value is novel.
//
// fieldName: parent field name (e.g. "bean")
// templateName: template key from config (e.g. "bean")
// novelValue: the typed string the user entered
function openSubformOverlay(fieldName, templateName, novelValue) {
  const template = (_config.templates || {})[templateName];
  if (!template) {
    // Template missing — server will 400 us, but we can't render the overlay.
    // Fall through to submit; server error will be surfaced via toast.
    return;
  }

  // Snapshot the parent input value for cancel-revert.
  const parentInput = document.getElementById("field-" + fieldName);
  const parentValueAtOpen = parentInput ? parentInput.value : "";

  // Disable the parent input while overlay is up — prevents editing behind it.
  if (parentInput) parentInput.disabled = true;

  _overlayContext = {
    fieldName,
    templateName,
    novelValue,
    parentValueAtOpen,
    focusReturnEl: parentInput || document.getElementById("field-" + fieldName),
  };

  // Header: "Create bean: 'Ethiopia Guji'"
  const titleEl = document.getElementById("subform-title");
  // Use the parent field prompt if available, else field name
  const parentField = (_currentModuleData ? (_currentModuleData.fields || []) : [])
    .find(f => f.name === fieldName);
  const label = (parentField && parentField.prompt) || fieldName;
  titleEl.textContent = "Create " + label;

  const novelEl = document.getElementById("subform-novel-value");
  novelEl.textContent = "‘" + novelValue + "’"; // left/right single quotes

  // Clear previous errors
  const topError = document.getElementById("subform-top-error");
  topError.textContent = "";
  topError.hidden = true;

  // Render template fields
  renderSubformFields(template);

  // Show overlay
  const overlay = document.getElementById("subform-overlay");
  overlay.hidden = false;
  overlay.setAttribute("aria-hidden", "false");

  // Trap focus: first focusable element in the panel
  const firstFocusable = overlay.querySelector(
    "button, input, select, textarea, [tabindex]"
  );
  if (firstFocusable) firstFocusable.focus();
}

// Dismiss the overlay (used by cancel and after confirm).
// Does NOT restore parent input value — callers do that before calling.
function closeSubformOverlay() {
  const overlay = document.getElementById("subform-overlay");
  overlay.hidden = true;
  overlay.setAttribute("aria-hidden", "true");

  // Re-enable the parent input
  if (_overlayContext) {
    const parentInput = document.getElementById("field-" + _overlayContext.fieldName);
    if (parentInput) parentInput.disabled = false;
  }

  // Return focus to originating element
  if (_overlayContext && _overlayContext.focusReturnEl) {
    _overlayContext.focusReturnEl.focus();
  }

  _overlayContext = null;
}

// TASK-2.3.4 — Cancel: revert parent input value, clear pending map entry.
function cancelSubform() {
  if (!_overlayContext) return;
  const { fieldName, parentValueAtOpen } = _overlayContext;

  // Revert the parent field value to what it was at open-time
  const parentInput = document.getElementById("field-" + fieldName);
  if (parentInput) {
    parentInput.value = parentValueAtOpen;
    // Re-enable before close so we can focus it
    parentInput.disabled = false;
  }

  // Drop any accumulated inputs for this field
  delete _pendingAutoCreateInputs[fieldName];

  // Clear focusReturnEl so closeSubformOverlay doesn't re-disable parent
  const focusEl = _overlayContext.focusReturnEl;
  _overlayContext.focusReturnEl = null;
  closeSubformOverlay();

  if (focusEl) focusEl.focus();
}

// TASK-2.3.3 — Confirm: validate, accumulate _pendingAutoCreateInputs, close.
function confirmSubform() {
  if (!_overlayContext) return;
  const { fieldName, templateName, novelValue } = _overlayContext;

  const template = (_config.templates || {})[templateName];
  if (!template) {
    closeSubformOverlay();
    return;
  }

  // Validate required template fields (per-field error pills)
  let hasError = false;
  const inputs = {};
  for (const tf of (template.fields || [])) {
    const el = getSubformFieldValue(tf);
    const val = el !== null ? el : "";
    // Empty string is the contract §6.4 convention — send "", not null/omit.
    inputs[tf.name] = val;

    // Only text/number have explicit required. Template field spec (§7.4) says
    // required is not present in the template field config object, so we only
    // block on a required:true that is present.
    //
    // MAJOR 3 fix: for static_select with required=true and no configured default,
    // options[0] is displayed as a visual hint but the user has not "selected" it.
    // Read the _userInteracted flag — if false, treat the value as empty for required
    // validation so the user is forced to explicitly cycle to a value.
    let effectiveVal = val;
    if (tf.required && tf.field_type === "static_select" && !tf.default) {
      const wrapperId = "sf-field-" + tf.name + "-wrapper";
      const wrapper = document.getElementById(wrapperId);
      if (wrapper && wrapper.dataset.userInteracted !== "true") {
        effectiveVal = "";
      }
    }
    if (tf.required && !effectiveVal.trim()) {
      showSubformFieldError(tf.name, "Required");
      hasError = true;
    }
  }

  if (hasError) return;

  // Accumulate: parent field name → template field values map.
  // If the user earlier confirmed a different novel value for this field,
  // the previous entry is overwritten by this confirm.
  _pendingAutoCreateInputs[fieldName] = inputs;

  // The parent field value stays as the typed novel value (already in the input).
  closeSubformOverlay();
}

// Read a single subform field's current value.
// Returns the string value, or "" if the element is missing.
function getSubformFieldValue(templateField) {
  const id = "sf-field-" + templateField.name;
  if (templateField.field_type === "static_select") {
    // Inline cycling: read data attribute
    const wrapper = document.getElementById(id + "-wrapper");
    if (wrapper) return wrapper.dataset.currentValue || "";
    return "";
  }
  const el = document.getElementById(id);
  return el ? el.value : "";
}

// TASK-2.3.2 — Render template fields inside the overlay.
// Supports: text, number, static_select (inline cycling with ◂ ▸).
// NO <select> elements per memory feedback_subform_ux.md.
function renderSubformFields(template) {
  const container = document.getElementById("subform-fields");
  container.innerHTML = "";

  const fields = template.fields || [];

  for (let idx = 0; idx < fields.length; idx++) {
    const tf = fields[idx];

    const group = document.createElement("div");
    group.className = "field-group";
    group.dataset.sfField = tf.name;

    const label = document.createElement("label");
    // For text/number, point at the input id. For cycling, point at wrapper.
    label.setAttribute(
      "for",
      tf.field_type === "static_select"
        ? "sf-field-" + tf.name + "-wrapper"
        : "sf-field-" + tf.name
    );
    label.textContent = tf.prompt || tf.name;
    group.appendChild(label);

    const input = buildSubformFieldInput(tf, idx, fields.length);
    group.appendChild(input);

    const errEl = document.createElement("div");
    errEl.className = "field-error";
    errEl.id = "sf-err-" + tf.name;
    errEl.setAttribute("role", "alert");
    errEl.hidden = true;
    group.appendChild(errEl);

    container.appendChild(group);
  }

  // Wire Up/Down navigation AFTER all fields exist in DOM.
  // Up/Down move focus to previous/next field in tab order (per TASK-2.3.2).
  wireSubformNavigation(fields);
}

// Build the input element for a single template field.
// idx and totalFields are used by the Up/Down key handler for boundary checking.
function buildSubformFieldInput(tf, idx, totalFields) {
  const defaultVal = tf.default || "";

  if (tf.field_type === "static_select") {
    return buildInlineCycleControl(tf, defaultVal);
  }

  // text or number — standard input
  const inp = document.createElement("input");
  inp.type = tf.field_type === "number" ? "number" : "text";
  if (tf.field_type === "number") {
    inp.inputMode = "decimal";
    inp.step = "any";
  }
  inp.id = "sf-field-" + tf.name;
  inp.name = tf.name;
  inp.value = defaultVal;

  // Left/Right on text/number: native caret movement (do not intercept).
  // Up/Down: handled by wireSubformNavigation.

  return inp;
}

// Build an inline-cycling control for static_select template fields.
//
// A11y approach (per locked decisions):
//   role="combobox" + aria-haspopup="listbox" on wrapper div.
//   aria-activedescendant points at the focused option li.
//   ◂ ▸ buttons are aria-hidden; actual options in hidden <ul role="listbox">.
//   Buttons have aria-label="Previous option" / "Next option".
//   Left/Right cycle; Enter/Space no-op (value is always selected).
function buildInlineCycleControl(tf, currentValue) {
  const options = tf.options || [];
  const wrapperId = "sf-field-" + tf.name + "-wrapper";

  // Resolve initial index from default value (or 0 if no options)
  let currentIdx = options.indexOf(currentValue);
  if (currentIdx < 0) currentIdx = options.length > 0 ? 0 : -1;
  const startVal = currentIdx >= 0 ? options[currentIdx] : "";

  const wrapper = document.createElement("div");
  wrapper.className = "inline-cycle-wrapper";
  wrapper.id = wrapperId;
  wrapper.setAttribute("role", "combobox");
  wrapper.setAttribute("aria-haspopup", "listbox");
  wrapper.setAttribute("aria-expanded", "false");
  wrapper.setAttribute("aria-label", (tf.prompt || tf.name));
  wrapper.dataset.currentValue = startVal;
  wrapper.dataset.currentIdx = String(currentIdx);
  if (!startVal) wrapper.dataset.empty = "true";
  // MAJOR 3 fix: track whether the user has explicitly interacted with this
  // cycling control. A required static_select with no configured default must
  // not pass validation silently just because options[0] is displayed as a hint.
  // Set to "true" on first ◂/▸/Left/Right interaction; required check reads it.
  wrapper.dataset.userInteracted = "false";

  // aria-activedescendant points at the active option li
  const activeId = wrapperId + "-opt-" + Math.max(0, currentIdx);
  wrapper.setAttribute("aria-activedescendant", activeId);

  // Previous button (◂)
  const prevBtn = document.createElement("button");
  prevBtn.type = "button";
  prevBtn.className = "inline-cycle-btn";
  prevBtn.setAttribute("aria-label", "Previous option");
  prevBtn.setAttribute("aria-hidden", "true"); // decorative — listbox is the a11y surface
  prevBtn.tabIndex = -1; // navigated via Left arrow key only
  prevBtn.innerHTML = "&#9666;"; // ◂

  // Current value display
  const valueEl = document.createElement("span");
  valueEl.className = "inline-cycle-value";
  valueEl.id = wrapperId + "-display";
  valueEl.textContent = startVal || "— select —";

  // Next button (▸)
  const nextBtn = document.createElement("button");
  nextBtn.type = "button";
  nextBtn.className = "inline-cycle-btn";
  nextBtn.setAttribute("aria-label", "Next option");
  nextBtn.setAttribute("aria-hidden", "true");
  nextBtn.tabIndex = -1;
  nextBtn.innerHTML = "&#9656;"; // ▸

  // Hidden listbox for screen readers
  const listbox = document.createElement("ul");
  listbox.className = "inline-cycle-listbox";
  listbox.setAttribute("role", "listbox");
  listbox.id = wrapperId + "-listbox";
  for (let i = 0; i < options.length; i++) {
    const li = document.createElement("li");
    li.setAttribute("role", "option");
    li.id = wrapperId + "-opt-" + i;
    li.setAttribute("aria-selected", i === currentIdx ? "true" : "false");
    li.textContent = options[i];
    listbox.appendChild(li);
  }

  wrapper.appendChild(prevBtn);
  wrapper.appendChild(valueEl);
  wrapper.appendChild(nextBtn);
  wrapper.appendChild(listbox);

  // Update helper: advance the cycling control by delta (-1 or +1), wrapping.
  function cycle(delta) {
    if (options.length === 0) return;
    let idx = parseInt(wrapper.dataset.currentIdx, 10);
    idx = ((idx + delta) + options.length) % options.length;
    wrapper.dataset.currentIdx = String(idx);
    wrapper.dataset.currentValue = options[idx];
    wrapper.dataset.userInteracted = "true";
    delete wrapper.dataset.empty;
    valueEl.textContent = options[idx];
    // Update aria-activedescendant and listbox aria-selected
    wrapper.setAttribute("aria-activedescendant", wrapperId + "-opt-" + idx);
    for (const li of listbox.querySelectorAll("[role='option']")) {
      li.setAttribute("aria-selected", li.id === wrapperId + "-opt-" + idx ? "true" : "false");
    }
    // Clear per-field error on change
    const errEl = document.getElementById("sf-err-" + tf.name);
    if (errEl) { errEl.textContent = ""; errEl.hidden = true; }
  }

  // Left/Right cycle; wrapper must be focusable for keydown.
  // The wrapper itself gets tabIndex=0 so it receives keyboard focus.
  wrapper.tabIndex = 0;

  // Keyboard: Left/Right on the wrapper cycle options.
  // Up/Down are intercepted by wireSubformNavigation (wired after DOM construction).
  // Enter/Space: no-op — value is always "selected".
  wrapper.addEventListener("keydown", e => {
    if (e.key === "ArrowLeft") { e.preventDefault(); cycle(-1); }
    else if (e.key === "ArrowRight") { e.preventDefault(); cycle(1); }
    // Up/Down are handled by the navigation wire — do NOT stopPropagation here.
  });

  // Tap ◂/▸ buttons
  prevBtn.addEventListener("click", e => { e.stopPropagation(); cycle(-1); wrapper.focus(); });
  nextBtn.addEventListener("click", e => { e.stopPropagation(); cycle(1); wrapper.focus(); });

  // Tap on scrim (wrapper background) focuses the wrapper
  wrapper.addEventListener("click", () => wrapper.focus());

  return wrapper;
}

// Wire Up/Down navigation across template fields.
// Up/Down move focus to the previous/next field's focusable input.
function wireSubformNavigation(fields) {
  const container = document.getElementById("subform-fields");
  if (!container) return;

  // Gather focusable elements in DOM order
  const focusables = Array.from(
    container.querySelectorAll("input, [tabindex='0'].inline-cycle-wrapper")
  );

  for (let i = 0; i < focusables.length; i++) {
    const el = focusables[i];
    el.addEventListener("keydown", e => {
      if (e.key === "ArrowUp" && i > 0) {
        e.preventDefault();
        focusables[i - 1].focus();
      } else if (e.key === "ArrowDown" && i < focusables.length - 1) {
        e.preventDefault();
        focusables[i + 1].focus();
      }
    });
  }
}

// Show a per-field error pill inside the overlay.
function showSubformFieldError(fieldName, msg) {
  const el = document.getElementById("sf-err-" + fieldName);
  if (el) { el.textContent = msg; el.hidden = false; }
}

// Show a top-level banner error in the overlay (defensive / server relay).
function showSubformTopError(msg) {
  const el = document.getElementById("subform-top-error");
  if (el) { el.textContent = msg; el.hidden = false; }
}

// TASK-2.3.5 — Re-open overlay with server-side errors pinned to fields.
// Called when parent submit returns 400 validation_failed with fields[]
// entries that match template field names.
//
// fieldErrors: array of { field, code } from server error details
// errorBanner: optional top-level message string
function reopenSubformWithErrors(fieldName, fieldErrors, errorBanner) {
  if (!_overlayContext || _overlayContext.fieldName !== fieldName) return;

  const overlay = document.getElementById("subform-overlay");
  overlay.hidden = false;
  overlay.setAttribute("aria-hidden", "false");

  const parentInput = document.getElementById("field-" + fieldName);
  if (parentInput) parentInput.disabled = true;

  if (errorBanner) showSubformTopError(errorBanner);

  for (const fe of (fieldErrors || [])) {
    showSubformFieldError(fe.field, fe.code || "invalid");
  }

  // Return focus to overlay
  const firstFocusable = overlay.querySelector("button, input, [tabindex='0']");
  if (firstFocusable) firstFocusable.focus();
}

// Focus trap: Tab/Shift-Tab cycles within the overlay panel.
document.addEventListener("keydown", e => {
  const overlay = document.getElementById("subform-overlay");
  if (!overlay || overlay.hidden) return;

  if (e.key === "Escape") {
    e.preventDefault();
    cancelSubform();
    return;
  }

  if (e.key === "Tab") {
    const panel = document.getElementById("subform-panel");
    const focusables = Array.from(
      panel.querySelectorAll(
        "button:not([disabled]), input:not([disabled]), [tabindex='0']:not([disabled])"
      )
    ).filter(el => el.offsetParent !== null); // only visible

    if (focusables.length === 0) return;

    const first = focusables[0];
    const last = focusables[focusables.length - 1];

    // If focus has drifted outside the panel (e.g. body, queue badge, parent input),
    // recapture it immediately before evaluating first/last wrap-around.
    if (!panel.contains(document.activeElement)) {
      e.preventDefault();
      if (e.shiftKey) { last.focus(); } else { first.focus(); }
      return;
    }

    if (e.shiftKey) {
      if (document.activeElement === first) {
        e.preventDefault();
        last.focus();
      }
    } else {
      if (document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    }
  }
}, true); // capture phase so ESC fires before any field handler

// ---------------------------------------------------------------------------
// Toast
// ---------------------------------------------------------------------------

let _toastTimer = null;
function showToast(msg, durationMs = 4000) {
  const el = document.getElementById("toast");
  el.textContent = msg;
  el.hidden = false;
  if (_toastTimer) clearTimeout(_toastTimer);
  _toastTimer = setTimeout(() => { el.hidden = true; }, durationMs);
}

// ---------------------------------------------------------------------------
// Health check
// ---------------------------------------------------------------------------

async function checkHealth() {
  const pill = document.getElementById("status-pill");
  try {
    const resp = await apiFetch("/api/v1/health");
    if (!resp.ok) {
      pill.textContent = "Server error (" + resp.status + ")";
      pill.className = "status-disconnected";
      return false;
    }
    const data = await resp.json();

    // Update wordmark vault name from health response
    const vaultPath = data.vault_base_path || "";
    const vaultName = vaultPath.split(/[/\\]/).filter(Boolean).pop() || "vault";
    const vaultEl = document.getElementById("header-vault");
    if (vaultEl) vaultEl.textContent = vaultName;

    const mode = data.transport_mode || "?";
    pill.textContent = mode;
    pill.className = mode === "API"
      ? "status-api"
      : mode === "FileSystem"
        ? "status-filesystem"
        : "status-disconnected";
    return true;
  } catch (e) {
    // 401 is thrown by apiFetch and handled above; other errors are connectivity
    if (e.message && e.message.startsWith("Unauthorized")) return false;
    pill.textContent = "Offline";
    pill.className = "status-disconnected";
    showToast("Server unreachable — is pour serve running?");
    return false;
  }
}

// ---------------------------------------------------------------------------
// Module list
// ---------------------------------------------------------------------------

async function loadDashboard() {
  // Reset idempotency key and auto-create state when navigating away from a form session
  _pendingIdempotencyKey = null;
  _pendingAutoCreateInputs = {};
  // CRITICAL 2 FIX: if the user abandons a queue-edit by going to the dashboard,
  // clear _editingQueueId so the abandoned record is NOT deleted on the next submit.
  _editingQueueId = null;

  try {
    const resp = await apiFetch("/api/v1/config");
    if (!resp.ok) { showToast("Failed to load config (" + resp.status + ")"); return; }
    _config = await resp.json();
  } catch (_e) {
    showToast("Failed to load config — server unreachable.");
    return;
  }

  const grid = document.getElementById("module-grid");
  grid.innerHTML = "";

  const modules = _config.modules || [];

  if (modules.length === 0) {
    const empty = document.createElement("p");
    empty.className = "empty-state";
    empty.textContent = "No modules configured. Edit ~/.pour/config.toml and re-run pour serve.";
    grid.appendChild(empty);
  } else {
    for (const mod of modules) {
      const btn = document.createElement("button");
      btn.className = "module-tile";
      btn.setAttribute("aria-pressed", "false");

      const iconSpan = document.createElement("span");
      iconSpan.className = "tile-icon";
      iconSpan.textContent = mod.icon || "📝";

      const nameSpan = document.createElement("span");
      nameSpan.className = "tile-name";
      nameSpan.textContent = mod.display_name || mod.key;

      const keySpan = document.createElement("span");
      keySpan.className = "tile-key";
      keySpan.textContent = "[" + mod.key + "]";

      btn.appendChild(iconSpan);
      btn.appendChild(nameSpan);
      btn.appendChild(keySpan);
      btn.addEventListener("click", () => openForm(mod.key));
      grid.appendChild(btn);
    }
  }

  showView("dashboard");
}

// ---------------------------------------------------------------------------
// Form rendering
// ---------------------------------------------------------------------------

async function openForm(moduleKey) {
  const mod = (_config.modules || []).find(m => m.key === moduleKey);
  if (!mod) { showToast("Unknown module: " + moduleKey); return; }

  _currentModule = moduleKey;
  _currentModuleData = mod;
  _presetsCache = [];
  _activePreset = null;
  // Rotate idempotency key for this new form session
  _pendingIdempotencyKey = null;
  // Clear any accumulated auto-create inputs from the previous session
  _pendingAutoCreateInputs = {};
  // CRITICAL 2 FIX: clear any in-flight queue edit so a freshly-opened form
  // does not accidentally delete the abandoned queued record on submit.
  _editingQueueId = null;

  // Show skeleton immediately — form shell with loading selects
  renderFormSkeleton(mod);
  showView("form");

  // Pre-fetch dynamic_select options AND presets in parallel
  _optionsCache = {};
  const dynFields = (mod.fields || []).filter(f => f.field_type === "dynamic_select");
  const [, presets] = await Promise.all([
    Promise.all(dynFields.map(f => fetchOptions(moduleKey, f.name))),
    fetchPresets(moduleKey),
  ]);
  // null = fetch failed — treat as empty so form renders without preset row
  _presetsCache = presets || [];

  // Full render once data is available
  renderForm(mod, {});
}

// Render a lightweight skeleton so the user sees the form shell immediately
// while options and presets are fetching in the background.
function renderFormSkeleton(mod) {
  const titleEl = document.getElementById("form-title");
  titleEl.innerHTML =
    '<span>' + escapeHtml(mod.icon || "") + '</span> ' +
    escapeHtml(mod.display_name || mod.key);

  const form = document.getElementById("capture-form");
  form.innerHTML = "";

  for (const field of (mod.fields || [])) {
    const group = document.createElement("div");
    group.className = "field-group";
    group.dataset.field = field.name;

    const label = document.createElement("label");
    label.setAttribute("for", "field-" + field.name);
    label.textContent = field.prompt || field.name;
    if (field.required) {
      const star = document.createElement("span");
      star.className = "required-star";
      star.textContent = "*";
      label.appendChild(star);
    }
    group.appendChild(label);

    // Skeleton placeholder for select fields; text/textarea rendered normally
    if (field.field_type === "static_select" || field.field_type === "dynamic_select") {
      const sel = document.createElement("select");
      sel.id = "field-" + field.name;
      sel.name = field.name;
      sel.disabled = true;
      const loadingOpt = document.createElement("option");
      loadingOpt.textContent = "Loading…";
      sel.appendChild(loadingOpt);
      group.appendChild(sel);
    } else {
      // Render real input immediately for non-select fields
      const input = buildFieldInput(field, "");
      group.appendChild(input);
    }

    form.appendChild(group);
  }

  appendFormActions(form);
}

function renderForm(mod, currentValues) {
  const form = document.getElementById("capture-form");
  form.innerHTML = "";

  // Preset chip row (above fields) — Phase 1.5 tap-to-apply + Phase 2 Stream C mutation.
  // Chip row only renders when there are presets; save-as zone always renders so users
  // can save the first preset even when none exist yet.
  if (_presetsCache.length > 0) {
    const chipRow = buildPresetChipRow(mod);
    form.appendChild(chipRow);
  }

  // TASK-2.4.2/2.4.3: host div for the edit panel (rendered on demand by openPresetEditPanel)
  const editPanelHost = document.createElement("div");
  editPanelHost.id = "preset-edit-panel-host";
  form.appendChild(editPanelHost);

  // TASK-2.4.1: inline save-as zone (always rendered — lets users add the first preset)
  const saveZone = buildPresetSaveZone(mod);
  form.appendChild(saveZone);

  // Render ALL fields (including hidden ones) so recomputeVisibility can toggle them.
  // Fields with show_when that currently evaluate to false start with hidden=true.
  const visible = computeVisible(mod.fields || [], currentValues);

  for (const field of (mod.fields || [])) {
    const group = document.createElement("div");
    group.className = "field-group";
    group.dataset.field = field.name;
    // Fields not in the visible set start hidden; recomputeVisibility toggles them.
    group.hidden = !visible.has(field.name);

    const label = document.createElement("label");
    label.setAttribute("for", "field-" + field.name);
    label.textContent = field.prompt || field.name;
    if (field.required) {
      const star = document.createElement("span");
      star.className = "required-star";
      star.textContent = "*";
      label.appendChild(star);
    }
    group.appendChild(label);

    const input = buildFieldInput(field, currentValues[field.name] || "");
    group.appendChild(input);

    const errEl = document.createElement("div");
    errEl.className = "field-error";
    errEl.id = "err-" + field.name;
    errEl.setAttribute("role", "alert");
    errEl.hidden = true;
    group.appendChild(errEl);

    form.appendChild(group);
  }

  appendFormActions(form);

  // Reactivity: recompute on any change or input event.
  // Both listeners are needed: "change" fires on <select> and on <input> blur;
  // "input" fires on every keystroke for text/number inputs. Together they cover
  // all field types without a stale read.
  form.addEventListener("change", () => {
    const vals = readCurrentFieldValues();
    recomputeVisibility(mod.fields || [], vals);
  });
  form.addEventListener("input", () => {
    const vals = readCurrentFieldValues();
    recomputeVisibility(mod.fields || [], vals);
  });

  form.addEventListener("submit", async e => {
    e.preventDefault();
    await handleSubmit();
  });
}

// Build and append sticky submit + back buttons to form.
// Extracted so renderFormSkeleton and renderForm share the same action bar.
function appendFormActions(form) {
  const actions = document.createElement("div");
  actions.className = "form-actions";

  const submitBtn = document.createElement("button");
  submitBtn.type = "submit";
  submitBtn.className = "btn-primary";
  submitBtn.textContent = "Pour";
  actions.appendChild(submitBtn);

  const backBtn = document.createElement("button");
  backBtn.type = "button";
  backBtn.className = "btn-secondary";
  backBtn.style.width = "100%";
  backBtn.textContent = "Back";
  backBtn.addEventListener("click", loadDashboard);
  actions.appendChild(backBtn);

  form.appendChild(actions);
}

function buildFieldInput(field, currentValue) {
  switch (field.field_type) {
    case "textarea": {
      const ta = document.createElement("textarea");
      ta.id = "field-" + field.name;
      ta.name = field.name;
      ta.rows = 4;
      ta.value = currentValue;
      if (field.required) {
        ta.required = true;
        ta.setAttribute("aria-required", "true");
        ta.setAttribute("aria-describedby", "err-" + field.name);
      }
      return ta;
    }
    case "number": {
      const inp = document.createElement("input");
      inp.type = "number";
      inp.inputMode = "decimal";
      inp.step = "any";
      inp.id = "field-" + field.name;
      inp.name = field.name;
      inp.value = currentValue;
      if (field.required) {
        inp.required = true;
        inp.setAttribute("aria-required", "true");
        inp.setAttribute("aria-describedby", "err-" + field.name);
      }
      return inp;
    }
    case "static_select": {
      const sel = document.createElement("select");
      sel.id = "field-" + field.name;
      sel.name = field.name;
      const blank = document.createElement("option");
      blank.value = "";
      blank.textContent = "— select —";
      sel.appendChild(blank);
      for (const opt of (field.options || [])) {
        const o = document.createElement("option");
        o.value = opt;
        o.textContent = opt;
        if (opt === currentValue) o.selected = true;
        sel.appendChild(o);
      }
      if (field.required) {
        sel.required = true;
        sel.setAttribute("aria-required", "true");
        sel.setAttribute("aria-describedby", "err-" + field.name);
      }
      return sel;
    }
    case "dynamic_select": {
      const cacheKey = _currentModule + ":" + field.name;
      const opts = _optionsCache[cacheKey] || [];
      if (field.allow_create) {
        // Combobox: <input list="..."> + <datalist>
        const wrapper = document.createElement("div");
        const inp = document.createElement("input");
        inp.type = "text";
        inp.id = "field-" + field.name;
        inp.name = field.name;
        inp.value = currentValue;
        inp.setAttribute("list", "dl-" + field.name);
        if (field.required) {
          inp.required = true;
          inp.setAttribute("aria-required", "true");
          inp.setAttribute("aria-describedby", "err-" + field.name);
        }
        const dl = document.createElement("datalist");
        dl.id = "dl-" + field.name;
        for (const opt of opts) {
          const o = document.createElement("option");
          o.value = opt;
          dl.appendChild(o);
        }
        wrapper.appendChild(inp);
        wrapper.appendChild(dl);

        // TASK-2.3.3 — If field has create_template, wire a "change" listener
        // that opens the sub-form overlay when the typed value is novel.
        // Also: if the user edits back to a known value after confirming,
        // drop the accumulated auto_create_inputs entry for this field.
        if (field.create_template) {
          const templateName = field.create_template;
          inp.addEventListener("change", () => {
            const val = inp.value;
            const currentOpts = _optionsCache[cacheKey] || [];

            if (!isNovelValue(val, currentOpts)) {
              // Typed back to an existing value — drop any pending auto-create for this field.
              delete _pendingAutoCreateInputs[field.name];
              return;
            }
            // Novel value — open overlay (only if not already open for this field)
            if (!_overlayContext) {
              openSubformOverlay(field.name, templateName, val);
            }
          });
        }

        return wrapper;
      } else {
        const sel = document.createElement("select");
        sel.id = "field-" + field.name;
        sel.name = field.name;
        const blank = document.createElement("option");
        blank.value = "";
        blank.textContent = opts.length === 0
          ? "No options yet — pour something via the TUI first."
          : "— select —";
        sel.appendChild(blank);
        for (const opt of opts) {
          const o = document.createElement("option");
          o.value = opt;
          o.textContent = opt;
          if (opt === currentValue) o.selected = true;
          sel.appendChild(o);
        }
        if (field.required) {
          sel.required = true;
          sel.setAttribute("aria-required", "true");
          sel.setAttribute("aria-describedby", "err-" + field.name);
        }
        return sel;
      }
    }
    case "composite_array": {
      // TODO(phase-2): prefill from preset apply — thread currentValue here when presets land.
      return buildCompositeEditor(field, []);
    }
    default: {
      // text and fallback
      const inp = document.createElement("input");
      inp.type = "text";
      inp.id = "field-" + field.name;
      inp.name = field.name;
      inp.value = currentValue;
      if (field.required) {
        inp.required = true;
        inp.setAttribute("aria-required", "true");
        inp.setAttribute("aria-describedby", "err-" + field.name);
      }
      return inp;
    }
  }
}

// ---------------------------------------------------------------------------
// Composite array editor
// ---------------------------------------------------------------------------

function buildCompositeEditor(field, initialRows) {
  const container = document.createElement("div");
  container.id = "field-" + field.name;
  container.dataset.composite = field.name;

  const subFields = field.sub_fields || [];

  function addRow(values) {
    const row = document.createElement("div");
    row.className = "composite-row";
    for (let i = 0; i < subFields.length; i++) {
      const sf = subFields[i];
      const inp = document.createElement("input");
      inp.type = "text";
      inp.placeholder = sf.prompt || sf.name;
      inp.value = values ? (values[i] || "") : "";
      row.appendChild(inp);
    }
    const rmBtn = document.createElement("button");
    rmBtn.type = "button";
    rmBtn.className = "btn-remove-row";
    rmBtn.textContent = "×";  // × multiplication sign
    rmBtn.addEventListener("click", () => row.remove());
    row.appendChild(rmBtn);
    container.insertBefore(row, addBtn);
  }

  const addBtn = document.createElement("button");
  addBtn.type = "button";
  addBtn.className = "btn-add-row";
  addBtn.textContent = "+ Add row";
  addBtn.addEventListener("click", () => addRow(null));
  container.appendChild(addBtn);

  for (const r of initialRows) addRow(r);
  // Start with one empty row
  if (initialRows.length === 0) addRow(null);

  return container;
}

// ---------------------------------------------------------------------------
// show_when evaluation — mirrors src/visibility.rs is_field_visible exactly.
//
// Server rule (visibility.rs line 18-21):
//   If field_values[show_when.field] is absent or empty → hidden (return false).
//   equals variant: visible iff controlling == equals.
//   one_of variant: visible iff controlling is in the list.
//   show_when present but neither equals nor one_of → hidden.
// ---------------------------------------------------------------------------

function computeVisible(fields, values) {
  const visible = new Set();
  for (const f of fields) {
    if (!f.show_when) { visible.add(f.name); continue; }
    const rule = f.show_when;
    // Absent or empty controlling value → hidden (mirrors server: Some(v) if !v.is_empty())
    const controlling = (values[rule.field] != null ? values[rule.field] : "");
    if (controlling === "") continue;
    if (rule.equals !== undefined) {
      if (controlling === rule.equals) visible.add(f.name);
    } else if (rule.one_of !== undefined) {
      if (rule.one_of.includes(controlling)) visible.add(f.name);
    }
    // show_when present but neither equals nor one_of → hidden (matches server default)
  }
  return visible;
}

// Read current field values directly from the DOM, including all fields regardless of
// visibility. This snapshot is used by recomputeVisibility so the controlling field's
// current value is always fresh. (Previously collectValues() gated on visibility, which
// caused a chicken-and-egg: the controlling field was visible and present, but hidden
// dependent fields were absent from the snapshot — breaking the show_when evaluation.)
function readCurrentFieldValues() {
  const v = {};
  const fields = _currentModuleData ? (_currentModuleData.fields || []) : [];
  for (const f of fields) {
    if (f.field_type === "composite_array") continue;
    const el = document.getElementById("field-" + f.name);
    if (el) v[f.name] = el.value || "";
  }
  return v;
}

function recomputeVisibility(fields, values) {
  const visible = computeVisible(fields, values);
  for (const f of fields) {
    const group = document.querySelector("[data-field='" + f.name + "']");
    if (group) group.hidden = !visible.has(f.name);
  }
}

// ---------------------------------------------------------------------------
// Collect form values (for submit — visible fields only, excluding composite)
// ---------------------------------------------------------------------------

function collectValues() {
  const vals = {};
  const fields = _currentModuleData ? (_currentModuleData.fields || []) : [];
  // Use readCurrentFieldValues to get a fresh snapshot of all field values first,
  // then filter to only those fields that are currently visible.
  const allVals = readCurrentFieldValues();
  const visible = computeVisible(fields, allVals);

  for (const f of fields) {
    if (!visible.has(f.name)) continue;
    if (f.field_type === "composite_array") continue;
    const el = document.getElementById("field-" + f.name);
    if (el) vals[f.name] = el.value || "";
  }
  return vals;
}

function collectCompositeData() {
  const composite = {};
  const fields = _currentModuleData ? (_currentModuleData.fields || []) : [];
  const currentVals = collectValues();
  const visible = computeVisible(fields, currentVals);

  for (const f of fields) {
    if (f.field_type !== "composite_array") continue;
    if (!visible.has(f.name)) continue;
    const container = document.querySelector("[data-composite='" + f.name + "']");
    if (!container) continue;
    const rows = [];
    for (const row of container.querySelectorAll(".composite-row")) {
      const cells = Array.from(row.querySelectorAll("input")).map(i => i.value);
      if (cells.some(c => c.trim() !== "")) rows.push(cells);
    }
    if (rows.length > 0) composite[f.name] = rows;
  }
  return composite;
}

// ---------------------------------------------------------------------------
// Presets — Phase 1.5 (read-only tap-to-apply) + Phase 2 Stream C (mutation)
// ---------------------------------------------------------------------------

// Returns the preset list on success, or null on network/server error.
// Callers that re-render chips after mutations should skip the refresh if null
// is returned (to avoid accidentally wiping the chip row on a transient fetch failure).
async function fetchPresets(moduleKey) {
  try {
    const resp = await apiFetch("/api/v1/presets/" + moduleKey);
    if (resp.ok) {
      const data = await resp.json();
      return data.presets || [];
    }
  } catch (_e) {
    // Network error or 401 (handled by apiFetch) — degrade silently
  }
  return null;
}

// ---------------------------------------------------------------------------
// Phase 2 Stream C — TASK-2.4.5: Offline guard for preset mutations.
// Returns true if online; shows toast and returns false if offline.
// ---------------------------------------------------------------------------
function assertOnlineForPreset() {
  if (!navigator.onLine) {
    showToast("Offline — preset changes need a connection.");
    return false;
  }
  return true;
}

// ---------------------------------------------------------------------------
// Phase 2 Stream C — Re-render the chip row from a fresh presets list.
// Always called after any mutation response so the DOM stays in sync with
// the server's authoritative state (TASK-2.4.5: never trust optimistic local).
// ---------------------------------------------------------------------------
function refreshPresetChipRowFromList(mod, presets) {
  _presetsCache = presets;
  // If _activePreset was deleted or renamed away, clear it and reset form fields (MAJOR 5).
  if (_activePreset !== null && !presets.some(p => p.name === _activePreset)) {
    _activePreset = null;
    // Reset form fields to defaults so the form doesn't silently hold the deleted preset's values.
    applyPreset(mod, null);
  }

  const oldRow = document.getElementById("preset-chip-row");
  const editHost = document.getElementById("preset-edit-panel-host");

  if (presets.length === 0) {
    // Remove chip row if it exists (no presets left)
    if (oldRow) oldRow.remove();
  } else {
    const newRow = buildPresetChipRow(mod);
    if (oldRow) {
      // Replace existing row in place
      oldRow.replaceWith(newRow);
    } else if (editHost) {
      // No chip row yet (first preset was just saved) — insert before edit host
      editHost.before(newRow);
    }
  }

  // Refresh the save-as zone if it exists (re-build preserves collapse state would be nicer
  // but re-building is simpler and correct — the user just saved, so collapse it)
  const oldSaveZone = document.getElementById("preset-save-zone");
  if (oldSaveZone) {
    const newSaveZone = buildPresetSaveZone(mod);
    oldSaveZone.replaceWith(newSaveZone);
  }
}

// ---------------------------------------------------------------------------
// Phase 2 Stream C — TASK-2.4.2/2.4.3: Preset edit panel.
// Shows name (editable), description (editable), Save + Delete buttons.
// Panel is appended to #preset-edit-panel-host in the form section.
// ---------------------------------------------------------------------------
function openPresetEditPanel(mod, preset) {
  closePresetEditPanel(); // dismiss any existing panel

  const host = document.getElementById("preset-edit-panel-host");
  if (!host) return;

  const panel = document.createElement("div");
  panel.id = "preset-edit-panel";
  panel.className = "preset-edit-panel";
  panel.setAttribute("role", "group");
  panel.setAttribute("aria-label", "Edit preset: " + preset.name);

  // Helper text — user is editing metadata, not re-snapshotting values
  const hint = document.createElement("p");
  hint.className = "preset-edit-hint";
  hint.textContent = "Edit name or description. To update values, delete and re-save.";
  panel.appendChild(hint);

  // Name field
  const nameGroup = document.createElement("div");
  nameGroup.className = "field-group";
  const nameLabel = document.createElement("label");
  nameLabel.textContent = "Name";
  nameLabel.setAttribute("for", "preset-edit-name");
  const nameInput = document.createElement("input");
  nameInput.type = "text";
  nameInput.id = "preset-edit-name";
  nameInput.value = preset.name;
  nameInput.maxLength = 64;
  nameInput.setAttribute("aria-describedby", "preset-edit-name-err");
  const nameErr = document.createElement("div");
  nameErr.className = "field-error";
  nameErr.id = "preset-edit-name-err";
  nameErr.hidden = true;
  nameGroup.appendChild(nameLabel);
  nameGroup.appendChild(nameInput);
  nameGroup.appendChild(nameErr);
  panel.appendChild(nameGroup);

  // Description field
  const descGroup = document.createElement("div");
  descGroup.className = "field-group";
  const descLabel = document.createElement("label");
  descLabel.textContent = "Description";
  descLabel.setAttribute("for", "preset-edit-desc");
  const descInput = document.createElement("input");
  descInput.type = "text";
  descInput.id = "preset-edit-desc";
  descInput.value = preset.description || "";
  descGroup.appendChild(descLabel);
  descGroup.appendChild(descInput);
  panel.appendChild(descGroup);

  // Action row: Save + Delete
  const actionsRow = document.createElement("div");
  actionsRow.className = "preset-edit-actions";

  // Save button
  const saveBtn = document.createElement("button");
  saveBtn.type = "button";
  saveBtn.className = "btn-primary preset-edit-save";
  saveBtn.textContent = "Save";
  saveBtn.addEventListener("click", async () => {
    if (!assertOnlineForPreset()) return;

    const newName = nameInput.value.trim();
    // Validate name
    if (!newName || newName.length > 64) {
      nameErr.textContent = "Name must be 1–64 characters.";
      nameErr.hidden = false;
      return;
    }
    if (newName.includes("/")) {
      nameErr.textContent = "Name may not contain '/'.";
      nameErr.hidden = false;
      return;
    }
    if (newName.toLowerCase() === "order") {
      nameErr.textContent = "'order' is reserved — pick another name.";
      nameErr.hidden = false;
      return;
    }
    nameErr.hidden = true;

    const newDescription = descInput.value; // allow empty string
    const oldName = preset.name;
    const nameChanged = newName !== oldName;

    // Pre-check rename clobber: if new name already exists as a different preset, block (CRITICAL 3).
    if (nameChanged && _presetsCache.find(p => p.name === newName)) {
      nameErr.textContent = "A preset named '" + newName + "' already exists. Pick a different name.";
      nameErr.hidden = false;
      return;
    }

    saveBtn.disabled = true;
    saveBtn.textContent = "Saving…";

    try {
      // PUT new name first
      const putResp = await apiFetch(
        "/api/v1/presets/" + encodeURIComponent(mod.key) + "/" + encodeURIComponent(newName),
        {
          method: "PUT",
          body: JSON.stringify({
            description: newDescription || null,
            values: preset.values || {},
          }),
        }
      );
      if (!putResp.ok) {
        const err = await putResp.json().catch(() => ({}));
        const msg = (err.error && err.error.message) || ("Save failed: " + putResp.status);
        showToast(msg);
        saveBtn.disabled = false;
        saveBtn.textContent = "Save";
        return;
      }

      // If name changed, DELETE old — only after PUT succeeded
      if (nameChanged) {
        // Best-effort DELETE. On failure we log a known minor leak.
        // The user can manually clean up via next mutation.
        const delResp = await apiFetch(
          "/api/v1/presets/" + encodeURIComponent(mod.key) + "/" + encodeURIComponent(oldName),
          { method: "DELETE" }
        ).catch(() => null);
        // 204 = success, 404 = already gone (race). Both are fine. Other codes = minor leak.
        if (delResp && !delResp.ok && delResp.status !== 404) {
          // Known minor leak: PUT succeeded, DELETE failed — a duplicate may exist.
          // The user can see and remove duplicates on the next mutation.
          console.warn("pour: preset rename — old entry not deleted (status " + delResp.status + ")");
        }
      }

      // If the active preset was renamed, update _activePreset to the new name BEFORE
      // re-rendering chips so the renamed chip renders highlighted (MAJOR 5).
      if (nameChanged && _activePreset === oldName) {
        _activePreset = newName;
      }

      // Re-fetch canonical list from server, re-render from response
      const freshPresets = await fetchPresets(mod.key);
      // null means the re-fetch failed — don't wipe the chip row; show a warning toast
      if (freshPresets !== null) {
        refreshPresetChipRowFromList(mod, freshPresets);
      } else {
        showToast("Saved — but couldn't refresh preset list.");
      }
      closePresetEditPanel();
    } catch (e) {
      const msg = e?.message || String(e);
      if (!msg.startsWith("Unauthorized")) {
        showToast("Save failed — server unreachable.");
      }
      saveBtn.disabled = false;
      saveBtn.textContent = "Save";
    }
  });

  // Delete button — TASK-2.4.3: single-tap-confirm pattern
  const deleteBtn = document.createElement("button");
  deleteBtn.type = "button";
  deleteBtn.className = "btn-secondary preset-edit-delete";
  deleteBtn.textContent = "Delete";
  deleteBtn.dataset.confirmPending = "false";
  let deleteConfirmTimer = null;

  deleteBtn.addEventListener("click", async () => {
    if (!assertOnlineForPreset()) return;

    if (deleteBtn.dataset.confirmPending === "true") {
      // Second tap within 2s — execute delete
      if (deleteConfirmTimer !== null) { clearTimeout(deleteConfirmTimer); deleteConfirmTimer = null; }
      deleteBtn.disabled = true;
      deleteBtn.textContent = "Deleting…";

      try {
        const delResp = await apiFetch(
          "/api/v1/presets/" + encodeURIComponent(mod.key) + "/" + encodeURIComponent(preset.name),
          { method: "DELETE" }
        );
        // 204 = deleted; 404 = already gone (race — treat as success per TASK-2.4.3)
        if (delResp.ok || delResp.status === 404) {
          // _activePreset cleared in refreshPresetChipRowFromList if name matches
          const freshPresets = await fetchPresets(mod.key);
          if (freshPresets !== null) {
            refreshPresetChipRowFromList(mod, freshPresets);
          } else {
            // Re-fetch failed — optimistically remove the deleted preset from local cache
            // so the chip disappears. This is the one case where we trust local state
            // because the DELETE already 204'd (or 404'd), so the item is definitely gone.
            const optimisticList = _presetsCache.filter(p => p.name !== preset.name);
            refreshPresetChipRowFromList(mod, optimisticList);
          }
          closePresetEditPanel();
        } else {
          const err = await delResp.json().catch(() => ({}));
          const msg = (err.error && err.error.message) || ("Delete failed: " + delResp.status);
          showToast(msg);
          deleteBtn.disabled = false;
          deleteBtn.textContent = "Delete";
          deleteBtn.dataset.confirmPending = "false";
        }
      } catch (e) {
        const msg = e?.message || String(e);
        if (!msg.startsWith("Unauthorized")) showToast("Delete failed — server unreachable.");
        deleteBtn.disabled = false;
        deleteBtn.textContent = "Delete";
        deleteBtn.dataset.confirmPending = "false";
      }
    } else {
      // First tap — arm confirm
      deleteBtn.dataset.confirmPending = "true";
      deleteBtn.textContent = "Tap again to confirm";
      deleteConfirmTimer = setTimeout(() => {
        if (deleteBtn.dataset.confirmPending === "true") {
          deleteBtn.dataset.confirmPending = "false";
          deleteBtn.textContent = "Delete";
        }
        deleteConfirmTimer = null;
      }, 2000);
    }
  });

  // Cancel / close button
  const closeBtn = document.createElement("button");
  closeBtn.type = "button";
  closeBtn.className = "btn-secondary preset-edit-close";
  closeBtn.textContent = "Cancel";
  closeBtn.addEventListener("click", closePresetEditPanel);

  actionsRow.appendChild(saveBtn);
  actionsRow.appendChild(deleteBtn);
  actionsRow.appendChild(closeBtn);
  panel.appendChild(actionsRow);

  host.appendChild(panel);
  nameInput.focus();
  nameInput.select();
}

function closePresetEditPanel() {
  const panel = document.getElementById("preset-edit-panel");
  if (panel) panel.remove();
}

// ---------------------------------------------------------------------------
// Phase 2 Stream C — TASK-2.4.4: Drag-to-reorder chip row.
//
// Uses pointer events. Strategy: on pointerdown start a drag; on pointermove
// update a ghost element position + visual placeholder in the chip row;
// on pointerup commit the new order.
//
// Coexists with horizontal scroll-snap: we set touch-action: none on the chip
// during drag and restore it after. Scroll is via normal overflow-x on the row.
//
// Concurrency: "latest-wins" serialized. Only one PUT in flight at a time.
// If new drops arrive while a PUT is pending, we queue the latest known order
// and fire it when the prior PUT settles.
// ---------------------------------------------------------------------------
let _reorderInFlight = false;
let _reorderPendingOrder = null; // latest names array waiting to fire

async function fireReorderPut(mod, names) {
  if (_reorderInFlight) {
    // Queue latest order; prior PUT will fire it when done
    _reorderPendingOrder = names;
    return;
  }

  _reorderInFlight = true;
  _reorderPendingOrder = null;

  try {
    const resp = await apiFetch(
      "/api/v1/presets/" + encodeURIComponent(mod.key) + "/order",
      {
        method: "PUT",
        body: JSON.stringify({ names }),
      }
    );
    if (resp.ok) {
      const data = await resp.json();
      // Re-render from server response — server is authoritative
      refreshPresetChipRowFromList(mod, data.presets || []);
    } else {
      const err = await resp.json().catch(() => ({}));
      const msg = (err.error && err.error.message) || ("Reorder failed: " + resp.status);
      showToast(msg);
      // DOM was already mutated by the drag — re-fetch canonical order to restore it (MAJOR 6).
      const canonical = await fetchPresets(mod.key);
      if (canonical !== null) refreshPresetChipRowFromList(mod, canonical);
    }
  } catch (e) {
    const msg = e?.message || String(e);
    if (!msg.startsWith("Unauthorized")) showToast("Reorder failed — server unreachable.");
  } finally {
    _reorderInFlight = false;
    // If a new order arrived while we were in flight, fire it now
    if (_reorderPendingOrder !== null) {
      const next = _reorderPendingOrder;
      _reorderPendingOrder = null;
      fireReorderPut(mod, next);
    }
  }
}

// Attach drag-reorder behaviour to a chip row.
// Also handles long-press vs short-tap dispatch for named chips.
// Long-press (~500ms without significant movement) opens the edit panel.
// Short tap (pointerup before the timer) applies the preset.
// Horizontal movement > DRAG_THRESHOLD_PX commits a drag, cancelling both timers.
//
// This single handler replaces the separate attachLongPress on each chip —
// having two pointerdown listeners on the same element caused both long-press
// and drag to activate simultaneously.
function attachDragReorder(row, mod) {
  let dragging = null;     // { chip, ghost, origIdx, placeholder, longPressTimer, preset }
  let dragStartX = 0;
  let dragStartY = 0;
  const DRAG_THRESHOLD_PX = 8; // pixels of movement before drag is "committed"
  const LONG_PRESS_MS = 500;   // hold threshold for edit panel
  let dragCommitted = false;
  let longPressFired = false;  // true if long-press handler already fired (blocks tap)

  row.addEventListener("pointerdown", e => {
    const chip = e.target.closest(".preset-chip[data-preset-name]");
    // Only handle named preset chips via this handler — <none> chip uses plain click
    if (!chip || chip.dataset.presetName === "") return;
    if (e.button !== 0) return;

    dragStartX = e.clientX;
    dragStartY = e.clientY;
    dragCommitted = false;
    longPressFired = false;

    // Find the preset object for this chip
    const chipPreset = _presetsCache.find(p => p.name === chip.dataset.presetName);

    // Start long-press timer — fires edit panel if user holds without moving
    const longPressTimer = setTimeout(async () => {
      if (!dragCommitted && dragging) {
        longPressFired = true;
        let resolvedPreset = chipPreset;
        if (!resolvedPreset) {
          // Cache desync — re-fetch and retry before opening panel (CRITICAL 4).
          const fresh = await fetchPresets(mod.key);
          if (fresh !== null) {
            _presetsCache = fresh;
            resolvedPreset = fresh.find(p => p.name === chip.dataset.presetName);
          }
          if (!resolvedPreset) {
            showToast("Preset state out of sync — refreshing");
            const refreshed = await fetchPresets(mod.key);
            if (refreshed !== null) refreshPresetChipRowFromList(mod, refreshed);
            return;
          }
        }
        openPresetEditPanel(mod, resolvedPreset);
      }
    }, LONG_PRESS_MS);

    dragging = {
      chip,
      origIdx: chipIndex(chip),
      placeholder: null,
      ghost: null,
      longPressTimer,
      preset: chipPreset,
    };
    chip.setPointerCapture(e.pointerId);
  });

  row.addEventListener("pointermove", e => {
    if (!dragging) return;

    const dx = e.clientX - dragStartX;
    const dy = e.clientY - dragStartY;
    const dist = Math.sqrt(dx * dx + dy * dy);

    if (!dragCommitted) {
      if (dist < DRAG_THRESHOLD_PX) return;
      // Movement threshold crossed — cancel long-press and commit drag
      if (dragging.longPressTimer !== null) {
        clearTimeout(dragging.longPressTimer);
        dragging.longPressTimer = null;
      }
      dragCommitted = true;
      startDrag(dragging.chip, e);
    }

    moveDrag(e);
  });

  row.addEventListener("pointerup", e => {
    if (!dragging) return;
    if (dragging.longPressTimer !== null) {
      clearTimeout(dragging.longPressTimer);
      dragging.longPressTimer = null;
    }
    const wasCommitted = dragCommitted;
    const firedLongPress = longPressFired;
    endDrag(e, mod, wasCommitted, firedLongPress);
    dragging = null;
    dragCommitted = false;
    longPressFired = false;
  });

  row.addEventListener("pointercancel", () => {
    if (!dragging) return;
    if (dragging.longPressTimer !== null) {
      clearTimeout(dragging.longPressTimer);
      dragging.longPressTimer = null;
    }
    cancelDrag();
    dragging = null;
    dragCommitted = false;
    longPressFired = false;
  });

  function chipIndex(chip) {
    const chips = Array.from(row.querySelectorAll(".preset-chip[data-preset-name]"))
      .filter(c => c.dataset.presetName !== "");
    return chips.indexOf(chip);
  }

  function startDrag(chip, e) {
    chip.classList.add("preset-chip--dragging");
    // touch-action: none is set on .preset-chip--dragging in CSS
    // Create a visual placeholder in the original position
    const placeholder = document.createElement("div");
    placeholder.className = "preset-chip-placeholder";
    placeholder.style.width = chip.offsetWidth + "px";
    placeholder.style.minHeight = chip.offsetHeight + "px";
    chip.after(placeholder);
    dragging.placeholder = placeholder;

    // Ghost: a fixed clone of the chip that follows the pointer
    const rect = chip.getBoundingClientRect();
    const ghost = chip.cloneNode(true);
    ghost.classList.add("preset-chip--ghost");
    ghost.classList.remove("preset-chip--dragging");
    ghost.style.position = "fixed";
    ghost.style.left = rect.left + "px";
    ghost.style.top = rect.top + "px";
    ghost.style.width = rect.width + "px";
    ghost.style.pointerEvents = "none";
    ghost.style.zIndex = "300";
    document.body.appendChild(ghost);
    dragging.ghost = ghost;

    dragging.ghostOffsetX = e.clientX - rect.left;
    dragging.ghostOffsetY = e.clientY - rect.top;
  }

  function moveDrag(e) {
    if (!dragging || !dragging.ghost) return;

    // Move ghost
    dragging.ghost.style.left = (e.clientX - dragging.ghostOffsetX) + "px";
    dragging.ghost.style.top = (e.clientY - dragging.ghostOffsetY) + "px";

    // Reorder placeholder: find which chip the pointer is over (by center X)
    const chips = Array.from(row.querySelectorAll(".preset-chip[data-preset-name]"))
      .filter(c => c.dataset.presetName !== "" && c !== dragging.chip);
    const placeholder = dragging.placeholder;

    // Move the placeholder to the closest insertion point
    let inserted = false;
    for (const c of chips) {
      const rect = c.getBoundingClientRect();
      if (e.clientX < rect.left + rect.width / 2) {
        c.before(placeholder);
        inserted = true;
        break;
      }
    }
    if (!inserted) {
      // After all chips
      const lastChip = chips[chips.length - 1];
      if (lastChip) lastChip.after(placeholder);
    }
  }

  function endDrag(e, mod, wasCommitted, firedLongPress) {
    if (!dragging) return;

    if (dragging.ghost) { dragging.ghost.remove(); dragging.ghost = null; }
    dragging.chip.classList.remove("preset-chip--dragging");

    if (!wasCommitted) {
      // Was a short tap or long-press (handled above) — clean up
      if (dragging.placeholder) { dragging.placeholder.remove(); dragging.placeholder = null; }
      // If neither long-press NOR drag committed, it was a short tap → apply preset
      if (!firedLongPress && dragging.preset) {
        applyPreset(mod, dragging.preset);
      }
      return;
    }

    // Committed drag: replace placeholder with chip
    if (dragging.placeholder) {
      dragging.placeholder.replaceWith(dragging.chip);
      dragging.placeholder = null;
    }

    if (!assertOnlineForPreset()) return;

    // Read the new canonical order from the DOM (excludes <none>)
    const names = Array.from(row.querySelectorAll(".preset-chip[data-preset-name]"))
      .filter(c => c.dataset.presetName !== "")
      .map(c => c.dataset.presetName);

    fireReorderPut(mod, names);
  }

  function cancelDrag() {
    if (!dragging) return;
    if (dragging.ghost) { dragging.ghost.remove(); dragging.ghost = null; }
    dragging.chip.classList.remove("preset-chip--dragging");
    if (dragging.placeholder) {
      // Restore chip to original position — easiest: just re-fetch re-renders on next op
      dragging.placeholder.replaceWith(dragging.chip);
      dragging.placeholder = null;
    }
  }
}

// Build the horizontal chip row. Always includes a "<none>" chip first.
// Phase 2 Stream C: chips now support long-press to open edit panel.
function buildPresetChipRow(mod) {
  const row = document.createElement("div");
  row.className = "preset-chip-row";
  row.id = "preset-chip-row";

  // <none> chip — clears form back to defaults (no long-press on <none>)
  const noneChip = document.createElement("button");
  noneChip.type = "button";
  noneChip.className = "preset-chip";
  noneChip.textContent = "<none>";
  noneChip.dataset.presetName = "";
  noneChip.setAttribute("aria-pressed", _activePreset === null ? "true" : "false");
  if (_activePreset === null) noneChip.classList.add("preset-chip--active");
  noneChip.addEventListener("click", () => applyPreset(mod, null));
  row.appendChild(noneChip);

  for (const preset of _presetsCache) {
    const chip = document.createElement("button");
    chip.type = "button";
    chip.className = "preset-chip";
    chip.textContent = preset.name;
    chip.dataset.presetName = preset.name;
    chip.setAttribute("aria-pressed", _activePreset === preset.name ? "true" : "false");
    if (_activePreset === preset.name) chip.classList.add("preset-chip--active");
    // Long-press, short-tap, and drag are all handled by attachDragReorder (below)
    // which intercepts pointer events on the row and dispatches per gesture.
    row.appendChild(chip);
  }

  // TASK-2.4.4: attach drag-reorder to the row after all chips are in place.
  // Also handles long-press and short-tap for named chips via the unified handler.
  // Even when there's only 1 preset we still attach so long-press/tap work.
  if (_presetsCache.length > 0) {
    attachDragReorder(row, mod);
  }

  // TASK-2.4.2: right-click on named chips opens edit panel (desktop fallback).
  // -webkit-touch-callout: none (CSS) suppresses iOS native context menu.
  for (const preset of _presetsCache) {
    const chip = row.querySelector(`[data-preset-name="${CSS.escape(preset.name)}"]`);
    if (chip) {
      chip.addEventListener("contextmenu", e => {
        e.preventDefault();
        openPresetEditPanel(mod, preset);
      });
    }
  }

  return row;
}

// ---------------------------------------------------------------------------
// Phase 2 Stream C — TASK-2.4.1: "Save current values as preset" inline zone.
// Rendered below the chip row. Name input enforces 1-64 chars, no '/'.
// Values snapshot: excludes composite_array, preset_exclude=true, and
// fields hidden by show_when — mirrors src/data/presets.rs TUI behaviour.
// ---------------------------------------------------------------------------
function buildPresetSaveZone(mod) {
  const zone = document.createElement("div");
  zone.id = "preset-save-zone";
  zone.className = "preset-save-zone";

  // "Save as preset" toggle button — tap to reveal inline name input
  const toggleBtn = document.createElement("button");
  toggleBtn.type = "button";
  toggleBtn.className = "preset-save-toggle";
  toggleBtn.textContent = "+ Save as preset";
  toggleBtn.setAttribute("aria-expanded", "false");
  toggleBtn.setAttribute("aria-controls", "preset-save-form");
  zone.appendChild(toggleBtn);

  // Inline name input form (hidden until toggle)
  const saveForm = document.createElement("div");
  saveForm.id = "preset-save-form";
  saveForm.className = "preset-save-form";
  saveForm.hidden = true;
  saveForm.setAttribute("role", "group");
  saveForm.setAttribute("aria-label", "Save current values as preset");

  const nameInput = document.createElement("input");
  nameInput.type = "text";
  nameInput.id = "preset-save-name";
  nameInput.placeholder = "Preset name";
  nameInput.maxLength = 64;
  nameInput.setAttribute("autocomplete", "off");
  nameInput.setAttribute("aria-label", "Preset name");
  nameInput.setAttribute("aria-describedby", "preset-save-name-err");

  const nameErr = document.createElement("div");
  nameErr.className = "field-error";
  nameErr.id = "preset-save-name-err";
  nameErr.hidden = true;

  const saveBtn = document.createElement("button");
  saveBtn.type = "button";
  saveBtn.className = "btn-primary preset-save-btn";
  saveBtn.textContent = "Save";

  const cancelBtn = document.createElement("button");
  cancelBtn.type = "button";
  cancelBtn.className = "btn-secondary preset-save-cancel";
  cancelBtn.textContent = "Cancel";

  saveForm.appendChild(nameInput);
  saveForm.appendChild(nameErr);
  saveForm.appendChild(saveBtn);
  saveForm.appendChild(cancelBtn);
  zone.appendChild(saveForm);

  // Toggle show/hide
  toggleBtn.addEventListener("click", () => {
    const expanded = toggleBtn.getAttribute("aria-expanded") === "true";
    if (expanded) {
      saveForm.hidden = true;
      toggleBtn.setAttribute("aria-expanded", "false");
    } else {
      saveForm.hidden = false;
      toggleBtn.setAttribute("aria-expanded", "true");
      nameInput.focus();
    }
  });

  cancelBtn.addEventListener("click", () => {
    saveForm.hidden = true;
    toggleBtn.setAttribute("aria-expanded", "false");
    nameInput.value = "";
    nameErr.textContent = "";
    nameErr.hidden = true;
  });

  saveBtn.addEventListener("click", async () => {
    if (!assertOnlineForPreset()) return;

    const rawName = nameInput.value.trim();
    // Validate: 1–64 chars, no '/'
    if (!rawName || rawName.length > 64) {
      nameErr.textContent = "Name must be 1–64 characters.";
      nameErr.hidden = false;
      return;
    }
    if (rawName.includes("/")) {
      nameErr.textContent = "Name may not contain '/'.";
      nameErr.hidden = false;
      return;
    }
    if (rawName.toLowerCase() === "order") {
      nameErr.textContent = "'order' is reserved — pick another name.";
      nameErr.hidden = false;
      return;
    }
    nameErr.hidden = true;

    // Check for existing preset with same name (silent-overwrite guard — MAJOR 7)
    const existingPreset = _presetsCache.find(p => p.name === rawName);
    if (existingPreset && !saveBtn.dataset.overwritePending) {
      nameErr.textContent = "A preset named '" + rawName + "' already exists. Tap Save again to overwrite.";
      nameErr.hidden = false;
      saveBtn.dataset.overwritePending = "true";
      const resetOverwrite = setTimeout(() => {
        saveBtn.dataset.overwritePending = "";
        nameErr.hidden = true;
        nameErr.textContent = "";
      }, 5000);
      nameInput.addEventListener("input", () => {
        clearTimeout(resetOverwrite);
        saveBtn.dataset.overwritePending = "";
      }, { once: true });
      return;
    }
    saveBtn.dataset.overwritePending = "";

    // Collect visible, non-excluded field values (mirrors TUI preset save logic)
    const values = collectPresetValues(mod);

    saveBtn.disabled = true;
    saveBtn.textContent = "Saving…";

    try {
      const resp = await apiFetch(
        "/api/v1/presets/" + encodeURIComponent(mod.key) + "/" + encodeURIComponent(rawName),
        {
          method: "PUT",
          body: JSON.stringify({ description: "", values }),
        }
      );

      if (resp.ok) {
        // Re-render chip row from response (upsert returns the single preset;
        // re-fetch to get the canonical ordered list)
        const freshPresets = await fetchPresets(mod.key);
        if (freshPresets !== null) {
          refreshPresetChipRowFromList(mod, freshPresets);
        } else {
          showToast("Saved — but couldn't refresh preset list.");
        }
        // Collapse the save form
        saveForm.hidden = true;
        toggleBtn.setAttribute("aria-expanded", "false");
        nameInput.value = "";
      } else {
        const err = await resp.json().catch(() => ({}));
        const msg = (err.error && err.error.message) || ("Save failed: " + resp.status);
        // Show inline error for validation failures; toast for server errors
        if (resp.status === 400) {
          nameErr.textContent = msg;
          nameErr.hidden = false;
        } else {
          showToast(msg);
        }
      }
    } catch (e) {
      const msg = e?.message || String(e);
      if (!msg.startsWith("Unauthorized")) showToast("Save failed — server unreachable.");
    } finally {
      saveBtn.disabled = false;
      saveBtn.textContent = "Save";
    }
  });

  // Allow Enter key in name input to submit
  nameInput.addEventListener("keydown", e => {
    if (e.key === "Enter") { e.preventDefault(); saveBtn.click(); }
    if (e.key === "Escape") { cancelBtn.click(); }
  });

  return zone;
}

// Collect current form values suitable for saving as a preset.
// Excludes: composite_array fields, preset_exclude=true fields, hidden fields.
// Mirrors src/data/presets.rs TUI save logic.
function collectPresetValues(mod) {
  const fields = mod.fields || [];
  const allVals = readCurrentFieldValues();
  const visible = computeVisible(fields, allVals);
  const values = {};
  for (const f of fields) {
    if (f.field_type === "composite_array") continue;
    if (f.preset_exclude) continue;
    if (!visible.has(f.name)) continue;
    const el = document.getElementById("field-" + f.name);
    if (el) {
      // Mirror TUI src/tui/form.rs:2731 — skip fields whose value is empty.
      // TUI uses !val.is_empty() (raw, no trim). Match exactly.
      const val = el.value;
      if (val !== "") values[f.name] = val;
    }
  }
  return values;
}

// Apply a preset (or null to clear). Mirrors TUI Presets::apply semantics (§3.4):
//   - Fields in preset.values → set to preset value
//   - Fields absent from preset.values → reset to field default or empty
//   - Fields with preset_exclude = true → not overwritten (keep current DOM value)
//   - After apply → run computeVisible so dependent fields appear/disappear
function applyPreset(mod, preset) {
  _activePreset = preset ? preset.name : null;

  // Update chip active states
  const row = document.getElementById("preset-chip-row");
  if (row) {
    for (const chip of row.querySelectorAll(".preset-chip")) {
      const isActive = chip.dataset.presetName === (_activePreset || "");
      chip.setAttribute("aria-pressed", isActive ? "true" : "false");
      chip.classList.toggle("preset-chip--active", isActive);
    }
  }

  const fields = mod.fields || [];
  const presetValues = preset ? (preset.values || {}) : {};

  for (const field of fields) {
    // preset_exclude: skip — do not overwrite the current value
    if (field.preset_exclude) continue;
    if (field.field_type === "composite_array") continue;

    const el = document.getElementById("field-" + field.name);
    if (!el) continue;

    if (preset && Object.prototype.hasOwnProperty.call(presetValues, field.name)) {
      // Field present in preset: set to preset value
      el.value = presetValues[field.name];
    } else {
      // Field absent from preset (or <none> applied): reset to default or empty
      el.value = field.default || "";
    }
  }

  // Re-run visibility so show_when fields appear/disappear after preset apply
  const allVals = readCurrentFieldValues();
  recomputeVisibility(fields, allVals);
}

// ---------------------------------------------------------------------------
// Options fetching
// ---------------------------------------------------------------------------

async function fetchOptions(moduleKey, fieldName) {
  const cacheKey = moduleKey + ":" + fieldName;
  try {
    const resp = await apiFetch("/api/v1/options/" + moduleKey + "/" + fieldName);
    if (resp.ok) {
      const data = await resp.json();
      _optionsCache[cacheKey] = data.options || [];
    }
  } catch (_e) {
    _optionsCache[cacheKey] = [];
  }
}

// ---------------------------------------------------------------------------
// Submit
// ---------------------------------------------------------------------------

async function handleSubmit() {
  clearFieldErrors();

  const fieldValues = collectValues();
  const compositeData = collectCompositeData();

  // Validate required visible fields client-side
  const fields = _currentModuleData ? (_currentModuleData.fields || []) : [];
  const visible = computeVisible(fields, fieldValues);
  let hasError = false;
  for (const f of fields) {
    if (!visible.has(f.name)) continue;
    if (f.field_type === "composite_array") continue;
    if (f.required && !(fieldValues[f.name] || "").trim()) {
      showFieldError(f.name, "Required");
      hasError = true;
    }
  }
  if (hasError) return;

  // idempotency-key persists across retries within a form session.
  // Generate once; rotate only after 2xx success or explicit form reset.
  if (!_pendingIdempotencyKey) {
    _pendingIdempotencyKey = uuidv4();
  }
  const idempotencyKey = _pendingIdempotencyKey;
  const capturedAt = new Date().toISOString();

  // TASK-2.3.3 — Include auto_create_inputs when the map is non-empty.
  // The map is keyed by parent field name → template field values map.
  // All values are strings per contract §6.4 (empty inputs as "", not null).
  // The map accumulates via overlay confirm; cleared on reset and after 2xx.
  const hasAutoCreate = Object.keys(_pendingAutoCreateInputs).length > 0;
  const body = {
    field_values: fieldValues,
    composite_data: Object.keys(compositeData).length > 0 ? compositeData : undefined,
    auto_create_inputs: hasAutoCreate ? _pendingAutoCreateInputs : undefined,
    captured_at: capturedAt,
    client_id: "phone-pwa",
  };

  const submitBtn = document.querySelector(".form-actions button[type='submit']");
  if (submitBtn) { submitBtn.disabled = true; submitBtn.textContent = "Pouring..."; }

  try {
    const resp = await apiFetch("/api/v1/submit/" + _currentModule, {
      method: "POST",
      body: JSON.stringify(body),
      headers: { "Idempotency-Key": idempotencyKey },
    });

    if (resp.status === 201) {
      const data = await resp.json();
      // Check for Idempotency-Replay header (TASK-2.1.5 — edit-and-resubmit path).
      // If the server replayed a cached 201 (prior drain succeeded), show a distinct toast.
      const isReplay = resp.headers.get('Idempotency-Replay') === 'true';
      if (isReplay) {
        showToast('This was already saved — showing the original.', 5000);
      }
      // Success — rotate key and clear auto-create state for the new session.
      _pendingIdempotencyKey = null;
      _pendingAutoCreateInputs = {};
      // If this was an edit of a queued record (TASK-2.1.5), clean it up from IDB.
      if (_editingQueueId !== null) {
        const editId = _editingQueueId;
        _editingQueueId = null;
        removeQueueRecord(editId).catch(() => {}); // non-fatal if already gone
        _queueCount = Math.max(0, _queueCount - 1);
        updateQueueBadge();
      }
      showSummary(data);
      return;
    }

    // CRITICAL 1 FIX: 507 must be checked BEFORE (and independently of) 202.
    // The SW returns synthetic 507 on QuotaExceededError — this is a distinct
    // status from the synthetic 202 queued-ok path. They can never be both.
    if (resp.status === 507) {
      showToast("Queue is full — clear discarded captures or sync existing ones first.", 8000);
      return;
    }

    // TASK-2.1.2 — Synthetic 202: submit was queued offline by the service worker.
    // The SW emits this when the network is unreachable or the server returns 5xx.
    // We show a "Queued" summary view, NOT the normal "Saved" view.
    // Idempotency-Key is NOT rotated here — the same key will be reused on drain
    // (contract §9 round 5: same key + recoverable error = re-execute).
    // Once the drain succeeds (2xx), the key is effectively consumed server-side.
    if (resp.status === 202) {
      const data = await resp.json().catch(() => ({}));
      // Key is NOT rotated — will be reused by drain
      // _pendingAutoCreateInputs cleared so the same body isn't re-accumulated
      _pendingAutoCreateInputs = {};
      showQueuedSummary(data);
      // Refresh badge count after a new record is queued
      refreshQueueBadge();
      return;
    }

    // Error handling — key is NOT rotated; next tap reuses same key (idempotent retry)
    const err = await resp.json().catch(() => ({}));
    if (resp.status === 400 && err.error && err.error.details && err.error.details.fields) {
      const fieldErrors = err.error.details.fields;

      // TASK-2.3.5 — Detect auto_create_input_required: re-open overlay with banner.
      // This is a defensive code path; if TASK-2.3.3 is correct it should be unreachable.
      const autoCreateRequired = err.error.details && err.error.details.code === "auto_create_input_required";
      if (autoCreateRequired) {
        // Find which field triggered it
        const problemField = fieldErrors.find(fe => fe.code === "auto_create_input_required");
        if (problemField && _overlayContext && _overlayContext.fieldName === problemField.field) {
          reopenSubformWithErrors(
            problemField.field,
            [],
            "Please complete the template before submitting."
          );
        } else if (problemField) {
          // Context was cleared — try to re-open
          const parentField = (_currentModuleData ? (_currentModuleData.fields || []) : [])
            .find(f => f.name === problemField.field);
          if (parentField && parentField.create_template) {
            const parentInput = document.getElementById("field-" + problemField.field);
            const val = parentInput ? parentInput.value : "";
            // MAJOR 4 fix: guard re-open to avoid clobbering partial user input.
            // If overlay is already open (e.g. iOS Safari 400-response race), just
            // show the top error banner on the existing overlay without re-rendering.
            if (val && !_overlayContext) {
              openSubformOverlay(problemField.field, parentField.create_template, val);
            }
            showSubformTopError("Please complete the template before submitting.");
          }
        }
        return;
      }

      // TASK-2.3.5 — Route validation_failed field errors to the parent form.
      // The server's details.fields[] contains ONLY parent-field names (e.g. "bean").
      // Template-field errors never appear here — they come via auto_create_input_required
      // (handled above at line 2189). Routing all entries to the parent is correct and
      // avoids false-positive overlay routing when a parent field shares a name with a
      // template field.
      for (const fe of fieldErrors) {
        showFieldError(fe.field, fe.code || "invalid");
      }
    } else {
      const msg = (err.error && err.error.message) || ("Submit failed: " + resp.status);
      showToast(msg);
    }
  } catch (e) {
    // apiFetch throws "Unauthorized …" and already swapped to token-gate view.
    // No toast needed — showing one would be misleading.
    const msg = e?.message || String(e);
    if (msg.startsWith("Unauthorized")) return;
    // Network/5xx error — key preserved for idempotent retry
    showToast("Submit failed — server unreachable.");
  } finally {
    if (submitBtn) { submitBtn.disabled = false; submitBtn.textContent = "Pour"; }
  }
}

// TASK-2.1.2 — Show "Queued" summary when the SW queued the submit offline.
// Distinct from showSummary ("Saved") — the capture exists in IDB but has not
// reached the server yet. The user can see it in the queue badge panel (TASK-2.1.4).
function showQueuedSummary(data) {
  const msgEl = document.getElementById("summary-message");
  msgEl.textContent = "Queued — will sync when online.";

  // Show the module key or captured_at as context so the user knows what was queued.
  const pathEl = document.getElementById("summary-path");
  if (data && data.captured_at) {
    pathEl.textContent = "Captured at " + new Date(data.captured_at).toLocaleString();
  } else {
    pathEl.textContent = "";
  }

  const transportEl = document.getElementById("summary-transport");
  transportEl.textContent = "Offline";

  const warningContainer = document.getElementById("summary-warnings");
  if (warningContainer) warningContainer.innerHTML = "";

  showView("summary");
}

function showSummary(data) {
  document.getElementById("summary-message").textContent = "Entry saved.";
  document.getElementById("summary-path").textContent = data.vault_path || "";
  document.getElementById("summary-transport").textContent = data.transport_mode || "";

  // TASK-2.3.5 — Show non-fatal warning chips when autocreate succeeded partially.
  // contract §6.4: 201 body MAY include warnings[].code === "autocreate_failed"
  const warningContainer = document.getElementById("summary-warnings");
  if (warningContainer) warningContainer.innerHTML = "";

  const warnings = data.warnings || [];
  for (const w of warnings) {
    if (w.code === "autocreate_failed") {
      // NOTE: per contract §14 we do not log user content.
      // The message from the server is safe to display (server sanitizes it).
      if (!warningContainer) continue;
      const chip = document.createElement("div");
      chip.className = "autocreate-warning";
      // w.field is the parent field name (e.g. "bean") — server-supplied, not user content.
      // w.message is intentionally code-only on the server (no user-typed values, no
      // filesystem paths). textContent prevents XSS regardless, but the server guarantee
      // means the message is safe to display as human-readable context.
      chip.textContent = "Saved, but note creation for '" + (w.field || "") + "' failed: " + (w.message || "");
      warningContainer.appendChild(chip);
    }
  }

  showView("summary");
}

// ---------------------------------------------------------------------------
// Field error helpers
// ---------------------------------------------------------------------------

function clearFieldErrors() {
  for (const el of document.querySelectorAll(".field-error")) {
    el.textContent = "";
    el.hidden = true;
  }
}

function showFieldError(fieldName, msg) {
  const el = document.getElementById("err-" + fieldName);
  if (el) { el.textContent = msg; el.hidden = false; }
}

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

function escapeHtml(s) {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

// Pour serves over plain HTTP on the LAN, so crypto.randomUUID() (secure-contexts-only)
// is unavailable on http:// origins. This fallback uses crypto.getRandomValues which
// works in any context (including http://), falling back to Math.random as last resort.
function uuidv4() {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  // Fallback for non-secure contexts (e.g. http:// LAN IP).
  // Uses crypto.getRandomValues if available, else Math.random.
  const bytes = new Uint8Array(16);
  if (typeof crypto !== "undefined" && typeof crypto.getRandomValues === "function") {
    crypto.getRandomValues(bytes);
  } else {
    for (let i = 0; i < 16; i++) bytes[i] = Math.floor(Math.random() * 256);
  }
  // Set version (4) and variant (10xx) bits per RFC 4122.
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = [...bytes].map(b => b.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0,8)}-${hex.slice(8,12)}-${hex.slice(12,16)}-${hex.slice(16,20)}-${hex.slice(20)}`;
}

// ---------------------------------------------------------------------------
// Phase 2 Stream A — Service worker registration (TASK-2.2.2)
// ---------------------------------------------------------------------------

// Register the service worker exactly once on DOMContentLoaded, AFTER token
// bootstrap. Registration failure MUST NOT block the form from rendering —
// the PWA works without a SW (offline features just won't be available).
function registerServiceWorker() {
  if (!('serviceWorker' in navigator)) return; // SW not supported — degrade silently
  navigator.serviceWorker.register('/sw.js', { scope: '/' })
    .then(reg => {
      // TASK-2.2.3: check if a new SW is waiting to activate.
      // This handles the case where the user opened the app, a new deploy happened,
      // and the new SW installed but is waiting for the old one to yield.
      if (reg.waiting) {
        showUpdateBanner();
      }
      reg.addEventListener('updatefound', () => {
        const newWorker = reg.installing;
        if (newWorker) {
          newWorker.addEventListener('statechange', () => {
            if (newWorker.state === 'installed' && navigator.serviceWorker.controller) {
              // New SW installed and ready, old one still controlling — show banner.
              showUpdateBanner();
            }
          });
        }
      });
    })
    .catch(err => {
      // Registration failed — log to console only, never toast (§14: low-chatter).
      console.warn('pour: SW registration failed', err);
    });

  // TASK-2.2.3: listen for SW_UPDATED message from newly activated SW.
  navigator.serviceWorker.addEventListener('message', handleSwMessage);

  // CRITICAL 4: On page load, if an SW is already controlling (return visit) and
  // the network is available, trigger a drain. The SW needs the page open to get
  // a fresh token — this ensures queued records sync when the app reopens online.
  if (navigator.serviceWorker.controller && navigator.onLine) {
    navigator.serviceWorker.controller.postMessage({ type: 'DRAIN_NOW' });
  }

  // TASK-2.1.3 (Safari fallback): when the page comes online, tell the SW to drain.
  // Background Sync API is not available on iOS Safari; this event fires when the
  // device's network returns. The SW's drainQueue() is identical on both paths.
  window.addEventListener('online', () => {
    if (navigator.serviceWorker.controller) {
      navigator.serviceWorker.controller.postMessage({ type: 'DRAIN_NOW' });
    }
    // CRITICAL 5: network returned — clear "Network unreachable" if it was showing.
    // The drain will set "Syncing…" via DRAIN_STARTED once it begins.
    if (_queuePanelOpen) setQueueSyncStatus('');
  });

  // CRITICAL 5: network lost — if the queue panel is open, show status immediately.
  window.addEventListener('offline', () => {
    if (_queuePanelOpen) setQueueSyncStatus('offline');
  });
}

// ---------------------------------------------------------------------------
// Phase 2 Stream A — SW message handler (TASK-2.1.3, TASK-2.1.4)
// ---------------------------------------------------------------------------

// Handle postMessages from the service worker.
// These update the queue badge without requiring a full IDB re-read.
function handleSwMessage(event) {
  if (!event.data) return;

  switch (event.data.type) {
    case 'SW_UPDATED':
      // TASK-2.2.3: new SW activated (on cold start — the previous tab was already
      // closed). Show banner so the user knows to refresh for the latest version.
      showUpdateBanner();
      break;

    case 'QUEUED':
      // TASK-2.1.2: a submit was queued offline. Increment badge count.
      _queueCount++;
      updateQueueBadge();
      break;

    case 'DRAINED':
      // TASK-2.1.3: a record was drained successfully. Decrement badge count (min 0).
      _queueCount = Math.max(0, _queueCount - 1);
      updateQueueBadge();
      if (_queuePanelOpen) refreshQueuePanel();
      break;

    case 'DRAIN_STARTED':
      // CRITICAL 5: SW started draining — show "Syncing…" in the queue panel status.
      setQueueSyncStatus('syncing');
      break;

    case 'DRAIN_FINISHED':
      // CRITICAL 5: SW finished a drain pass. If queue is now empty, show "Synced"
      // briefly; if records remain (partial drain / 4xx poison pills), stay neutral.
      if (_queueCount === 0) {
        setQueueSyncStatus('synced');
      } else {
        setQueueSyncStatus('');
      }
      break;

    case 'DRAIN_ERROR':
      // TASK-2.1.3: a drain attempt returned 4xx. Badge stays (record is still in
      // queue). If the panel is open, refresh it to show the updated error state.
      if (_queuePanelOpen) refreshQueuePanel();
      break;

    case 'GET_TOKEN':
      // CRITICAL 4: SW is asking this page for its current auth token so it can
      // drain queued records without using a stale cached token. Respond via the
      // MessageChannel port the SW passed with the request.
      // This runs in the page context where localStorage is accessible.
      if (event.ports && event.ports[0]) {
        const token = getToken();
        event.ports[0].postMessage({
          auth_header: token ? ('Bearer ' + token) : null,
        });
      }
      break;
  }
}

// ---------------------------------------------------------------------------
// CRITICAL 5: Queue sync status indicator (#queue-sync-status)
// ---------------------------------------------------------------------------
// States: '' (clear/hidden) | 'syncing' | 'synced' | 'offline'
// Written by SW message events (DRAIN_STARTED/DRAIN_FINISHED) and by the
// window online/offline events when the panel is open.

let _syncStatusClearTimer = null;

function setQueueSyncStatus(state) {
  const el = document.getElementById('queue-sync-status');
  if (!el) return;

  clearTimeout(_syncStatusClearTimer);
  el.className = 'queue-sync-status'; // reset modifier classes

  if (state === 'syncing') {
    el.textContent = 'Syncing…';
    el.classList.add('queue-sync-status--syncing');
    el.hidden = false;
  } else if (state === 'synced') {
    el.textContent = 'Synced';
    el.classList.add('queue-sync-status--synced');
    el.hidden = false;
    // Auto-clear after 3 s
    _syncStatusClearTimer = setTimeout(() => setQueueSyncStatus(''), 3000);
  } else if (state === 'offline') {
    el.textContent = 'Network unreachable';
    el.classList.add('queue-sync-status--offline');
    el.hidden = false;
  } else {
    // Empty state — hide element
    el.textContent = '';
    el.hidden = true;
  }
}

// ---------------------------------------------------------------------------
// Phase 2 Stream A — Update banner (TASK-2.2.3)
// ---------------------------------------------------------------------------

function showUpdateBanner() {
  let banner = document.getElementById('sw-update-banner');
  if (banner) return; // already showing

  banner = document.createElement('div');
  banner.id = 'sw-update-banner';
  banner.className = 'sw-update-banner';
  banner.setAttribute('role', 'status');

  const msg = document.createElement('span');
  msg.textContent = 'New version available';

  // "Tap to refresh" button — user-initiated skipWaiting (NEVER auto-skipWaiting).
  const btn = document.createElement('button');
  btn.type = 'button';
  btn.className = 'sw-update-btn';
  btn.textContent = 'tap to refresh';
  btn.addEventListener('click', () => {
    if (navigator.serviceWorker.controller) {
      navigator.serviceWorker.controller.postMessage({ type: 'SKIP_WAITING' });
    }
    // Reload once the new SW has claimed the page.
    navigator.serviceWorker.addEventListener('controllerchange', () => {
      window.location.reload();
    }, { once: true });
    // Fallback reload in case controllerchange doesn't fire (e.g. already controlled).
    setTimeout(() => window.location.reload(), 1000);
  });

  // "Dismiss" button — hides the banner for this session; the next session still
  // shows the banner until the user reloads (the new SW activates on next cold start).
  const dismiss = document.createElement('button');
  dismiss.type = 'button';
  dismiss.className = 'sw-update-dismiss';
  dismiss.setAttribute('aria-label', 'Dismiss update notification');
  dismiss.textContent = '×';
  dismiss.addEventListener('click', () => banner.remove());

  banner.appendChild(msg);
  banner.appendChild(btn);
  banner.appendChild(dismiss);

  // Insert after header, before main content.
  const header = document.querySelector('header');
  if (header && header.nextSibling) {
    document.body.insertBefore(banner, header.nextSibling);
  } else {
    document.body.prepend(banner);
  }
}

// ---------------------------------------------------------------------------
// Phase 2 Stream A — Queue badge (TASK-2.1.4)
// ---------------------------------------------------------------------------

// Read the IDB queue count and update the badge. Called on init and after
// page reloads (badge survives reload because IDB persists).
async function refreshQueueBadge() {
  try {
    // countQueue is in queue.js; queue.js is loaded as a separate <script>.
    // In the SW context, IDB is accessed via swOpenQueue. In the page context,
    // we import queue.js via a <script> tag added to index.html.
    // For now, list the queue and count records (listQueue is available from queue.js).
    const records = await listQueue();
    _queueCount = records.length;
  } catch (_e) {
    _queueCount = 0;
  }
  updateQueueBadge();
}

// Update the badge DOM element from _queueCount.
function updateQueueBadge() {
  const badge = document.getElementById('queue-badge');
  if (!badge) return;
  if (_queueCount > 0) {
    badge.textContent = 'Queued (' + _queueCount + ')';
    badge.hidden = false;
  } else {
    badge.hidden = true;
    // Close panel if it was open and queue is now empty.
    if (_queuePanelOpen) closeQueuePanel();
  }
}

// Open the queue panel — shows pending records.
// Per §14: shows module key + relative queued_at only. NO field values shown.
async function openQueuePanel() {
  _queuePanelOpen = true;
  const panel = document.getElementById('queue-panel');
  if (!panel) return;
  panel.hidden = false;
  // CRITICAL 5: set initial sync status when panel opens based on current network state.
  if (!navigator.onLine) {
    setQueueSyncStatus('offline');
  } else {
    setQueueSyncStatus('');
  }
  await refreshQueuePanel();
}

function closeQueuePanel() {
  _queuePanelOpen = false;
  const panel = document.getElementById('queue-panel');
  if (panel) panel.hidden = true;
  // CRITICAL 5: clear sync status when panel closes — stale "Syncing…" is misleading.
  setQueueSyncStatus('');
}

// Re-render the queue panel contents from IDB.
async function refreshQueuePanel() {
  const list = document.getElementById('queue-panel-list');
  if (!list) return;

  let records;
  try {
    records = await listQueue();
  } catch (_e) {
    records = [];
  }

  list.innerHTML = '';

  if (records.length === 0) {
    const empty = document.createElement('p');
    empty.className = 'queue-empty';
    empty.textContent = 'No queued captures.';
    list.appendChild(empty);
    return;
  }

  for (const record of records) {
    const item = document.createElement('div');
    item.className = 'queue-item';
    item.dataset.queueId = record.id;

    const info = document.createElement('div');
    info.className = 'queue-item-info';

    const moduleEl = document.createElement('span');
    moduleEl.className = 'queue-item-module';
    // module key only — no field values (§14 / memory feedback_pour_aesthetic.md)
    moduleEl.textContent = record.module_key;

    const timeEl = document.createElement('span');
    timeEl.className = 'queue-item-time';
    timeEl.textContent = relativeTime(record.queued_at);

    if (record.last_error) {
      const errEl = document.createElement('span');
      errEl.className = 'queue-item-error';
      errEl.textContent = record.last_error; // error code only (§14)
      info.appendChild(moduleEl);
      info.appendChild(timeEl);
      info.appendChild(errEl);
    } else {
      info.appendChild(moduleEl);
      info.appendChild(timeEl);
    }

    // TASK-2.1.5 action buttons
    const actions = document.createElement('div');
    actions.className = 'queue-item-actions';

    // Retry now — re-fires drain for this record only
    const retryBtn = document.createElement('button');
    retryBtn.type = 'button';
    retryBtn.className = 'queue-btn-retry';
    retryBtn.textContent = 'Retry';
    retryBtn.addEventListener('click', () => retryQueueRecord(record.id));

    // Edit — pre-fills the form with the queued body, retains same Idempotency-Key
    const editBtn = document.createElement('button');
    editBtn.type = 'button';
    editBtn.className = 'queue-btn-edit';
    editBtn.textContent = 'Edit';
    editBtn.addEventListener('click', () => editQueueRecord(record));

    // Discard — single tap confirm pattern (low-friction per memory feedback_pour_aesthetic.md)
    const discardBtn = document.createElement('button');
    discardBtn.type = 'button';
    discardBtn.className = 'queue-btn-discard';
    discardBtn.textContent = 'Discard';
    discardBtn.dataset.confirmPending = 'false';
    discardBtn.addEventListener('click', () => {
      if (discardBtn.dataset.confirmPending === 'true') {
        discardQueueRecord(record.id);
      } else {
        discardBtn.dataset.confirmPending = 'true';
        discardBtn.textContent = 'Confirm?';
        setTimeout(() => {
          if (discardBtn.dataset.confirmPending === 'true') {
            discardBtn.dataset.confirmPending = 'false';
            discardBtn.textContent = 'Discard';
          }
        }, 3000);
      }
    });

    actions.appendChild(retryBtn);
    actions.appendChild(editBtn);
    actions.appendChild(discardBtn);

    item.appendChild(info);
    item.appendChild(actions);
    list.appendChild(item);
  }
}

// TASK-2.1.5: Discard a queued record — deletes from IDB, no resubmit.
async function discardQueueRecord(id) {
  try {
    await removeQueueRecord(id);
    _queueCount = Math.max(0, _queueCount - 1);
    updateQueueBadge();
    await refreshQueuePanel();
  } catch (_e) {
    showToast('Failed to discard queued capture.');
  }
}

// TASK-2.1.5: Retry a single queued record immediately.
// Sends a DRAIN_NOW message to the SW, which runs the full drainQueue().
// The SW will process records in FIFO order, so this record may not be
// the first to drain if there are earlier records in the queue.
function retryQueueRecord(_id) {
  if (navigator.serviceWorker && navigator.serviceWorker.controller) {
    navigator.serviceWorker.controller.postMessage({ type: 'DRAIN_NOW' });
    showToast('Attempting to sync…', 2000);
  } else {
    showToast('Service worker not active — try refreshing.');
  }
}

// TASK-2.1.5: Edit a queued record — pre-fills the form, retains Idempotency-Key.
//
// The same Idempotency-Key is reused. Contract §9 (round 5): if the server
// has a cached 201 for this key (meaning a prior drain attempt succeeded but
// the page never knew), the resubmit replays the cached response — user sees
// "This was already saved". If no cached response, the edited body executes fresh.
//
// captured_at stays ORIGINAL (the user is editing content, not the moment).
// The IDB record is NOT deleted until the edit is resubmitted successfully.
async function editQueueRecord(record) {
  // Close the queue panel first
  closeQueuePanel();

  if (!record.body) return;

  // Find the module config
  const moduleKey = record.module_key;
  if (!_config) {
    showToast('Config not loaded — tap a module to reload.');
    return;
  }
  const mod = (_config.modules || []).find(m => m.key === moduleKey);
  if (!mod) {
    showToast('Module "' + moduleKey + '" not found in current config.');
    return;
  }

  // Pre-fill the form with the queued body.
  _currentModule = moduleKey;
  _currentModuleData = mod;
  _presetsCache = [];
  _activePreset = null;

  // Restore the original Idempotency-Key — NOT rotated (critical for §9 compliance).
  _pendingIdempotencyKey = record.idempotency_key;

  // Restore auto_create_inputs if the queued body had them.
  _pendingAutoCreateInputs = record.body.auto_create_inputs || {};

  // Render the form pre-filled with the queued field values.
  renderFormSkeleton(mod);
  showView('form');

  // Pre-fetch options for dynamic_select fields.
  _optionsCache = {};
  const dynFields = (mod.fields || []).filter(f => f.field_type === 'dynamic_select');
  const [, presets] = await Promise.all([
    Promise.all(dynFields.map(f => fetchOptions(moduleKey, f.name))),
    fetchPresets(moduleKey),
  ]);
  _presetsCache = presets || [];

  // Render with the queued field values pre-filled.
  const fieldValues = record.body.field_values || {};
  renderForm(mod, fieldValues);

  // Show a banner indicating this is an edit of a queued capture.
  const capturedAt = record.body.captured_at;
  const capturedLabel = capturedAt
    ? 'Editing queued capture from ' + new Date(capturedAt).toLocaleString()
    : 'Editing queued capture';
  showToast(capturedLabel, 5000);

  // When this form is submitted, if the server responds with Idempotency-Replay: true,
  // show "This was already saved — opened the original." per contract §9 round 5.
  // The submit handler in handleSubmit reads the Idempotency-Replay header and
  // shows the toast. The IDB record is deleted after a successful resubmit (see
  // handleSubmit's 201 path — the key rotation signals success).
  // Store the queue id in a module-level variable so handleSubmit can clean up IDB.
  _editingQueueId = record.id;
}

// ID of the IDB record being edited via editQueueRecord(), or null.
// Set by editQueueRecord, cleared after successful submit.
let _editingQueueId = null;

// Helper: format a queued_at ISO string as a relative time ("3m ago", "2h ago", etc.)
function relativeTime(isoString) {
  if (!isoString) return '';
  const diff = Date.now() - new Date(isoString).getTime();
  if (isNaN(diff)) return '';
  const seconds = Math.floor(diff / 1000);
  if (seconds < 60) return seconds + 's ago';
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return minutes + 'm ago';
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return hours + 'h ago';
  const days = Math.floor(hours / 24);
  return days + 'd ago';
}

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------

document.addEventListener("DOMContentLoaded", async () => {
  const token = getToken();
  if (!token) {
    showView("token-gate");
    return;
  }

  // TASK-2.2.2: register service worker AFTER token bootstrap (so we know auth
  // is ready before any SW-intercepted submits could fire). Registration failure
  // MUST NOT block the form — graceful degradation.
  registerServiceWorker();

  // TASK-2.1.4: read initial queue count from IDB (badge survives page reload).
  refreshQueueBadge().catch(() => {});

  const ok = await checkHealth();
  if (!ok) {
    // Health check sets view if auth fails; for connectivity failures, stay put
    return;
  }

  // Check if ?m= deep-link to a module
  const params = new URLSearchParams(window.location.search);
  const moduleParam = params.get("m");

  await loadDashboard();

  if (moduleParam && _config) {
    const mod = (_config.modules || []).find(m => m.key === moduleParam);
    if (mod) await openForm(moduleParam);
  }
});

// Wire up summary buttons
document.addEventListener("DOMContentLoaded", () => {
  document.getElementById("btn-pour-another").addEventListener("click", () => {
    // Rotate idempotency key for the new session (user explicitly started fresh)
    _pendingIdempotencyKey = null;
    _pendingAutoCreateInputs = {};
    // CRITICAL 2 FIX: "Pour Another" is a fresh session — not a continuation of a
    // queue edit. Clear so the abandoned queued record is not deleted on this submit.
    _editingQueueId = null;
    if (_currentModule) openForm(_currentModule);
    else loadDashboard();
  });
  document.getElementById("btn-dashboard").addEventListener("click", () => {
    _pendingIdempotencyKey = null;
    _pendingAutoCreateInputs = {};
    loadDashboard();
  });
});

// Wire up sub-form overlay buttons (TASK-2.3.1, 2.3.3, 2.3.4)
document.addEventListener("DOMContentLoaded", () => {
  document.getElementById("subform-confirm").addEventListener("click", confirmSubform);
  document.getElementById("subform-cancel").addEventListener("click", cancelSubform);
  // Tap on scrim (outside the panel) triggers cancel
  document.getElementById("subform-scrim").addEventListener("click", cancelSubform);
});

// TASK-2.1.4: Wire up queue badge tap → open panel; panel close button.
document.addEventListener("DOMContentLoaded", () => {
  const badge = document.getElementById("queue-badge");
  if (badge) {
    badge.addEventListener("click", () => {
      if (_queuePanelOpen) {
        closeQueuePanel();
        badge.setAttribute("aria-expanded", "false");
      } else {
        openQueuePanel();
        badge.setAttribute("aria-expanded", "true");
      }
    });
  }

  const closeBtn = document.getElementById("queue-panel-close");
  if (closeBtn) {
    closeBtn.addEventListener("click", () => {
      closeQueuePanel();
      if (badge) badge.setAttribute("aria-expanded", "false");
    });
  }
});

// ---------------------------------------------------------------------------
// Phase 2 Stream D — History view state
// ---------------------------------------------------------------------------

// Whether the history view has been loaded at least once (lazy-load).
let _historyLoaded = false;

// Cursor for next-page fetch. null = no more pages, undefined = not yet fetched.
let _historyNextCursor = undefined;

// true while a history page fetch is in flight (prevents duplicate requests).
let _historyFetchInFlight = false;

// Whether there are more pages on the server (from last response).
let _historyHasMore = false;

// ---------------------------------------------------------------------------
// Phase 2 Stream D — Back-button state machine (TASK-2.5.6 + inspector fixes)
//
// Two levels of pushState per session:
//   Level 1: dashboard → history view
//     { view: "history" }
//   Level 2: history view → capture panel open
//     { view: "history", panel: <historyId> }
//
// Rules:
//   - _historyViewPushed is set to true on the FIRST dashboard→history
//     transition per session. Subsequent calls to openHistoryTab() do NOT
//     push again (e.g. capture-tab click after a popstate round-trip resets
//     _historyViewPushed so the next open pushes fresh).
//   - openCapturePanel() always pushes Level 2 after the panel is shown.
//   - closeCapturePanel(fromPopstate) — when called by the popstate handler
//     it does NOT call history.back() (that would double-pop); when called
//     by the close button it calls history.back() so the Level 2 entry is
//     consumed and the URL returns to the Level 1 state.
//   - popstate handler:
//       • If the NEW state has panel!=null → open panel (shouldn't normally
//         occur in forward navigation, defensive only).
//       • If the NEW state has view==="history" AND OLD state had panel →
//         close the panel WITHOUT going to dashboard.
//       • If the NEW state is null or view!=="history" → go to dashboard,
//         reset _historyViewPushed so next visit pushes fresh.
// ---------------------------------------------------------------------------

// true once the history-view pushState has been issued this session.
let _historyViewPushed = false;

// true once the global heatmap click listener has been attached (prevent duplicates).
let _heatmapClickListenerAttached = false;

// ---------------------------------------------------------------------------
// Phase 2 Stream D — TASK-2.5.1: Open history tab (lazy load)
// ---------------------------------------------------------------------------

async function openHistoryTab() {
  // TASK-2.5.6 / CRITICAL-4 fix: push once per session, not once per page-load.
  //
  // DO NOT guard on history.state.view — history.state persists across reloads.
  // If the user reloads while on the history view, history.state.view is already
  // "history", the old guard would skip the push, and browser back would exit the
  // PWA instead of returning to dashboard. Instead we track _historyViewPushed in
  // module-level state which is always fresh after a reload.
  if (!_historyViewPushed) {
    _historyViewPushed = true;
    window.history.pushState({ view: "history" }, "", "");
  }
  showView("history");

  if (!_historyLoaded) {
    _historyLoaded = true;
    _historyNextCursor = undefined;
    _historyHasMore = false;
    await loadHistoryFirstPage();
  }
}

// ---------------------------------------------------------------------------
// Phase 2 Stream D — TASK-2.5.2: History cursor pagination
// ---------------------------------------------------------------------------

// Load the first page of history.
// First load: GET /api/v1/history?limit=50 (NO cursor — gets summary for heatmap).
// CURSOR SOURCE: always from server's `next_cursor` field; never derived from timestamp.
async function loadHistoryFirstPage() {
  const list = document.getElementById("history-list");
  if (list) list.innerHTML = "";
  const statsEl = document.getElementById("history-stats");
  if (statsEl) statsEl.innerHTML = "";

  // Clear heatmap while loading
  const heatmapEl = document.getElementById("heatmap-container");
  if (heatmapEl) heatmapEl.innerHTML = "";

  _historyFetchInFlight = true;
  setHistorySpinner(true);

  try {
    const resp = await apiFetch("/api/v1/history?limit=50");
    if (!resp.ok) {
      const err = await resp.json().catch(() => ({}));
      showHistoryError("Couldn't load history (" + resp.status + ").");
      return;
    }
    const data = await resp.json();

    // Render summary stats (streak + today count) if present.
    // summary is only included on the no-since/no-until call (contract §6.5).
    if (data.summary) {
      renderHistoryStats(data.summary);
    }

    // Store pagination state.
    // CURSOR DISCIPLINE: use server's next_cursor, never entries[last].timestamp.
    _historyHasMore = data.has_more === true;
    _historyNextCursor = data.next_cursor || null;

    renderHistoryEntries(data.entries || [], false);

    if ((data.entries || []).length === 0) {
      showHistoryEmpty();
    }

    // Kick off heatmap data fetch and render asynchronously.
    // Heatmap uses a separate paginated fetch; it does NOT share the list cursor.
    // We pass the first-page entries and summary for the initial data, then
    // continue fetching if has_more.
    fetchAndRenderHeatmap(data.entries || [], data.has_more === true, data.next_cursor || null);

  } catch (e) {
    if (!(e?.message || "").startsWith("Unauthorized")) {
      showHistoryError("Couldn't load history — server unreachable.");
    }
  } finally {
    _historyFetchInFlight = false;
    setHistorySpinner(false);
  }
}

// Append the next page of history entries.
// Only called when has_more === true and not already fetching.
async function loadHistoryNextPage() {
  if (_historyFetchInFlight || !_historyHasMore || !_historyNextCursor) return;

  _historyFetchInFlight = true;
  setHistorySpinner(true);
  clearHistoryRetry();

  try {
    // CURSOR DISCIPLINE: use server's next_cursor field, never a derived timestamp.
    const url = "/api/v1/history?limit=50&cursor=" + encodeURIComponent(_historyNextCursor);
    const resp = await apiFetch(url);
    if (!resp.ok) {
      showHistoryRetry("Couldn't load more — tap to retry.", loadHistoryNextPage);
      return;
    }
    const data = await resp.json();

    _historyHasMore = data.has_more === true;
    _historyNextCursor = data.next_cursor || null;

    renderHistoryEntries(data.entries || [], true);

    if ((data.entries || []).length === 0 && !_historyHasMore) {
      // Reached the end naturally
    }
  } catch (e) {
    if (!(e?.message || "").startsWith("Unauthorized")) {
      showHistoryRetry("Couldn't load more — tap to retry.", loadHistoryNextPage);
    }
  } finally {
    _historyFetchInFlight = false;
    setHistorySpinner(false);
  }
}

// Render a list of history entries into the DOM.
// append=true → append to existing list; append=false → replace.
function renderHistoryEntries(entries, append) {
  const list = document.getElementById("history-list");
  if (!list) return;

  if (!append) list.innerHTML = "";

  for (const entry of entries) {
    const item = document.createElement("div");
    item.className = "history-entry";
    item.setAttribute("role", "button");
    item.tabIndex = 0;
    item.dataset.historyId = entry.id;

    const headerRow = document.createElement("div");
    headerRow.className = "history-entry-header";

    const moduleEl = document.createElement("span");
    moduleEl.className = "history-entry-module";
    moduleEl.textContent = entry.module_key;

    const timeEl = document.createElement("span");
    timeEl.className = "history-entry-time";
    timeEl.textContent = relativeTime(entry.timestamp);

    headerRow.appendChild(moduleEl);
    headerRow.appendChild(timeEl);
    item.appendChild(headerRow);

    if (entry.first_field) {
      const firstFieldEl = document.createElement("div");
      firstFieldEl.className = "history-entry-first-field";
      firstFieldEl.textContent = entry.first_field;
      item.appendChild(firstFieldEl);
    }

    // TASK-2.5.3: tap entry → fetch and show capture content.
    const tapHandler = () => openCapturePanel(entry.id);
    item.addEventListener("click", tapHandler);
    item.addEventListener("keydown", e => {
      if (e.key === "Enter" || e.key === " ") { e.preventDefault(); tapHandler(); }
    });

    list.appendChild(item);
  }
}

function showHistoryEmpty() {
  const list = document.getElementById("history-list");
  if (!list) return;
  list.innerHTML = "";
  const p = document.createElement("p");
  p.className = "history-empty";
  p.textContent = "No captures yet. Tap a module to start pouring.";
  list.appendChild(p);
}

function showHistoryError(msg) {
  const list = document.getElementById("history-list");
  if (!list) return;
  list.innerHTML = "";
  const p = document.createElement("p");
  p.className = "history-empty";
  p.textContent = msg;
  list.appendChild(p);
}

function setHistorySpinner(visible) {
  const spinner = document.getElementById("history-spinner");
  if (spinner) spinner.hidden = !visible;
}

function clearHistoryRetry() {
  const existing = document.getElementById("history-retry-container");
  if (existing) existing.remove();
}

function showHistoryRetry(msg, handler) {
  clearHistoryRetry();
  const container = document.createElement("div");
  container.id = "history-retry-container";
  container.className = "history-retry";

  const btn = document.createElement("button");
  btn.type = "button";
  btn.className = "history-retry-btn";
  btn.textContent = msg;
  btn.addEventListener("click", () => {
    container.remove();
    handler();
  });
  container.appendChild(btn);

  const spinner = document.getElementById("history-spinner");
  if (spinner && spinner.parentNode) {
    spinner.parentNode.insertBefore(container, spinner);
  } else {
    const historySection = document.getElementById("history");
    if (historySection) historySection.appendChild(container);
  }
}

// Render history stats strip: streak + today count from summary.
function renderHistoryStats(summary) {
  const statsEl = document.getElementById("history-stats");
  if (!statsEl) return;
  statsEl.innerHTML = "";

  function makeStat(value, label) {
    const stat = document.createElement("div");
    stat.className = "history-stat";
    const val = document.createElement("span");
    val.className = "history-stat-value";
    val.textContent = String(value);
    const lbl = document.createElement("span");
    lbl.className = "history-stat-label";
    lbl.textContent = label;
    stat.appendChild(val);
    stat.appendChild(lbl);
    return stat;
  }

  if (summary.streak_days !== undefined) {
    statsEl.appendChild(makeStat(summary.streak_days, "day streak"));
  }
  if (summary.today_count !== undefined) {
    statsEl.appendChild(makeStat(summary.today_count, "today"));
  }
  if (summary.week_count !== undefined) {
    statsEl.appendChild(makeStat(summary.week_count, "this week"));
  }
}

// ---------------------------------------------------------------------------
// Phase 2 Stream D — TASK-2.5.3: Capture read-back panel
// ---------------------------------------------------------------------------

async function openCapturePanel(historyId) {
  const panel = document.getElementById("capture-panel");
  const pathEl = document.getElementById("capture-panel-path");
  const transportEl = document.getElementById("capture-panel-transport");
  const contentEl = document.getElementById("capture-panel-content");
  if (!panel || !pathEl || !transportEl || !contentEl) return;

  // Reset panel state before showing
  pathEl.textContent = "";
  transportEl.textContent = "";
  // Use textContent — NEVER innerHTML. See security convention at top of file.
  // Content is raw vault file text (may contain HTML tags); textContent is safe.
  contentEl.textContent = "Loading…";
  panel.hidden = false;

  // CRITICAL-3 fix: push Level 2 history state so browser back closes the panel
  // before leaving the history view. Back-button state machine — see comment above
  // _historyViewPushed declaration.
  window.history.pushState({ view: "history", panel: historyId }, "", "");

  try {
    const resp = await apiFetch("/api/v1/captures/" + encodeURIComponent(historyId));

    if (resp.status === 404) {
      contentEl.textContent = "This capture's file no longer exists in the vault.";
      return;
    }
    if (resp.status === 502) {
      contentEl.textContent = "Couldn't read — vault unreachable.";
      return;
    }
    if (!resp.ok) {
      contentEl.textContent = "Couldn't load capture (" + resp.status + ").";
      return;
    }

    const data = await resp.json();

    pathEl.textContent = data.vault_path || "";
    transportEl.textContent = data.transport_mode || "";

    // SECURITY: content rendered via textContent only. Never innerHTML.
    // The user trusts their own vault, but a markdown file with <script> would
    // execute via innerHTML. textContent is immune.
    // Per contract §14: we do NOT log content — it appears only in this panel.
    contentEl.textContent = data.content || "";

  } catch (e) {
    if (!(e?.message || "").startsWith("Unauthorized")) {
      contentEl.textContent = "Couldn't load — server unreachable.";
    }
  }
}

// closeCapturePanel(fromPopstate)
//   fromPopstate=true  → called by the popstate handler; panel is already being
//                         popped by the browser — do NOT call history.back() or
//                         we'd double-pop and lose the history-view Level 1 state.
//   fromPopstate=false → called by the close button; Level 2 entry is still on
//                         the stack — call history.back() so popstate fires and
//                         the URL returns to the Level 1 (history-view) state.
function closeCapturePanel(fromPopstate) {
  const panel = document.getElementById("capture-panel");
  if (panel) panel.hidden = true;
  const contentEl = document.getElementById("capture-panel-content");
  if (contentEl) contentEl.textContent = "";
  if (!fromPopstate) {
    // Only back() if we pushed a Level 2 state. Guard: current state must have panel.
    if (window.history.state && window.history.state.panel != null) {
      window.history.back();
    }
  }
}

// ---------------------------------------------------------------------------
// Phase 2 Stream D — TASK-2.5.4: Heatmap renderer
//
// Data source: client-side rollup from /api/v1/history (path (a) — see note at top).
// Heatmap window: HEATMAP_DAYS = 90 (constant at top of file, NOT user-configurable).
//
// Loop termination:
//   - Continues fetching pages while has_more===true AND the oldest entry in the
//     latest page is within the HEATMAP_DAYS window.
//   - Stops when has_more===false or all remaining entries are older than the window.
//   - This prevents unbounded fetches for users with years of history.
// ---------------------------------------------------------------------------

async function fetchAndRenderHeatmap(firstPageEntries, firstPageHasMore, firstPageNextCursor) {
  // Build the window: today (local date) back HEATMAP_DAYS.
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const windowStart = new Date(today);
  windowStart.setDate(windowStart.getDate() - (HEATMAP_DAYS - 1));

  // Accumulate all entries within the window.
  // Keyed by "YYYY-MM-DD" (local date) → count.
  const counts = {};

  function addEntry(entry) {
    // Use local date for heatmap (not UTC) — a capture at 11pm local shouldn't
    // appear on the wrong day just because UTC rolled over.
    const d = new Date(entry.timestamp);
    if (isNaN(d.getTime())) return;
    if (d < windowStart) return; // older than window — skip
    const key = d.getFullYear() + "-" +
      String(d.getMonth() + 1).padStart(2, "0") + "-" +
      String(d.getDate()).padStart(2, "0");
    counts[key] = (counts[key] || 0) + 1;
  }

  // Process first page (already fetched by loadHistoryFirstPage)
  for (const e of firstPageEntries) addEntry(e);

  // Continue fetching if has_more and we haven't gone past the window yet.
  // CURSOR DISCIPLINE: always use server's next_cursor, never derive from timestamp.
  let hasMore = firstPageHasMore;
  let nextCursor = firstPageNextCursor;
  // CRITICAL-1: iteration counter. Abort at HEATMAP_MAX_PAGES regardless of has_more.
  let iterations = 0;

  while (hasMore && nextCursor) {
    // CRITICAL-1 (a): hard iteration cap.
    if (iterations >= HEATMAP_MAX_PAGES) {
      console.warn("fetchAndRenderHeatmap: hit HEATMAP_MAX_PAGES (" + HEATMAP_MAX_PAGES +
        ") — rendering heatmap with partial data.");
      break;
    }
    iterations++;

    // Check if the oldest entry on the last page was already before our window.
    // If so, we've covered the entire window and can stop.
    const oldestSoFar = firstPageEntries.length > 0
      ? new Date(firstPageEntries[firstPageEntries.length - 1].timestamp)
      : null;
    if (oldestSoFar && oldestSoFar < windowStart) break;

    try {
      // CURSOR DISCIPLINE: use next_cursor, not timestamp-derived cursor.
      const url = "/api/v1/history?limit=" + HEATMAP_FETCH_LIMIT +
        "&cursor=" + encodeURIComponent(nextCursor);
      const resp = await apiFetch(url);
      if (!resp.ok) break; // fetch error — render with partial data

      const data = await resp.json();
      const entries = data.entries || [];

      // CRITICAL-1 (b): empty-page guard. An empty page with has_more=true is a
      // malformed server response. Don't trust it — render what we have so far.
      if (entries.length === 0) {
        if (data.has_more === true) {
          console.warn("fetchAndRenderHeatmap: empty entries with has_more=true — " +
            "malformed response; aborting loop and rendering partial data.");
        }
        break;
      }

      for (const e of entries) addEntry(e);

      hasMore = data.has_more === true;
      // CURSOR DISCIPLINE: use server's next_cursor field.
      nextCursor = data.next_cursor || null;

      // Loop termination: if the oldest entry in this page is before the window,
      // all remaining entries will also be — no need to fetch further pages.
      const oldestEntry = entries[entries.length - 1];
      const oldestDate = new Date(oldestEntry.timestamp);
      if (!isNaN(oldestDate.getTime()) && oldestDate < windowStart) break;

    } catch (_e) {
      break; // network error — render with partial data
    }
  }

  renderHeatmap(counts, today, windowStart);
}

// Render the heatmap grid into #heatmap-container.
// counts: { "YYYY-MM-DD": number } — only dates with captures need to be present.
// today: Date (local midnight).
// windowStart: Date (local midnight, HEATMAP_DAYS ago).
function renderHeatmap(counts, today, windowStart) {
  const container = document.getElementById("heatmap-container");
  if (!container) return;
  container.innerHTML = "";

  // CRITICAL-2 fix: reuse a single popover element across all renderHeatmap calls.
  // Multiple Dashboard ⇄ History tab round-trips would otherwise accumulate one
  // orphaned popover + one global click listener per visit.
  let popover = document.getElementById("heatmap-popover");
  if (!popover) {
    popover = document.createElement("div");
    popover.id = "heatmap-popover";
    popover.className = "heatmap-popover";
    popover.hidden = true;
    popover.setAttribute("role", "tooltip");
    document.body.appendChild(popover);
  } else {
    // Reuse: just ensure it is hidden until a cell is tapped.
    popover.hidden = true;
  }

  // Day-of-week labels (Mon=1, Wed=3, Fri=5 have text; others empty)
  const DOW_LABELS = ["", "M", "", "W", "", "F", ""];

  const labelCol = document.createElement("div");
  labelCol.className = "heatmap-weekday-labels";
  for (let dow = 0; dow < 7; dow++) {
    const lbl = document.createElement("div");
    lbl.className = "heatmap-weekday-label";
    lbl.textContent = DOW_LABELS[dow];
    lbl.setAttribute("aria-hidden", "true");
    labelCol.appendChild(lbl);
  }

  // Build grid cells from windowStart to today (inclusive).
  // Columns-of-weeks: column 0 is the first week in the window, column N the last.
  // Cells are placed in a CSS grid with grid-auto-flow: column (top-to-bottom, then next column).
  // The first cell's row offset = windowStart's day-of-week (0=Sun).
  const grid = document.createElement("div");
  grid.className = "heatmap-grid";

  // We start at windowStart and step day by day to today.
  // To ensure the grid aligns week columns properly, prepend empty cells for
  // the days before windowStart in its week (so the first day lands in the
  // correct row slot).
  const startDow = windowStart.getDay(); // 0=Sun … 6=Sat

  // Prepend spacer cells to align the first real cell in the correct row.
  for (let i = 0; i < startDow; i++) {
    const spacer = document.createElement("div");
    spacer.className = "heatmap-cell";
    spacer.style.visibility = "hidden";
    spacer.setAttribute("aria-hidden", "true");
    grid.appendChild(spacer);
  }

  // Generate a cell for each day in [windowStart, today].
  const cursor = new Date(windowStart);
  while (cursor <= today) {
    const key = cursor.getFullYear() + "-" +
      String(cursor.getMonth() + 1).padStart(2, "0") + "-" +
      String(cursor.getDate()).padStart(2, "0");
    const count = counts[key] || 0;

    // Readable date for aria-label (e.g. "April 27, 2026")
    const longDate = cursor.toLocaleDateString(undefined, {
      year: "numeric", month: "long", day: "numeric",
    });

    const cell = document.createElement("div");
    cell.className = "heatmap-cell";
    cell.dataset.count = count <= 3 ? String(count) : "3"; // CSS tiers 0/1/2-3/4+
    if (count >= 4) cell.dataset.countMany = "true";
    // TASK-2.5.4 accessibility (Open Q10): each cell carries aria-label with date + count.
    // With 90 cells this is fine for screen readers — each cell is individually labeled.
    cell.setAttribute("aria-label", longDate + ": " + count + (count === 1 ? " capture" : " captures"));

    // Tap cell → show popover with date + count
    const captureLabel = count === 0
      ? "no captures"
      : count === 1
        ? "1 capture"
        : count + " captures";
    cell.addEventListener("click", e => {
      showHeatmapPopover(popover, e.clientX, e.clientY, longDate + ": " + captureLabel);
    });

    grid.appendChild(cell);
    cursor.setDate(cursor.getDate() + 1);
  }

  // Scroll row: day-of-week labels + grid side by side, container handles horizontal scroll.
  const scrollRow = document.createElement("div");
  scrollRow.className = "heatmap-scroll-row";
  scrollRow.appendChild(labelCol);
  scrollRow.appendChild(grid);
  container.appendChild(scrollRow);

  // CRITICAL-2 fix: attach the global dismiss listener only once per session.
  // _heatmapClickListenerAttached is a module-level flag; subsequent renderHeatmap
  // calls skip this block. The handler references `popover` via document.getElementById
  // so it always operates on the current (reused) element, not a stale closure.
  if (!_heatmapClickListenerAttached) {
    _heatmapClickListenerAttached = true;
    document.addEventListener("click", e => {
      if (!e.target.classList.contains("heatmap-cell")) {
        const pop = document.getElementById("heatmap-popover");
        if (pop) pop.hidden = true;
      }
    }, { capture: false });
  }
}

// Position and show the heatmap popover near the tapped cell.
function showHeatmapPopover(popover, clientX, clientY, text) {
  popover.textContent = text;
  popover.hidden = false;
  // Position above the tap point; clamp to viewport edges.
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  const pw = popover.offsetWidth || 120;
  const ph = popover.offsetHeight || 28;
  let left = clientX - pw / 2;
  let top = clientY - ph - 8;
  if (left < 8) left = 8;
  if (left + pw > vw - 8) left = vw - pw - 8;
  if (top < 8) top = clientY + 20; // flip below if too close to top
  popover.style.left = left + "px";
  popover.style.top = top + "px";
}

// ---------------------------------------------------------------------------
// Phase 2 Stream D — TASK-2.5.6: Tab nav wiring + pushState/popstate
// ---------------------------------------------------------------------------

document.addEventListener("DOMContentLoaded", () => {
  // Tab buttons
  const tabCapture = document.getElementById("tab-capture");
  const tabHistory = document.getElementById("tab-history");

  if (tabCapture) {
    tabCapture.addEventListener("click", () => {
      // Return to dashboard (the "capture" tab shows the module list).
      // Reset pushState so back button exits the PWA from dashboard, not history.
      if (window.history.state && window.history.state.view === "history") {
        window.history.back();
      } else {
        loadDashboard();
      }
    });
  }

  if (tabHistory) {
    tabHistory.addEventListener("click", () => {
      openHistoryTab();
    });
  }

  // TASK-2.5.6 / CRITICAL-3+4 fix: two-level popstate handler.
  //
  // State machine (see full comment above _historyViewPushed declaration):
  //   - NEW state has panel field → the user navigated FORWARD into a panel
  //     (defensive; shouldn't happen in normal flow).
  //   - NEW state is { view: "history" } (no panel) → we just popped the Level 2
  //     panel entry. Close the panel without going to dashboard.
  //   - NEW state is null or view !== "history" → we just popped the Level 1
  //     history entry. Go to dashboard and reset session push flag.
  window.addEventListener("popstate", e => {
    const newState = e.state;

    if (newState && newState.view === "history" && newState.panel != null) {
      // Popped forward into a panel state (defensive). Open the panel.
      openCapturePanel(newState.panel);
      return;
    }

    if (newState && newState.view === "history" && newState.panel == null) {
      // Popped Level 2 (panel → history view). Close the panel.
      // Pass fromPopstate=true so closeCapturePanel does NOT call history.back().
      closeCapturePanel(true);
      return;
    }

    // Popped Level 1 (history view → dashboard) or landed on initial state.
    // Reset push flag so the next openHistoryTab() call pushes fresh.
    _historyViewPushed = false;
    _historyLoaded = false; // reset so next history visit re-fetches
    loadDashboard();
  });

  // Capture-panel close button
  // Pass fromPopstate=false explicitly — we're closing via button, not popstate.
  const closePanelBtn = document.getElementById("capture-panel-close");
  if (closePanelBtn) {
    closePanelBtn.addEventListener("click", () => closeCapturePanel(false));
  }

  // Scroll-triggered pagination: when user scrolls within ~200px of the bottom
  // of the history section, load the next page (debounced by _historyFetchInFlight).
  const historySection = document.getElementById("history");
  if (historySection) {
    historySection.addEventListener("scroll", () => {
      if (!_historyHasMore || _historyFetchInFlight) return;
      const scrollBottom = historySection.scrollTop + historySection.clientHeight;
      const threshold = historySection.scrollHeight - 200;
      if (scrollBottom >= threshold) {
        loadHistoryNextPage();
      }
    }, { passive: true });
  }

  // Also listen on window scroll for the history view (the section may not be
  // scroll-overflow — the page itself may scroll).
  window.addEventListener("scroll", () => {
    // Only act if history view is currently shown.
    const historyEl = document.getElementById("history");
    if (!historyEl || historyEl.hidden) return;
    if (!_historyHasMore || _historyFetchInFlight) return;

    const scrollY = window.scrollY || window.pageYOffset;
    const windowH = window.innerHeight;
    const docH = document.documentElement.scrollHeight;
    if (scrollY + windowH >= docH - 200) {
      loadHistoryNextPage();
    }
  }, { passive: true });
});
