// AgentStatus — settings window (decision 082). Pure UI: it renders the current
// preferences, writes each change locally, and announces it. The bar owns applying
// them, so there is exactly one implementation of what a preference *does*.
//
// The bar is also the source of truth for the values: this window asks for a
// snapshot on open rather than trusting its own webview's localStorage, which is a
// separate store that nothing guarantees is in sync.

const { invoke } = window.__TAURI__.core;
const { emit, listen } = window.__TAURI__.event;

// Announce a change and write it here too, so this window's controls stay correct
// without waiting for a round trip.
function setPref(key, value) {
  writePref(key, value);
  emitPref("pref-changed", { key, value });
}

// ── Rendering the controls from the current prefs ───────────────────────────

function markSeg(selector, attr, value) {
  for (const btn of document.querySelectorAll(`${selector} button`)) {
    btn.classList.toggle("active", btn.dataset[attr] === value);
  }
}

function renderAll() {
  const mode = currentMode();
  markSeg("#mode-seg", "mode", mode);
  markSeg("#condense-seg", "condense", String(currentCondense()));
  markSeg("#orient-seg", "orient", currentOrientation());
  markSeg("#sort-seg", "sort", currentSort());
  markSeg("#unknown-seg", "unknown", String(showUnknown()));
  markSeg("#pip-seg", "pip", String(showCursorPip()));
  markSeg("#audio-seg", "audio", audioEnabled() ? "on" : "off");

  document.getElementById("size-range").value = String(currentSize());
  document.getElementById("pad-range").value = String(currentPad());
  document.getElementById("opacity-range").value = String(currentOpacity());
  document.getElementById("vol-range").value = String(currentVolume());

  const colors = currentColors();
  for (const input of document.querySelectorAll('#colors input[type="color"]')) {
    input.value = colors[input.dataset.state] || DEFAULT_COLORS[input.dataset.state];
  }
  const chimes = currentChimes();
  for (const chk of document.querySelectorAll('input[type="checkbox"][data-chime]')) {
    chk.checked = !!chimes[chk.dataset.chime];
  }

  // Conditional rows: a control that cannot do anything is hidden, not shown and
  // ignored (decision 072). Condense is meaningless while floating and forced on
  // Windows; orientation is forced horizontal in menu-bar mode; the Cursor pip is
  // read off the macOS menu bar and has no Windows equivalent.
  document.getElementById("condense-row").hidden = mode !== "menubar" || PLATFORM === "windows";
  document.getElementById("orient-row").hidden = mode === "menubar";
  document.getElementById("pip-row").hidden = PLATFORM === "windows";
  document.getElementById("pip-note").hidden = PLATFORM === "windows";
  document.getElementById("audio-panel").hidden = !audioEnabled();
}

// "Menu bar" is simply the wrong word for the Windows notification area.
function applyPlatformChrome() {
  if (PLATFORM !== "windows") return;
  const modeBtn = document.querySelector('#mode-seg button[data-mode="menubar"]');
  if (modeBtn) modeBtn.textContent = "Tray";
}

// ── Wiring ──────────────────────────────────────────────────────────────────

function onSeg(id, handler) {
  document.getElementById(id).addEventListener("click", (e) => {
    const btn = e.target.closest("button");
    if (!btn) return;
    handler(btn);
    renderAll();
  });
}

function initControls() {
  for (const btn of document.querySelectorAll("#rail button")) {
    btn.addEventListener("click", () => {
      for (const other of document.querySelectorAll("#rail button")) {
        other.classList.toggle("active", other === btn);
      }
      for (const sec of document.querySelectorAll("#pane section")) {
        sec.hidden = sec.dataset.pane !== btn.dataset.pane;
      }
    });
  }

  onSeg("mode-seg", (b) => setPref(MODE_KEY, b.dataset.mode));
  onSeg("condense-seg", (b) => setPref(CONDENSE_KEY, b.dataset.condense));
  onSeg("orient-seg", (b) => setPref(ORIENT_KEY, b.dataset.orient));
  onSeg("sort-seg", (b) => setPref(SORT_KEY, b.dataset.sort));
  onSeg("unknown-seg", (b) => setPref(UNKNOWN_KEY, b.dataset.unknown));
  onSeg("pip-seg", (b) => setPref(PIP_KEY, b.dataset.pip));
  onSeg("audio-seg", (b) => setPref(AUDIO_KEY, b.dataset.audio));

  const slider = (id, key) =>
    document.getElementById(id).addEventListener("input", (e) => setPref(key, e.target.value));
  slider("size-range", SIZE_KEY);
  slider("pad-range", PAD_KEY);
  slider("opacity-range", OPACITY_KEY);
  slider("vol-range", VOL_KEY);

  // `input` fires live as the picker changes, so the bar previews the color instantly.
  document.getElementById("colors").addEventListener("input", (e) => {
    const input = e.target.closest('input[type="color"]');
    if (!input) return;
    setPref(COLORS_KEY, JSON.stringify({ ...currentColors(), [input.dataset.state]: input.value }));
  });

  document.getElementById("audio-panel").addEventListener("input", (e) => {
    const chk = e.target.closest('input[type="checkbox"][data-chime]');
    if (!chk) return;
    setPref(CHIME_KEY, JSON.stringify({ ...currentChimes(), [chk.dataset.chime]: chk.checked }));
  });

  // Reload the webviews — picks up frontend changes and recovers from any stuck
  // state without quitting the app.
  document.getElementById("reload-btn").addEventListener("click", () => {
    emitPref("reload-bar", {});
    window.location.reload();
  });
  document.getElementById("reset-btn").addEventListener("click", () => {
    for (const key of PREF_KEYS) writePref(key, null);
    emitPref("prefs-reset", {});
    renderAll();
  });
  document.getElementById("quit-btn").addEventListener("click", () => invoke("quit_app"));
}

window.addEventListener("DOMContentLoaded", async () => {
  initControls();
  try {
    PLATFORM = await invoke("platform");
  } catch (_) {
    /* treat as not-Windows, which is the pre-existing behaviour */
  }
  applyPlatformChrome();
  renderAll(); // from whatever this webview has, so the window is never blank

  // The bar's values win as soon as they arrive, and again whenever the bar changes a
  // pref on its own (it can fall back to floating when there is no tray to show).
  listen("prefs-snapshot", ({ payload }) => {
    applySnapshot(payload);
    renderAll();
  });
  emitPref("prefs-request", {});

  try {
    document.getElementById("version").textContent = `Version ${await invoke("app_version")}`;
  } catch (_) {
    document.getElementById("version").textContent = "";
  }
});
