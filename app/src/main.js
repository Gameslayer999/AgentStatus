// AgentStatus — frontend. Polls the Rust `list_sessions` command and renders
// one light per session, sizing the window to hug the bar so the transparent
// area never blocks clicks to apps behind it. Dots are reconciled in place (not
// rebuilt each poll) so hovering a light never dismisses its tooltip.

const { invoke } = window.__TAURI__.core;
// `currentMonitor` / `availableMonitors` are module-level functions in Tauri v2
// (not window methods).
const { getCurrentWindow, currentMonitor, availableMonitors } = window.__TAURI__.window;
// LogicalSize / PhysicalPosition live in the dpi namespace in Tauri v2; fall back
// to the window namespace just in case.
const LogicalSize =
  (window.__TAURI__.dpi && window.__TAURI__.dpi.LogicalSize) ||
  (window.__TAURI__.window && window.__TAURI__.window.LogicalSize);
const PhysicalPosition =
  (window.__TAURI__.dpi && window.__TAURI__.dpi.PhysicalPosition) ||
  (window.__TAURI__.window && window.__TAURI__.window.PhysicalPosition);

const POLL_MS = 1000;
const AUTO_RESIZE = true;
const MIN_W = 32; // never shrink below a grabbable pill
const MIN_H = 30;

const appWindow = getCurrentWindow();
const dots = new Map(); // session id -> dot element
const closers = new Map(); // session id -> its close button in the closers row
let emptyEl = null;

// Cursor menu-bar mirror (decision 038). Cursor's live running/idle status is
// renderer-memory-only, but its macOS menu-bar item exposes an aggregate count of
// composers awaiting the user — the one attention bit its hooks don't provide. We
// poll it (throttled) and render it as ONE extra pip, distinct from the per-session
// Cursor lights the hooks drive, so it never masquerades as a specific session.
let cursorPipEl = null;
let cursorAttention = 0; // last count read off Cursor's menu-bar item
let tickCount = 0;
// Poll Cursor's menu-bar item rarely: the AX read can dismiss the item's own popover if it
// lands while the user has it open, and the notification count changes seldom — so a gentle
// cadence keeps the pip fresh enough without interfering with Cursor's menu bar (decision 038).
const CURSOR_POLL_EVERY = 20; // every 20th tick (~20s)
// After a pip click opens a composer (decision 045), Cursor rewrites its menu-bar item
// asynchronously — wait this long before re-reading the true count.
const CURSOR_RECHECK_MS = 1500;

// Where the bar sits on screen — the one preference the bar keeps to itself. Every
// other one lives in prefs.js, shared with the settings window (decision 082).
const POS_KEY = "agentstatus.pos"; // last #lights screen anchor {x,y,scale} (physical px), restored on launch

// Most-urgent first (UI Principle #2). Uses the rendered displayState so a finished
// "done" turn clusters correctly, not the raw idle state.
const URGENCY_RANK = { error: 0, blocked: 1, done: 2, running: 3, unknown: 4, idle: 5 };

// Arrival order (decision 062). Until this, the strip was re-sorted every poll by
// `cwd` then session id — so a new session in a folder you already had open landed at
// a position decided by its random uuid and shoved every later light along, and a
// session that `cd`'d changed groups. Sessions come and go constantly (a background
// agent per /stop, pre-warmed spares), so lights the user was aiming at kept moving
// under the pointer. A light is a click target first: it must stay where it was put.
const arrivalSeq = new Map(); // session id -> the slot it claimed
let nextArrival = 0;

// Give every session we haven't seen before the next slot. New sessions are numbered
// in the order the backend delivered them (label, then id — deterministic), so the
// first poll after a launch lays the bar out the same way every time; after that each
// arrival simply appends. Sessions that are gone release their slot, which only ever
// closes a gap — the lights that remain keep their relative order.
function noteArrivals(sessions) {
  for (const s of sessions) if (!arrivalSeq.has(s.id)) arrivalSeq.set(s.id, nextArrival++);
  const live = new Set(sessions.map((s) => s.id));
  for (const id of arrivalSeq.keys()) if (!live.has(id)) arrivalSeq.delete(id);
}

function byArrival(a, b) {
  return (arrivalSeq.get(a.id) ?? 0) - (arrivalSeq.get(b.id) ?? 0);
}

// Return a new, ordered array for the current sort mode. In urgency mode a light
// only moves when its own state changes; within a state it stays in arrival order.
function sortSessions(sessions) {
  noteArrivals(sessions);
  const arr = sessions.slice();
  if (currentSort() === "urgency") {
    arr.sort((a, b) => {
      const r = (URGENCY_RANK[displayState(a)] ?? 9) - (URGENCY_RANK[displayState(b)] ?? 9);
      return r || byArrival(a, b);
    });
  } else {
    arr.sort(byArrival);
  }
  return arr;
}

// What the bar and the tray actually draw from the latest poll. Kept separate from
// `latestSessions` (which stays complete) so toggling the pref re-renders instantly
// from memory instead of waiting for the next poll.
function visibleSessions() {
  const shown = dismissedAt.size
    ? latestSessions.filter((s) => !dismissedAt.has(s.id))
    : latestSessions;
  if (showUnknown()) return shown;
  return shown.filter((s) => displayState(s) !== "unknown");
}

// ── Manual prune (decision 080) ─────────────────────────────────────────────
// Every automatic prune waits on evidence — a closed window, a dead pid, an archived
// composer, the idle backstop — so a light for a session the user knows is finished
// can outlive its usefulness by minutes. The close buttons beside the lights (shown
// with the settings panel) delete that session's status file now. A tombstone hides
// the light locally in the meantime, so a poll already in flight can't paint it back
// for a tick.
const DISMISS_GRACE_MS = 5000;
const dismissedAt = new Map(); // session id -> when its X was clicked

// Lift a tombstone as soon as the poll agrees the session is gone — and, if it never
// does (the file could not be deleted), after DISMISS_GRACE_MS, so a light is never
// hidden by a dismissal that didn't take (UI Principle #4).
function reapDismissed(sessions) {
  if (dismissedAt.size === 0) return;
  const live = new Set(sessions.map((s) => s.id));
  const now = Date.now();
  for (const [id, t] of dismissedAt) {
    if (!live.has(id) || now - t > DISMISS_GRACE_MS) dismissedAt.delete(id);
  }
}

