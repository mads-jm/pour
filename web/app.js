// Pour PWA — vanilla JS, no framework, no build step.
// ~260 lines. Modules via plain functions.
//
// SECURITY CONVENTION: All user-derived strings injected via innerHTML must pass
// through escapeHtml(). Prefer textContent / createElement for new code.

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

let _token = null;
let _config = null;          // full /api/v1/config response
let _currentModule = null;   // module key string
let _currentModuleData = null; // module object from config
let _optionsCache = {};      // { "module:field": [...] }

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
  return resp;
}

// ---------------------------------------------------------------------------
// View switching
// ---------------------------------------------------------------------------

function showView(id) {
  for (const v of ["token-gate", "dashboard", "form", "summary"]) {
    const el = document.getElementById(v);
    if (el) el.hidden = (v !== id);
  }
}

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
    if (resp.status === 401) {
      pill.textContent = "Invalid token";
      showToast("Invalid token — re-scan the QR code.");
      _token = null;
      localStorage.removeItem("pour_token");
      showView("token-gate");
      return false;
    }
    if (!resp.ok) {
      pill.textContent = `Server error (${resp.status})`;
      return false;
    }
    const data = await resp.json();
    const mode = data.transport_mode || "?";
    const emoji = mode === "API" ? "🟢" : mode === "FileSystem" ? "🟡" : "🔴";
    pill.textContent = emoji + " " + mode;
    return true;
  } catch (_e) {
    pill.textContent = "🔴 Offline";
    showToast("Server unreachable — is pour serve running?");
    return false;
  }
}

// ---------------------------------------------------------------------------
// Module list
// ---------------------------------------------------------------------------

