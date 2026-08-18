// AgentStatus — display preferences, shared by the light bar (main.js) and the
// settings window (settings.js). Loaded as a plain script before either, so its
// top-level bindings are visible to both.
//
// Every preference is app-local: it lives in the webview's localStorage and never
// touches the hook-written status files. The bar is the source of truth — the
// settings window asks it for a snapshot on open (`prefs-request`) and announces
// each change (`pref-changed`), so neither window depends on localStorage being
// shared or synchronised between webviews (decision 082).

const ORIENT_KEY = "agentstatus.orientation"; // "horizontal" | "vertical"
const SORT_KEY = "agentstatus.sort"; // "stable" | "urgency" ("window" = a pre-062 stable)
const UNKNOWN_KEY = "agentstatus.showunknown"; // "true" | "false"
const PIP_KEY = "agentstatus.cursorpip"; // "true" | "false" — the Cursor menu-bar pip
const MODE_KEY = "agentstatus.mode"; // "floating" | "menubar"
const CONDENSE_KEY = "agentstatus.menubarcondense"; // "true" | "false"
const SIZE_KEY = "agentstatus.dotsize";
const PAD_KEY = "agentstatus.barpad";
const OPACITY_KEY = "agentstatus.baropacity";
const COLORS_KEY = "agentstatus.colors";
const AUDIO_KEY = "agentstatus.audio"; // "on" | "off"
const CHIME_KEY = "agentstatus.chimes"; // JSON {blocked,error,done}
const VOL_KEY = "agentstatus.volume"; // 0–100

// Everything "Reset to defaults" clears. The bar's saved position (POS_KEY) is
// deliberately not here: a reset restores the look, it does not move the bar.
const PREF_KEYS = [
  ORIENT_KEY,
  SORT_KEY,
  UNKNOWN_KEY,
  PIP_KEY,
  MODE_KEY,
  CONDENSE_KEY,
  SIZE_KEY,
  PAD_KEY,
  OPACITY_KEY,
  COLORS_KEY,
  AUDIO_KEY,
  CHIME_KEY,
  VOL_KEY,
];

// Defaults mirror the CSS, so a cleared/absent pref looks identical to the stock bar.
const DEFAULT_SIZE = 13; // px
const DEFAULT_PAD = 9; // px, wrapper padding around the lights
const DEFAULT_OPACITY = 82; // percent of pill-fill alpha (matches the CSS default 0.82)
const DEFAULT_COLORS = {
  running: "#2ecc71",
  blocked: "#f39c12",
  done: "#ecf0f1",
  idle: "#7f8c8d",
  error: "#e74c3c",
};
const DEFAULT_CHIMES = { blocked: true, error: true, done: true };
const DEFAULT_VOLUME = 60;

// Which platform the backend is on, asked once per window at startup (decision 072).
// Empty until the first answer arrives; every use treats that as "not Windows", which
// is the pre-existing behaviour.
let PLATFORM = "";

function currentOrientation() {
  return localStorage.getItem(ORIENT_KEY) === "vertical" ? "vertical" : "horizontal";
}

// Light ordering. "stable" is arrival order — a session takes the next free slot the
// first time it is seen and holds it for as long as it exists; "urgency" surfaces the
// attention states first. A stored "window" is a pre-062 preference and reads as stable.
function currentSort() {
  return localStorage.getItem(SORT_KEY) === "urgency" ? "urgency" : "stable";
}

// Whether "unknown" lights (sessions we get no signal from — decision 042) appear on
// the bar at all. Defaults to showing them: a session you can't read is still a session
// you may want to know exists (decision 044).
function showUnknown() {
  return localStorage.getItem(UNKNOWN_KEY) !== "false";
}

// Whether the aggregate Cursor menu-bar pip (decision 038) is drawn. Defaults on — it
// is the only attention signal Cursor exposes. Turning it off also stops the periodic
// Accessibility read behind it, which is the one thing the bar does that can cancel
// Cursor's own menu while it is open (decisions 081/082).
function showCursorPip() {
  return localStorage.getItem(PIP_KEY) !== "false";
}

function currentMode() {
  return localStorage.getItem(MODE_KEY) === "menubar" ? "menubar" : "floating";
}

function currentCondense() {
  // A Windows notification-area icon is square (16x16 logical, scaled by DPI). A row of
  // dots stretched into that is an unreadable smear, so Windows always shows the single
  // summary dot — the Dots/Single choice is hidden there rather than offered and ignored.
  if (PLATFORM === "windows") return true;
  return localStorage.getItem(CONDENSE_KEY) === "true";
}

// Menu-bar mode always renders horizontally to match the menu bar (a vertical popover
// hanging off the bar looks wrong); the user's saved orientation is restored when they
// switch back to floating.
function effectiveOrientation() {
  return currentMode() === "menubar" ? "horizontal" : currentOrientation();
}

function currentSize() {
  const n = parseInt(localStorage.getItem(SIZE_KEY), 10);
  return Number.isFinite(n) ? Math.min(24, Math.max(8, n)) : DEFAULT_SIZE;
}

function currentPad() {
  const n = parseInt(localStorage.getItem(PAD_KEY), 10);
  return Number.isFinite(n) ? Math.min(20, Math.max(2, n)) : DEFAULT_PAD;
}

// Pill-fill opacity, stored as a whole percent (0–100) to match the int slider.
function currentOpacity() {
  const n = parseInt(localStorage.getItem(OPACITY_KEY), 10);
  return Number.isFinite(n) ? Math.min(100, Math.max(0, n)) : DEFAULT_OPACITY;
}

function currentColors() {
  let saved = {};
  try {
    saved = JSON.parse(localStorage.getItem(COLORS_KEY)) || {};
  } catch (_) {
    /* corrupt value → fall back to defaults */
  }
  return { ...DEFAULT_COLORS, ...saved };
}

function audioEnabled() {
  return localStorage.getItem(AUDIO_KEY) === "on";
}

function currentChimes() {
  let saved = {};
  try {
    saved = JSON.parse(localStorage.getItem(CHIME_KEY)) || {};
  } catch (_) {
    /* corrupt value → defaults */
  }
  return { ...DEFAULT_CHIMES, ...saved };
}

function currentVolume() {
  const n = parseInt(localStorage.getItem(VOL_KEY), 10);
  return Number.isFinite(n) ? Math.min(100, Math.max(0, n)) : DEFAULT_VOLUME;
}

// Write one preference locally. `null` removes it (back to the default).
function writePref(key, value) {
  try {
    if (value === null) localStorage.removeItem(key);
    else localStorage.setItem(key, value);
  } catch (_) {
    /* fail-silent: a pref that can't be stored still applies for this run */
  }
}

function emitPref(name, payload) {
  try {
    window.__TAURI__.event.emit(name, payload);
  } catch (_) {
    /* no Tauri (a test harness, or the API not ready) — the local write still stands */
  }
}

// Every pref as it stands right now, `null` for the ones never set. This is what the
// bar hands the settings window on open, so the window renders the live values rather
// than whatever its own webview last happened to store.
function prefsSnapshot() {
  const out = {};
  for (const key of PREF_KEYS) out[key] = localStorage.getItem(key);
  return out;
}

function applySnapshot(snap) {
  if (!snap) return;
  for (const key of PREF_KEYS) writePref(key, snap[key] ?? null);
}