async function dismissSession(id) {
  dismissedAt.set(id, Date.now());
  render(visibleSessions()); // the light goes on the click, not on the next poll
  if (currentMode() === "menubar") {
    lastTraySig = null;
    pushTrayImage();
  }
  try {
    await invoke("dismiss_session", { id });
  } catch (_) {
    /* fail-silent: the grace timer un-hides the light if the delete never happened */
  }
}

// Repaint the menu-bar/tray icon from the latest sessions. No-op while floating.
function repaintTray() {
  if (currentMode() !== "menubar") return;
  lastTraySig = null; // the image changed for a reason the signature can't see
  pushTrayImage();
}

// Lay the lights out as a row or a column. The window auto-resizes to hug whichever
// shape results.
function applyOrientation(orient) {
  const bar = document.getElementById("bar");
  bar.classList.toggle("vertical", orient === "vertical");
  resizeToContent();
}

// ── Presentation mode: floating vs. macOS menu bar (decision 024) ───────────
// Floating = the always-visible NSPanel (current behavior). Menu-bar = a tray
// item showing the lights as a generated image; clicking it reveals the bar as a
// popover under the menu bar. The mode itself is a shared pref (prefs.js); what it
// *does* lives here.

// Apply a mode for real: flip the tray + panel visibility in the backend, then do
// the mode-specific frontend work — floating re-anchors the panel to the saved
// position; menu-bar paints the tray from the latest sessions.
async function applyMode(mode) {
  applyOrientation(effectiveOrientation()); // menu-bar forces horizontal; floating restores the saved pref
  try {
    // The backend answers false when menu-bar mode has no tray to represent the panel.
    // Honouring that matters: without a tray there is no tray icon, no taskbar button
    // (skipTaskbar) and no Dock icon, so the first light click — which hides the popover —
    // would leave the app on screen nowhere and reachable only by killing the process.
    // Fall back to floating instead, and persist that so the next launch is not trapped too.
    const ok = await invoke("set_mode", { mode });
    if (mode === "menubar" && ok === false) {
      writePref(MODE_KEY, "floating");
      // The settings window is showing "Menu bar" for a mode that didn't take; hand it
      // the values that actually stand (decision 082).
      emitPref("prefs-snapshot", prefsSnapshot());
      applyOrientation(effectiveOrientation());
      await restoreAnchor();
      return;
    }
  } catch (_) {
    /* backend not ready yet; the next toggle / load will retry */
  }
  if (mode === "menubar") {
    lastTraySig = null; // force a repaint
    await pushTrayImage();
  } else {
    await restoreAnchor();
  }
}

// ── Menu-bar tray image ─────────────────────────────────────────────────────
// The webview draws the dots to an offscreen canvas and hands the pixels to Rust,
// which sets them as the tray icon — reusing displayState()/currentColors() so the
// menu bar honors the exact same per-state colors as the bar. We only push when the
// image actually changed (signature), matching the DOM reconciler's "update on
// change only" discipline.
let trayCanvas = null;
let lastTraySig = null;
let latestSessions = []; // most recent poll, so a pref change can repaint the tray

// Condense picks the single most-urgent state to show (UI Principle #2 — surface
// what needs the user first).
const TRAY_PRIORITY = ["error", "blocked", "done", "running", "unknown", "idle"];

function summaryState(states) {
  for (const p of TRAY_PRIORITY) if (states.includes(p)) return p;
  return "empty";
}

// Draw the dot row (or a single condensed dot) and return RGBA pixels + dims.
// Rendered at ~2× the menu-bar height so it's crisp on retina; macOS scales the
// image down to the bar height, preserving aspect.
function drawTray(states, colors, condense) {
  const H = 44;
  const D = 22; // dot diameter
  const R = D / 2;
  const G = 12; // gap between dots
  const P = 6; // horizontal padding
  let list;
  if (condense) list = [summaryState(states)];
  else if (states.length === 0) list = ["empty"];
  else list = states;
  const N = list.length;
  const W = Math.max(D + P * 2, P * 2 + N * D + (N - 1) * G);
  if (!trayCanvas) trayCanvas = document.createElement("canvas");
  const cv = trayCanvas;
  cv.width = W;
  cv.height = H;
  const ctx = cv.getContext("2d");
  ctx.clearRect(0, 0, W, H);
  list.forEach((st, i) => {
    const cx = P + R + i * (D + G);
    const cy = H / 2;
    let fill = colors[st] || colors.idle;
    let alpha = 1;
    if (st === "empty") {
      fill = "#ffffff";
      alpha = 0.28;
    } else if (st === "idle") {
      alpha = 0.55; // dim acknowledged/dormant sessions, like the bar
    }
    ctx.globalAlpha = alpha;
    ctx.beginPath();
    // "unknown" draws a hollow ring, matching the bar's `.dot.unknown`.
    if (st === "unknown") {
      const LW = 3;
      ctx.arc(cx, cy, R - LW / 2, 0, Math.PI * 2);
      ctx.lineWidth = LW;
      ctx.strokeStyle = fill;
      ctx.stroke();
      return;
    }
    ctx.arc(cx, cy, R, 0, Math.PI * 2);
    ctx.fillStyle = fill;
    ctx.fill();
  });
  ctx.globalAlpha = 1;
  const data = ctx.getImageData(0, 0, W, H).data;
  return { rgba: Array.from(data), width: W, height: H };
}

// Build the tray image from the latest poll and push it to Rust if it changed.
async function pushTrayImage() {
  if (currentMode() !== "menubar") return;
  const colors = currentColors();
  const condense = currentCondense();
  const states = visibleSessions().map(displayState);
  const sig = JSON.stringify([states, colors, condense]);
  if (sig === lastTraySig) return;
  lastTraySig = sig;
  const img = drawTray(states, colors, condense);
  try {
    await invoke("set_tray_image", { rgba: img.rgba, width: img.width, height: img.height });
  } catch (_) {
    /* fail-silent */
  }
}

// Hide the popover (menu-bar mode) — used after clicking a light. A subsequent tray
// click re-shows it (Rust toggles on is_visible).
function hidePopover() {
  try {
    appWindow.hide();
  } catch (_) {
    /* fail-silent */
  }
}

// Drop the latched hover scale after a click hands focus to another app: no
// mouseleave ever arrives, so the dot would stay enlarged. Cleared on the next
// mousemove, i.e. as soon as the pointer's real position is known again.
function suppressHover() {
  const bar = document.getElementById("bar");
  if (!bar || bar.classList.contains("nohover")) return;
  bar.classList.add("nohover");
  document.addEventListener("mousemove", () => bar.classList.remove("nohover"), { once: true });
}