async function loadDashboard() {
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
  for (const mod of modules) {
    const btn = document.createElement("button");
    btn.className = "module-tile";
    btn.innerHTML =
      '<span class="tile-icon">' + escapeHtml(mod.icon || "📝") + '</span>' +
      '<span>' + escapeHtml(mod.display_name || mod.key) + '</span>';
    btn.addEventListener("click", () => openForm(mod.key));
    grid.appendChild(btn);
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

  // Pre-fetch dynamic_select options
  _optionsCache = {};
  const dynFields = (mod.fields || []).filter(f => f.field_type === "dynamic_select");
  await Promise.all(dynFields.map(f => fetchOptions(moduleKey, f.name)));

  // Render
  const titleEl = document.getElementById("form-title");
  titleEl.innerHTML = '<span>' + escapeHtml(mod.icon || "") + '</span> ' + escapeHtml(mod.display_name || mod.key);

  renderForm(mod, {});
  showView("form");
}

function renderForm(mod, currentValues) {
  const form = document.getElementById("capture-form");
  form.innerHTML = "";

  const visible = computeVisible(mod.fields || [], currentValues);

  for (const field of (mod.fields || [])) {
    if (!visible.has(field.name)) continue;

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

    const input = buildFieldInput(field, currentValues[field.name] || "");
    group.appendChild(input);

    const errEl = document.createElement("div");
    errEl.className = "field-error";
    errEl.id = "err-" + field.name;
    errEl.hidden = true;
    group.appendChild(errEl);

    form.appendChild(group);
  }

  // Submit button
  const submitBtn = document.createElement("button");
  submitBtn.type = "submit";
  submitBtn.className = "btn-primary";
  submitBtn.textContent = "Pour";
  form.appendChild(submitBtn);

  // Back button
  const backBtn = document.createElement("button");
  backBtn.type = "button";
  backBtn.className = "btn-secondary";
  backBtn.style.marginTop = "10px";
  backBtn.style.width = "100%";
  backBtn.textContent = "Back";
  backBtn.addEventListener("click", loadDashboard);
  form.appendChild(backBtn);

  // Reactivity: re-render on any change to handle show_when
  form.addEventListener("change", () => {
    const vals = collectValues();
    recomputeVisibility(mod.fields || [], vals);
  });

  form.addEventListener("submit", async e => {
    e.preventDefault();
    await handleSubmit();
  });
}

function buildFieldInput(field, currentValue) {
  switch (field.field_type) {
    case "textarea": {
      const ta = document.createElement("textarea");
      ta.id = "field-" + field.name;
      ta.name = field.name;
      ta.rows = 4;
      ta.value = currentValue;
      if (field.required) ta.required = true;
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
      if (field.required) inp.required = true;
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
      if (field.required) sel.required = true;
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
        if (field.required) inp.required = true;
        const dl = document.createElement("datalist");
        dl.id = "dl-" + field.name;
        for (const opt of opts) {
          const o = document.createElement("option");
          o.value = opt;
          dl.appendChild(o);
        }
        wrapper.appendChild(inp);
        wrapper.appendChild(dl);
        return wrapper;
      } else {
        const sel = document.createElement("select");
        sel.id = "field-" + field.name;
        sel.name = field.name;
        const blank = document.createElement("option");
        blank.value = "";
        blank.textContent = "— select —";
        sel.appendChild(blank);
        for (const opt of opts) {
          const o = document.createElement("option");
          o.value = opt;
          o.textContent = opt;
          if (opt === currentValue) o.selected = true;
          sel.appendChild(o);
        }
        if (field.required) sel.required = true;
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
      if (field.required) inp.required = true;
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
    rmBtn.textContent = "x";
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

function recomputeVisibility(fields, values) {
  const visible = computeVisible(fields, values);
  for (const f of fields) {
    const group = document.querySelector("[data-field='" + f.name + "']");
    if (group) group.hidden = !visible.has(f.name);
  }
}

// ---------------------------------------------------------------------------
// Collect form values
// ---------------------------------------------------------------------------

function collectValues() {
  const vals = {};
  const form = document.getElementById("capture-form");
  const fields = _currentModuleData ? (_currentModuleData.fields || []) : [];
  const visible = computeVisible(fields, (() => {
    // Quick pass to get current values for visibility computation
    const v = {};
    for (const f of fields) {
      const el = document.getElementById("field-" + f.name);
      if (el) v[f.name] = el.value || "";
    }
    return v;
  })());

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

  const idempotencyKey = uuidv4();
  const capturedAt = new Date().toISOString();

  const body = {
    field_values: fieldValues,
    composite_data: Object.keys(compositeData).length > 0 ? compositeData : undefined,
    captured_at: capturedAt,
    client_id: "phone-pwa",
  };

  const submitBtn = document.querySelector("#capture-form button[type='submit']");
  if (submitBtn) { submitBtn.disabled = true; submitBtn.textContent = "Pouring..."; }

  try {
    const resp = await apiFetch("/api/v1/submit/" + _currentModule, {
      method: "POST",
      body: JSON.stringify(body),
      headers: { "Idempotency-Key": idempotencyKey },
    });

    if (resp.status === 201) {
      const data = await resp.json();
      showSummary(data);
      return;
    }

    // Error handling
    const err = await resp.json().catch(() => ({}));
    if (resp.status === 400 && err.error && err.error.details && err.error.details.fields) {
      for (const fe of err.error.details.fields) {
        showFieldError(fe.field, fe.code || "invalid");
      }
    } else {
      const msg = (err.error && err.error.message) || ("Submit failed: " + resp.status);
      showToast(msg);
    }
  } catch (_e) {
    showToast("Submit failed — server unreachable.");
  } finally {
    if (submitBtn) { submitBtn.disabled = false; submitBtn.textContent = "Pour"; }
  }
}

function showSummary(data) {
  document.getElementById("summary-message").textContent = "Entry saved.";
  document.getElementById("summary-path").textContent = data.vault_path || "";
  document.getElementById("summary-transport").textContent = data.transport_mode || "";
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
// Init
// ---------------------------------------------------------------------------

document.addEventListener("DOMContentLoaded", async () => {
  const token = getToken();
  if (!token) {
    showView("token-gate");
    return;
  }

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
    if (_currentModule) openForm(_currentModule);
    else loadDashboard();
  });
  document.getElementById("btn-dashboard").addEventListener("click", loadDashboard);
});