// Push size + colors into the CSS variables on #bar, which drive dot geometry and
// every state color (including the color-mix glow).
function applyStyle() {
  const bar = document.getElementById("bar");
  const size = currentSize();
  const pad = currentPad();
  const opacity = currentOpacity();
  const colors = currentColors();
  bar.style.setProperty("--dot-size", `${size}px`);
  bar.style.setProperty("--bar-pad", `${pad}px`);
  bar.style.setProperty("--bar-opacity", String(opacity / 100));
  for (const [state, hex] of Object.entries(colors)) {
    bar.style.setProperty(`--c-${state}`, hex);
  }
  resizeToContent(); // a size change reshapes the bar
}

// ── Audio alerts (decision 0xx) ─────────────────────────────────────────────
// A per-state chime plays once when a session *transitions into* an attention
// state (blocked/error/done) — edge-triggered, so a light that stays orange beeps
// only on arrival, not every poll. Off by default (UI Principle #1: non-intrusive);
// the master toggle reveals per-state checkboxes + a volume slider. App-local, same
// localStorage pattern as the other display prefs. Never touches the status files.
const CHIME_STATES = ["blocked", "error", "done"];

// Short WebAudio tones — no bundled asset, so nothing to load and no CSP concern.
// Each state gets a distinct shape: blocked rises (a question), error is a lower
// urgent double, done is a single soft note.
let audioCtx = null;
const CHIME_TONES = {
  blocked: [{ f: 660, t: 0, d: 0.12 }, { f: 880, t: 0.12, d: 0.15 }],
  error: [{ f: 392, t: 0, d: 0.14 }, { f: 311, t: 0.16, d: 0.2 }],
  done: [{ f: 784, t: 0, d: 0.16 }],
};

function playChime(state, previewVol) {
  const tones = CHIME_TONES[state];
  if (!tones) return;
  const vol = (previewVol != null ? previewVol : currentVolume()) / 100;
  if (vol <= 0) return;
  try {
    if (!audioCtx) audioCtx = new (window.AudioContext || window.webkitAudioContext)();
    if (audioCtx.state === "suspended") audioCtx.resume();
    const now = audioCtx.currentTime;
    for (const tone of tones) {
      const osc = audioCtx.createOscillator();
      const gain = audioCtx.createGain();
      osc.type = "sine";
      osc.frequency.value = tone.f;
      const start = now + tone.t;
      const end = start + tone.d;
      // Quick attack, exponential decay — a soft blip, never a click.
      gain.gain.setValueAtTime(0.0001, start);
      gain.gain.linearRampToValueAtTime(vol * 0.25, start + 0.012);
      gain.gain.exponentialRampToValueAtTime(0.0001, end);
      osc.connect(gain).connect(audioCtx.destination);
      osc.start(start);
      osc.stop(end + 0.02);
    }
  } catch (_) {
    /* audio unavailable → silent (never surface noise) */
  }
}

// Edge-triggered chime dispatch. prevChimeState is seeded on the first poll WITHOUT
// firing (audioSeeded guard) so pre-existing blocked/error sessions don't blast on
// launch — a chime only ever marks a fresh transition.
const prevChimeState = new Map(); // id -> displayState at last check
let audioSeeded = false;
function checkChimes(sessions) {
  const seen = new Set();
  for (const s of sessions) {
    seen.add(s.id);
    const ds = displayState(s);
    const prev = prevChimeState.get(s.id);
    prevChimeState.set(s.id, ds);
    if (audioSeeded && ds !== prev && audioEnabled() && CHIME_STATES.includes(ds) && currentChimes()[ds]) {
      playChime(ds);
    }
  }
  for (const id of prevChimeState.keys()) {
    if (!seen.has(id)) prevChimeState.delete(id);
  }
  audioSeeded = true;
}

// ── Applying a preference the settings window changed (decision 082) ────────
// The settings window writes the value and says which key moved; this is the one
// place that knows what a preference actually *does* to the bar.

// The chime map as of the last apply, so turning one on can preview it — the window
// sends the whole map, not which checkbox moved.
let lastChimes = currentChimes();

function applyPref(key) {
  switch (key) {
    case MODE_KEY:
      applyMode(currentMode());
      break;
    case CONDENSE_KEY:
      repaintTray(); // the tray icon's shape changed
      break;
    case ORIENT_KEY:
      applyOrientation(effectiveOrientation());
      break;
    case SORT_KEY:
      latestSessions = sortSessions(latestSessions);
      render(visibleSessions());
      repaintTray();
      break;
    case UNKNOWN_KEY:
      render(visibleSessions());
      repaintTray();
      break;
    case PIP_KEY:
      // Hiding the pip also stops the Accessibility read behind it — the one thing the
      // bar does that can cancel Cursor's own menu while it is open (decisions 081/082).
      if (showCursorPip()) {
        refreshCursorAttention().then(() => render(visibleSessions()));
      } else {
        cursorAttention = 0;
        render(visibleSessions());
      }
      break;
    case SIZE_KEY:
    case PAD_KEY:
    case OPACITY_KEY:
      applyStyle();
      break;
    case COLORS_KEY:
      applyStyle();
      repaintTray(); // the tray icon draws with the same per-state colors
      break;
    case AUDIO_KEY:
      if (audioEnabled()) playChime("done"); // audible confirmation the toggle took
      break;
    case CHIME_KEY: {
      const now = currentChimes();
      for (const st of CHIME_STATES) if (now[st] && !lastChimes[st]) playChime(st);
      lastChimes = now;
      break;
    }
    case VOL_KEY:
      playChime("done", currentVolume()); // preview at the new level
      break;
  }
}

// Re-apply everything from storage — for a reset, where every key moved at once.
function applyAllPrefs() {
  lastChimes = currentChimes();
  applyOrientation(effectiveOrientation());
  applyStyle();
  latestSessions = sortSessions(latestSessions);
  render(visibleSessions());
  applyMode(currentMode()); // back to floating: shows the bar, hides the tray
}

// Show or hide the per-light close buttons, which ride with the settings window
// (decisions 080/082), while keeping the lights pinned in place. Anchor to the lights'
// current screen position, mutate the layout, resize the window to hug the new
// content, then move the window so the lights land back on that anchor. The window
// grows/shrinks around the lights — they never move. Capturing the anchor fresh each
// time means a drag while the buttons are up is respected (hiding keeps them where
// they now are, not where they were revealed).
async function showClosers(on) {
  const closerRow = document.getElementById("closers");
  const bar = document.getElementById("bar");
  if (on === !closerRow.hasAttribute("hidden")) return; // already in that state
  const anchor = await lightsScreenPos();
  // Both directions move the lights *inside* the window before the window itself can
  // catch up: the row is inserted (or dropped) on the frame it is toggled, while the
  // resize and the re-anchor are two more async IPC round trips behind it. Those frames
  // are the flicker. Decision 075 suppressed the paint for the collapse only, because
  // the settings panel then grew *below* the lights and revealing moved nothing; the
  // controls row is placed by `panel-above`, so it lands on the side the lights sit on
  // and shifts them either way. So both directions are suppressed now, symmetrically.
  // `visibility` rather than `display`/`opacity`: it stops the paint while keeping the
  // layout that `resizeToContent` has to measure.
  bar.style.visibility = "hidden";
  if (on) {
    if (anchor) await chooseGrowthDirection(anchor); // above/below, left/right toward center
    closerRow.removeAttribute("hidden");
  } else {
    closerRow.setAttribute("hidden", "");
    bar.classList.remove("panel-above", "panel-left");
    bar.style.alignItems = "";
  }
  try {
    await resizeToContent();
    await anchorLightsTo(anchor);
    await fitPopover();
  } finally {
    // Paint again only once the window is its final size and back in position, so the
    // first frame the user sees is the finished one. In a `finally` because
    // `resizeToContent` waits on animation frames, which stall if the popover is
    // dismissed mid-toggle — and a bar left `visibility: hidden` would stay invisible.
    bar.style.visibility = "";
  }
}

// The popover just appeared. Re-size it — the webview pauses rAF while hidden, so the
// layout may be stale — and pull it back inside the work area (decision 073).
async function onPopoverShown() {
  if (currentMode() !== "menubar") return;
  await resizeToContent();
  await fitPopover();
}

async function fitPopover() {
  if (currentMode() !== "menubar") return;
  try {
    await invoke("fit_popover");
  } catch (_) {
    /* fail-silent: the panel is merely positioned awkwardly, not broken */
  }
}

function initBar() {
  applyOrientation(effectiveOrientation());
  applyStyle();
  // Right-click anywhere on the bar (including a dot) reveals its controls — one close
  // button per light, plus the gear that opens the settings window (decision 082).
  // Right-click again puts them away. Suppress the native context menu.
  document.getElementById("bar").addEventListener("contextmenu", (e) => {
    e.preventDefault();
    showClosers(document.getElementById("closers").hasAttribute("hidden"));
  });
}

// Reviewed-state tracking (app-local; decision 014). A session that just finished
// a turn shows as "done" — a steady white attention light — until the user clicks
// it (which also jumps to the session), acknowledging that its output was seen.
// The ack is keyed by the finish time (updated_at), so the NEXT time a turn
// finishes the light re-lights on its own. This lives only in the app, never in
// the hook-written status file — the hook stays dumb and fast.
const reviewedAt = new Map(); // session id -> updated_at that was acknowledged

// A finished turn = idle with a wrap-up message. `Stop` writes a non-empty detail
// (the last assistant message); `SessionStart` forces detail="" — so detail is the
// reliable "a turn ended and there's output to review" signal (vs. a fresh idle).
// Cursor's bridged `Stop` carries no wrap-up message (verified, decision 038), so a
// Cursor light could never take that path — it dropped straight from green to dim
// idle with nothing saying "this finished and you haven't looked at it yet". For
// Cursor the finish comes from the observed transition instead (see noteFinishes).
function isFinishedTurn(s) {
  return s.state === "idle" && (!!s.detail || finishedAt.get(s.id) === s.updated_at);
}

// The unread signal for hosts that report no wrap-up message: the poll that first
// sees a session leave a non-idle state for idle IS the finish. We remember the
// `updated_at` it landed on, so every later poll (which sees only a plain idle)
// still reads as finished, and the same click/updated_at key that acknowledges a
// Claude Code "done" light acknowledges this one — the next finish re-lights it.
// Only Cursor is recorded: Claude Code's detail is a real, restart-proof signal,
// and deriving its lights from transitions too would just make them forget on a
// reload. A session already idle when we first see it is NOT a finish (no `prev`),
// so relaunching the bar never invents unread lights for old turns.
const finishedAt = new Map(); // session id -> updated_at of a finish we watched happen
const prevRawState = new Map(); // session id -> raw state at the previous poll

function noteFinishes(sessions) {
  const seen = new Set();
  for (const s of sessions) {
    seen.add(s.id);
    const prev = prevRawState.get(s.id);
    prevRawState.set(s.id, s.state);
    if (s.ide === "cursor" && s.state === "idle" && prev && prev !== "idle") {
      finishedAt.set(s.id, s.updated_at);
    }
  }
  for (const id of prevRawState.keys()) if (!seen.has(id)) prevRawState.delete(id);
  for (const id of finishedAt.keys()) if (!seen.has(id)) finishedAt.delete(id);
}

// A Cursor session with no workspace folder is unobservable (decision 042): Cursor
// runs command hooks only when a folder is open, so a folder-less window fires the
// bridged `sessionStart` (which is why the light exists at all) and then nothing —
// no prompt, tool, or stop events, verified in Cursor 3.12.10's own hook logs. Its
// recorded "idle" is the single stale event, not a real state, so we must not render
// it as one (UI Principle #4).
function isUnobservable(s) {
  return s.ide === "cursor" && !s.cwd;
}

// The state the light actually renders: "unknown" for a session we get no signal
// from, "done" for an unacknowledged finished turn, otherwise the raw session state
// (running/blocked/idle/error).
function displayState(s) {
  if (isUnobservable(s)) return "unknown";
  if (isFinishedTurn(s) && reviewedAt.get(s.id) !== s.updated_at) return "done";
  return s.state;
}

function shortId(id) {
  return id.length > 8 ? id.slice(0, 8) : id;
}

// "3 subagents: general-purpose ×2, Explore" (grouped by type with counts).
function subSummary(subs) {
  if (!subs || subs.length === 0) return "";
  const counts = {};
  for (const t of subs) counts[t] = (counts[t] || 0) + 1;
  const parts = Object.entries(counts).map(([t, c]) => (c > 1 ? `${t} ×${c}` : t));
  return `${subs.length} subagent${subs.length > 1 ? "s" : ""}: ${parts.join(", ")}`;
}

// What identifies the light in its tooltip: the project folder plus the host's own
// name for the session ("AgentStatus · agentstatus-5b"), which is the only thing that
// tells two sessions in the same folder apart. The name is dropped when it adds
// nothing (absent, or the same text as the folder), and stands alone when there's no
// folder — a light is never labeled with both nothing and a redundancy (decision 053).
function headFor(s) {
  const label = (s.label || "").trim();
  const name = (s.name || "").trim();
  if (label && name && name.toLowerCase() !== label.toLowerCase()) return `${label} · ${name}`;
  return label || name || shortId(s.id);
}

// Which application the session is running in — "VS Code", "Cursor", "Ghostty" — resolved
// by the backend, which is the only side that can walk a terminal session to its emulator
// (decision 060). It qualifies the identity, so it sits right after it and leaves the state
// at the end of the line where it already was.
function appTag(s) {
  const app = (s.app || "").trim();
  return app ? ` (${app})` : "";
}

// The hover tooltip: name (app) — state, the task, active subagents, then the activity.
function titleFor(s, ds) {
  // Say plainly that we have no signal, and why — never imply a state we don't know.
  if (ds === "unknown") {
    return (
      `Cursor ${(s.name || "").trim() || shortId(s.id)} — state unknown\n` +
      `↳ no folder open in this Cursor window, so it reports no progress · click to open Cursor`
    );
  }
  const stateText = ds === "done" ? "finished — click to acknowledge" : ds;
  const lines = [`${headFor(s)}${appTag(s)} — ${stateText}`];
  if (s.task) lines.push(`↳ ${s.task}`);
  const subs = subSummary(s.subagents);
  if (subs) lines.push(subs);
  if (s.detail) lines.push(s.detail);
  return lines.join("\n");
}

// The gear that opens the settings window, kept as the last item of the controls row
// so it sits one slot past the lights and never takes a light's place (decision 082).
// Built once and re-appended, because `render` rebuilds the row's order every poll.
let gearEl = null;

function ensureGear(closerRow) {
  if (!gearEl) {
    gearEl = document.createElement("div");
    gearEl.className = "gear";
    // Drawn, not typed: U+2699 renders as a color emoji in some system fonts, and at
    // dot size an emoji gear is a smudge. Eight teeth around a ring, in `currentColor`
    // so it inherits the row's ink. It overflows its dot-sized box on purpose — the box
    // has to stay a dot wide to keep the close buttons on the lights' grid (decision 080).
    gearEl.innerHTML =
      '<svg viewBox="-12 -12 24 24" aria-hidden="true">' +
      [0, 45, 90, 135, 180, 225, 270, 315]
        .map(
          (a) =>
            `<rect x="-2.1" y="-11.2" width="4.2" height="5.4" rx="1.1" transform="rotate(${a})"/>`
        )
        .join("") +
      '<circle r="6.1" fill="none" stroke="currentColor" stroke-width="3.4"/></svg>';
    gearEl.title = "AgentStatus settings";
    gearEl.addEventListener("click", () => {
      invoke("open_settings").catch(() => {});
      suppressHover();
    });
  }
  if (closerRow.lastElementChild !== gearEl) closerRow.appendChild(gearEl);
}

// Remove a light's close button when the light itself goes.
function dropCloser(id) {
  const x = closers.get(id);
  if (!x) return;
  x.remove();
  closers.delete(id);
}

function render(sessions) {
  const lights = document.getElementById("lights");
  const closerRow = document.getElementById("closers");
  let sizeChanged = false;

  // The Cursor menu-bar pip counts as content, so a bar with only pending Cursor
  // notifications (no tracked sessions) shows the pip, not the "empty" placeholder.
  if (sessions.length === 0 && cursorAttention === 0) {
    for (const [id, el] of dots) {
      el.remove();
      dots.delete(id);
      reviewedAt.delete(id);
      dropCloser(id);
      sizeChanged = true;
    }
    if (renderCursorPip(lights)) sizeChanged = true; // removes a lingering pip
    ensureGear(closerRow);
    if (!emptyEl) {
      emptyEl = document.createElement("div");
      emptyEl.className = "dot empty";
      emptyEl.title = "No active Claude Code or Cursor sessions";
      emptyEl.setAttribute("data-tauri-drag-region", "");
      lights.appendChild(emptyEl);
      sizeChanged = true;
    }
    if (sizeChanged) resizeToContent();
    return;
  }
  if (emptyEl) {
    emptyEl.remove();
    emptyEl = null;
    sizeChanged = true;
  }

  const seen = new Set();
  sessions.forEach((s, i) => {
    seen.add(s.id);
    let el = dots.get(s.id);
    if (!el) {
      el = document.createElement("div");
      // Click acknowledges a finished turn (keyed by its finish time) AND jumps to
      // the session's window. Reads cwd/updated_at off the element so both stay
      // correct across updates. NOT a drag region → a click never drags.
      el.addEventListener("click", () => {
        if (el._updatedAt != null) reviewedAt.set(s.id, el._updatedAt);
        if (el.className === "dot done") el.className = "dot idle"; // instant feedback
        focusSession(el._cwd, el._ide, s.id);
        suppressHover();
        if (currentMode() === "menubar") hidePopover(); // dismiss the popover on select
      });
      dots.set(s.id, el);
      sizeChanged = true;
    }
    el._cwd = s.cwd;
    el._ide = s.ide;
    el._updatedAt = s.updated_at;
    // Only touch the DOM when something actually changed, so an open tooltip
    // (and the hover) isn't disrupted every poll.
    const ds = displayState(s);
    const cls = `dot ${ds}`;
    if (el.className !== cls) el.className = cls;
    const title = titleFor(s, ds);
    if (el.title !== title) el.title = title;
    // Subagent count badge.
    const n = (s.subagents || []).length;
    let badge = el.firstElementChild;
    if (n > 0) {
      if (!badge) {
        badge = document.createElement("span");
        badge.className = "badge";
        el.appendChild(badge);
      }
      const txt = String(n);
      if (badge.textContent !== txt) badge.textContent = txt;
    } else if (badge) {
      badge.remove();
    }
    // Keep DOM order matching session order.
    const ref = lights.children[i];
    if (ref !== el) lights.insertBefore(el, ref || null);
    // The light's close button, held at the same index in the closers row so the two
    // stay aligned (decision 080). Built whether or not the row is currently shown —
    // the row's own `hidden` decides that, so opening the panel reveals a row that is
    // already correct.
    let x = closers.get(s.id);
    if (!x) {
      x = document.createElement("div");
      x.className = "closer";
      x.textContent = "\u00d7";
      x.addEventListener("click", () => dismissSession(s.id));
      closers.set(s.id, x);
    }
    const xTitle = `Close ${headFor(s)}\n\u21b3 drops this light now; it returns if the session is still active`;
    if (x.title !== xTitle) x.title = xTitle;
    const xref = closerRow.children[i];
    if (xref !== x) closerRow.insertBefore(x, xref || null);
  });

  for (const [id, el] of dots) {
    if (!seen.has(id)) {
      el.remove();
      dots.delete(id);
      reviewedAt.delete(id);
      dropCloser(id);
      sizeChanged = true;
    }
  }

  if (renderCursorPip(lights)) sizeChanged = true;
  ensureGear(closerRow);

  if (sizeChanged) resizeToContent();
}

// Render (or remove) the single aggregate Cursor menu-bar pip as the last element in
// the bar. It shows the count of Cursor composers awaiting the user (decision 038),
// clicks through the waiting composers one at a time (decision 045), and is styled
// distinctly (.cursor-pip) so it reads as the Cursor menu-bar mirror, not a session
// light. Returns true if it added or removed the element (so the caller re-measures
// the bar).
function renderCursorPip(lights) {
  if (cursorAttention > 0) {
    let created = false;
    if (!cursorPipEl) {
      cursorPipEl = document.createElement("div");
      cursorPipEl.className = "dot done cursor-pip";
      cursorPipEl.addEventListener("click", () => {
        openNextCursorAttention();
        suppressHover();
        if (currentMode() === "menubar") hidePopover();
      });
      const badge = document.createElement("span");
      badge.className = "badge";
      cursorPipEl.appendChild(badge);
      created = true;
    }
    const txt = String(cursorAttention);
    const badge = cursorPipEl.firstElementChild;
    if (badge.textContent !== txt) badge.textContent = txt;
    const title =
      `Cursor — ${cursorAttention} composer${cursorAttention > 1 ? "s" : ""} awaiting you\n` +
      `↳ from Cursor's menu bar · click to open the next one (clears Cursor's notifications)`;
    if (cursorPipEl.title !== title) cursorPipEl.title = title;
    if (lights.lastElementChild !== cursorPipEl) lights.appendChild(cursorPipEl);
    return created;
  }
  if (cursorPipEl) {
    cursorPipEl.remove();
    cursorPipEl = null;
    return true;
  }
  return false;
}

// +BADGE_PAD so a subagent badge overflowing the last dot's corner isn't clipped.
const BADGE_PAD = 6;

// The window size the content asks for right now. #bar is an auto-sized inline-flex
// box, so it measures its full content even when the window is currently too small
// to show it — which is what lets ensureSized() detect a clipped bar.
function contentSize() {
  const rect = document.getElementById("bar").getBoundingClientRect();
  if (!rect.width) return null; // pre-paint: measuring now would shrink to nothing
  return {
    w: Math.max(MIN_W, Math.ceil(rect.width) + BADGE_PAD),
    h: Math.max(MIN_H, Math.ceil(rect.height)),
  };
}

let appliedSize = null; // the size we last successfully set the window to

async function resizeToContent() {
  if (!AUTO_RESIZE || !LogicalSize) return;
  // Skip while the panel is hidden (menu-bar mode, popover closed): the webview
  // pauses requestAnimationFrame when off-screen, so the double-rAF below would
  // never resolve. The visibilitychange handler re-runs this when the popover
  // reappears, so the panel sizes correctly on open.
  if (document.hidden) return;
  // Wait for layout+paint so we never measure a 0-width bar (which shrank the
  // window to nothing before the content rendered). Bounded: animation frames stop firing
  // if the window is hidden mid-wait (a tray popover dismissed during a collapse), and an
  // unbounded wait here would leave every caller's cleanup — notably the visibility restore
  // in `showClosers` — permanently pending. Whichever fires first wins; resolving twice
  // is a no-op.
  await new Promise((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(resolve));
    setTimeout(resolve, 250);
  });
  const size = contentSize();
  if (!size) return;
  try {
    await appWindow.setSize(new LogicalSize(size.w, size.h));
    appliedSize = size;
  } catch (_) {
    /* fail-silent: keep last size */
  }
}

// Self-heal a window that doesn't match its content. A resize is normally triggered
// only when a light is added or removed, so a single lost one leaves the pill clipped
// (the bar sized for one light with five in it) until the next add/remove — and a
// resize CAN be lost: it awaits two animation frames, which the webview stops
// delivering while the window isn't being painted, so a size change that lands during
// launch or a relaunch can simply never apply. Checking the measurement each poll
// costs one layout read and fixes it on the next tick instead of never.
function ensureSized() {
  if (!AUTO_RESIZE || !LogicalSize || document.hidden) return;
  const size = contentSize();
  if (!size) return;
  if (!appliedSize || appliedSize.w !== size.w || appliedSize.h !== size.h) {
    resizeToContent();
  }
}

// Magnetic snap distance (logical px) for pinning the bar flush to a monitor edge.
const SNAP_LOGICAL = 16;

// Keep the bar on-screen across ALL monitors, with soft edge magnetism. Native
// drag regions let the OS move the window freely, so on each move we correct it in
// three steps: (1) snap any window edge sitting within SNAP of a monitor edge flush
// to it — easy pinning; (2) clamp the window inside the bounding box of every
// monitor so it can't leave the outer edges, while still sliding freely across the
// shared edges between displays; (3) if the bar's center lands in a dead gap
// between mismatched monitors, pull the whole bar onto the nearest one so it can
// never be lost. Re-entrancy is safe — the corrected position is in-bounds, so the
// setPosition it triggers produces a no-op moved event.
async function clampToMonitor(pos) {
  if (!PhysicalPosition) return;
  let monitors = [];
  try {
    if (availableMonitors) monitors = await availableMonitors();
    if (!monitors.length && currentMonitor) {
      const m = await currentMonitor();
      if (m) monitors = [m];
    }
  } catch (_) {
    return;
  }
  if (!monitors.length) return;

  let size;
  try {
    size = await appWindow.outerSize();
  } catch (_) {
    return;
  }
  const w = size.width;
  const h = size.height;
  let x = pos.x;
  let y = pos.y;

  // Per-monitor rectangles (physical px) with a scale-aware snap zone.
  const rects = monitors.map((m) => ({
    l: m.position.x,
    t: m.position.y,
    r: m.position.x + m.size.width,
    b: m.position.y + m.size.height,
    snap: Math.round(SNAP_LOGICAL * (m.scaleFactor || 1)),
  }));

  // (1) Edge magnetism — snap flush to any monitor edge within its snap zone.
  for (const m of rects) {
    if (Math.abs(x - m.l) <= m.snap) x = m.l;
    if (Math.abs(x + w - m.r) <= m.snap) x = m.r - w;
    if (Math.abs(y - m.t) <= m.snap) y = m.t;
    if (Math.abs(y + h - m.b) <= m.snap) y = m.b - h;
  }

  // (2) Clamp inside the bounding box of all monitors.
  const minX = Math.min(...rects.map((m) => m.l));
  const minY = Math.min(...rects.map((m) => m.t));
  const maxX = Math.max(...rects.map((m) => m.r)) - w;
  const maxY = Math.max(...rects.map((m) => m.b)) - h;
  if (maxX >= minX) x = Math.max(minX, Math.min(x, maxX));
  if (maxY >= minY) y = Math.max(minY, Math.min(y, maxY));

  // (3) Gap guard — if the bar's center isn't on any monitor (a dead zone between
  //     mismatched displays), pull the whole bar onto the nearest monitor.
  const cx = x + w / 2;
  const cy = y + h / 2;
  const onScreen = rects.some((m) => cx >= m.l && cx < m.r && cy >= m.t && cy < m.b);
  if (!onScreen) {
    let best = rects[0];
    let bestD = Infinity;
    for (const m of rects) {
      const dx = cx < m.l ? m.l - cx : cx > m.r ? cx - m.r : 0;
      const dy = cy < m.t ? m.t - cy : cy > m.b ? cy - m.b : 0;
      const d = dx * dx + dy * dy;
      if (d < bestD) {
        bestD = d;
        best = m;
      }
    }
    x = Math.max(best.l, Math.min(x, best.r - w));
    y = Math.max(best.t, Math.min(y, best.b - h));
  }

  if (x !== pos.x || y !== pos.y) {
    try {
      await appWindow.setPosition(new PhysicalPosition(x, y));
    } catch (_) {
      /* fail-silent */
    }
  }
}

// Persist WHERE THE LIGHTS SIT on screen — not the window's top-left. The window
// grows/shrinks around the fixed lights as the settings panel opens/closes, so its
// top-left depends on panel state; restoring it onto a differently-sized window
// would shift the lights (e.g. reloading from the panel-open state). The lights'
// screen position is stable, so that's what we save and restore. localStorage lives
// in the webview data dir (keyed by bundle id), so it survives replacing the .app.
function saveAnchor(a) {
  try {
    localStorage.setItem(
      POS_KEY,
      JSON.stringify({ x: Math.round(a.x), y: Math.round(a.y), scale: a.scale })
    );
  } catch (_) {
    /* fail-silent */
  }
}

function loadAnchor() {
  try {
    const a = JSON.parse(localStorage.getItem(POS_KEY) || "null");
    if (a && Number.isFinite(a.x) && Number.isFinite(a.y) && Number.isFinite(a.scale)) return a;
  } catch (_) {
    /* fall through */
  }
  return null;
}

// Don't persist the involuntary window moves that setSize triggers during startup
// layout — only start saving once we've restored the user's position.
let anchorReady = false;

// On any move: keep the bar on-screen, then (once ready) remember the lights' new
// screen position so a restart/rebuild reopens them here instead of recentering.
async function onWindowMoved(pos) {
  // In menu-bar mode the popover's position is tray-driven, not user-dragged — don't
  // clamp or persist it (that's a floating-mode concern; decision 022).
  if (currentMode() === "menubar") return;
  await clampToMonitor(pos);
  if (!anchorReady) return;
  const a = await lightsScreenPos();
  if (a) saveAnchor(a);
}

// Restore the saved lights position on launch (overriding the config's center):
// size the bar to its final shape, then shift the window so the lights land back on
// the saved anchor. clampToMonitor keeps it on-screen if the display setup changed.
async function restoreAnchor() {
  const saved = loadAnchor();
  if (!saved) return;
  await resizeToContent();
  await anchorLightsTo(saved);
  try {
    await clampToMonitor(await appWindow.outerPosition());
  } catch (_) {
    /* fail-silent */
  }
}

// Physical screen coordinates of the #lights element's top-left corner.
async function lightsScreenPos() {
  if (!currentMonitor) return null;
  const mon = await currentMonitor();
  const scale = (mon && mon.scaleFactor) || 1;
  let pos;
  try {
    pos = await appWindow.outerPosition();
  } catch (_) {
    return null;
  }
  const r = document.getElementById("lights").getBoundingClientRect();
  return { x: pos.x + Math.round(r.left * scale), y: pos.y + Math.round(r.top * scale), scale };
}

// Move the window so #lights sits back at `anchor` (its screen position from before
// the panel opened). Layout-agnostic: it measures where the lights currently are and
// corrects the delta — so the lights never move as the panel grows/shrinks around
// them, whichever side (above/below, left/right) the panel expanded toward.
async function anchorLightsTo(anchor) {
  if (!anchor || !PhysicalPosition) return;
  let pos;
  try {
    pos = await appWindow.outerPosition();
  } catch (_) {
    return;
  }
  const r = document.getElementById("lights").getBoundingClientRect();
  const curX = pos.x + Math.round(r.left * anchor.scale);
  const curY = pos.y + Math.round(r.top * anchor.scale);
  const nx = pos.x + (anchor.x - curX);
  const ny = pos.y + (anchor.y - curY);
  if (nx !== pos.x || ny !== pos.y) {
    try {
      await appWindow.setPosition(new PhysicalPosition(nx, ny));
    } catch (_) {
      /* fail-silent */
    }
  }
}

// Pick which way the panel grows so it heads toward the screen's middle (and stays
// on-screen): panel above the lights when they're in the bottom half, below when in
// the top half; panel aligned to the lights' right edge (grows left) when they're in
// the right half, left edge (grows right) when in the left half. The lights stay put
// regardless — this only decides the direction the extra space appears.
async function chooseGrowthDirection(anchor) {
  if (!currentMonitor) return;
  const mon = await currentMonitor();
  if (!mon) return;
  const bar = document.getElementById("bar");
  const r = document.getElementById("lights").getBoundingClientRect();
  const cx = anchor.x + (r.width * anchor.scale) / 2;
  const cy = anchor.y + (r.height * anchor.scale) / 2;
  const monCx = mon.position.x + mon.size.width / 2;
  const monCy = mon.position.y + mon.size.height / 2;
  bar.classList.toggle("panel-above", cy > monCy);
  // The close-button row grows the strip the same way the panel grows the bar, so it
  // follows the same rule: put it on the side facing the screen's middle, or a bar
  // resting on the bottom (or right) edge pushes its own buttons off-screen. A
  // horizontal row of lights takes the vertical answer, a vertical column the
  // horizontal one — which is why `panel-above` alone can't serve both (decision 080).
  bar.classList.toggle("panel-left", cx > monCx);
  bar.style.alignItems = cx > monCx ? "flex-end" : "flex-start";
}

async function focusSession(cwd, ide, id) {
  // Empty cwd is normally a no-op (nothing to focus), except the Cursor menu-bar pip
  // (decision 038) passes an empty cwd on purpose — the backend just activates Cursor.
  if (!cwd && ide !== "cursor") return;
  try {
    // sessionId → the extension focuses that exact session's tab (decision 019);
    // cwd/ide → the backend raises the right window. Tauri maps camelCase → snake_case.
    await invoke("focus_session", { cwd, ide: ide || "vscode", sessionId: id || "" });
  } catch (_) {
    /* fail-silent */
  }
}

// Re-read Cursor's menu-bar count. Fail-closed to 0 (no pip) on any error.
async function refreshCursorAttention() {
  // Off means off: no pip, and no Accessibility read either (decision 082).
  if (!showCursorPip()) {
    cursorAttention = 0;
    return;
  }
  try {
    cursorAttention = await invoke("cursor_attention_count");
  } catch (_) {
    cursorAttention = 0;
  }
}

// Click-through for the Cursor pip (decisions 045, 083): press the top entry in Cursor's
// tray menu that carries a notification — Cursor opens that composer — then press its
// "Clear All Notifications" entry, which is the only thing that actually marks composers
// read (opening one no longer does, verified on Cursor 3.15.6). Cursor exposes no
// per-composer clear, so one click empties the whole count, not just the opened
// composer's. The backend also activates Cursor after the presses, which they alone do
// not do (decision 046). Zero the count locally for immediate feedback, then re-read the
// real one once Cursor has updated its menu-bar item. If nothing was pressable (no
// notified entry, Cursor gone, AX not granted), fall back to the old behaviour: activate
// Cursor.
async function openNextCursorAttention() {
  let pressed = false;
  try {
    pressed = await invoke("cursor_open_next_attention");
  } catch (_) {
    /* fail-silent */
  }
  if (!pressed) {
    focusSession("", "cursor", ""); // empty cwd ⇒ just activate Cursor.app
    return;
  }
  cursorAttention = 0;
  render(visibleSessions());
  setTimeout(refreshCursorAttention, CURSOR_RECHECK_MS);
}

async function tick() {
  try {
    // Refresh the Cursor menu-bar count on a slower cadence than the session poll —
    // the AX read is not free, so once every few ticks is plenty and keeps it off the
    // hot path (decision 038).
    if (tickCount++ % CURSOR_POLL_EVERY === 0) await refreshCursorAttention();
    const polled = await invoke("list_sessions");
    // Before anything reads displayState() — sorting, chimes, render — record which
    // sessions just finished a turn, which is only visible as a change between polls.
    noteFinishes(polled);
    reapDismissed(polled); // un-hide anything a dismissal didn't actually remove
    const sessions = sortSessions(polled);
    latestSessions = sessions;
    checkChimes(sessions); // edge-triggered audio alerts (seeds silently on first tick)
    render(visibleSessions());
    ensureSized(); // catch a resize that never applied, so the pill is never clipped
    if (currentMode() === "menubar") await pushTrayImage();
  } catch (_) {
    /* backend not ready yet; try again next tick */
  }
}

window.addEventListener("DOMContentLoaded", async () => {
  // Ask the backend what it is running on before anything reads PLATFORM (decision 072).
  // Only the tray's shape and its control labels depend on the answer, and both are applied
  // below — so a failure here just leaves the macOS wording, never a broken bar.
  try {
    PLATFORM = await invoke("platform");
  } catch (_) {
    /* older backend without the command; treat as not-Windows */
  }
  initBar();
  // Bound native drags to the monitor (can't be dragged off-screen) and remember the
  // lights' resting position. Registered before restore so restore's own moves are
  // clamped; anchorReady stays false until restore finishes so those moves aren't saved.
  if (appWindow.onMoved) {
    appWindow.onMoved(({ payload }) => onWindowMoved(payload));
  }
  // The webview pauses rAF while the panel is hidden; when the menu-bar popover
  // reappears, re-run the resize so it sizes to the current dots on open.
  document.addEventListener("visibilitychange", () => {
    if (!document.hidden && currentMode() === "menubar") onPopoverShown();
  });
  // WebView2 keeps the document "visible" while the window is hidden, so on Windows
  // `visibilitychange` never fires for a popover reveal. The backend emits this instead.
  try {
    const { listen } = window.__TAURI__.event;
    listen("popover-shown", () => onPopoverShown());
    // ── The settings window (decision 082) ──────────────────────────────────
    // It writes each change and says which key moved; the bar owns what that key
    // does. The bar also holds the values: it answers `prefs-request` with a
    // snapshot, so the window renders what is actually on screen rather than
    // whatever its own webview's storage happens to hold.
    listen("pref-changed", ({ payload }) => {
      if (!payload || !payload.key) return;
      writePref(payload.key, payload.value ?? null);
      applyPref(payload.key);
    });
    listen("prefs-reset", () => {
      for (const key of PREF_KEYS) writePref(key, null);
      applyAllPrefs();
    });
    listen("prefs-request", () => emitPref("prefs-snapshot", prefsSnapshot()));
    listen("reload-bar", () => window.location.reload());
  } catch (_) {
    /* older backend without the events; the bar still works, settings just won't sync */
  }
  await tick(); // first render, so the bar has its real size before we anchor it
  anchorReady = true;
  // Apply the saved presentation mode: floating restores the anchor and shows the
  // panel; menu-bar hides the panel and paints the tray. (Menu-bar skips the anchor
  // restore — the popover is positioned by the tray, not the saved floating spot.)
  await applyMode(currentMode());
  setInterval(tick, POLL_MS);
});
