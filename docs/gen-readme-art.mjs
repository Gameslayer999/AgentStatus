#!/usr/bin/env node
// Reproducible README art for AgentStatus.
//
// Renders the light bar to self-contained SVGs using the SAME values as the app's
// stylesheet (app/src/styles.css) — dot geometry, per-state colors, and glow specs —
// so the pictures stay faithful to the real UI. Re-run after any visual change:
//
//     node docs/gen-readme-art.mjs
//
// Outputs (committed, embedded in README.md):
//   docs/lightbar-hero.svg         a realistic bar, mixed states — the hero
//   docs/lightbar-states.svg       every state labeled with its meaning
//   docs/lightbar-hover.svg        one light with its badge + hover tooltip
//   docs/lightbar-orientation.svg  the same bar horizontal vs vertical
//   docs/lightbar-settings.svg     the right-click settings panel
//
// SVGs bake in their own dark backdrop so the frosted pill and the white "done"
// light read correctly in both GitHub light and dark themes. GitHub's SVG sanitizer
// strips CSS animation, so the pulsing states (blocked/error) render static here and
// are labeled "(pulsing)".

import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const DIR = dirname(fileURLToPath(import.meta.url));

// ── Values mirrored from app/src/styles.css ──────────────────────────────────
// Per-state fill + glow. glow = [blur-px-ratio, alpha] from each `.dot.<state>`
// box-shadow (the color-mix percentage → alpha). idle carries no glow (opacity 0.55).
const STATES = {
  running: { color: "#2ecc71", glow: 0.7, label: "running", desc: "actively working on a turn" },
  blocked: { color: "#f39c12", glow: 0.85, pulse: true, label: "blocked (pulsing)", desc: "waiting on you — a permission prompt or a question" },
  done: { color: "#ecf0f1", glow: 0.6, label: "done", desc: "the turn just finished and you haven't looked yet" },
  idle: { color: "#7f8c8d", glow: 0, dim: 0.55, label: "idle", desc: "finished and acknowledged (you've focused it)" },
  error: { color: "#e74c3c", glow: 0.85, pulse: true, label: "error (pulsing)", desc: "a turn failed" },
  unknown: { color: "#7f8c8d", glow: 0, dim: 0.75, ring: true, label: "unknown (hollow)", desc: "a session we get no signal from — a Cursor window with no folder open" },
};

// Frosted pill: rgba(20,22,26,.82) fill, rgba(255,255,255,.09) hairline border.
const PILL_FILL = "rgba(20,22,26,0.82)";
const PILL_BORDER = "rgba(255,255,255,0.09)";
// Subagent badge / settings accent — --ui-accent oklch(60% 0.15 262) → sRGB, near-white ink.
const ACCENT = "#4c7dd9";
const ACCENT_INK = "#f4f7fe";
// Settings-panel neutrals — the OKLCH `--ui-*` vars from styles.css, resolved to sRGB.
const PANEL_BG = "#14161a"; // pill fill (rgb 20,22,26), opaque here for a crisp panel
const UI_INK = "#eef1f6"; // --ui-ink: values/text
const UI_MUTED = "#9aa7b4"; // --ui-muted: labels (ink at ~60% on the dark panel)
const UI_LINE = "rgba(238,241,246,0.14)"; // --ui-line: dividers, control borders

// Geometry (styles.css uses 13px dots; scaled up here for crisp README art, ratios kept).
const D = 30; // dot diameter
const R = D / 2;
const GAP = 24;
const PAD = 22; // vertical padding; horizontal is PAD + 8 to pill-out the row

// ── SVG helpers ──────────────────────────────────────────────────────────────
function haloGradient(id, color, alpha) {
  // A soft radial halo standing in for the CSS box-shadow glow.
  return `<radialGradient id="${id}" cx="50%" cy="50%" r="50%">
      <stop offset="0%" stop-color="${color}" stop-opacity="${(alpha * 0.9).toFixed(3)}"/>
      <stop offset="45%" stop-color="${color}" stop-opacity="${(alpha * 0.45).toFixed(3)}"/>
      <stop offset="100%" stop-color="${color}" stop-opacity="0"/>
    </radialGradient>`;
}

function dot(cx, cy, state, gradId, badge) {
  const s = STATES[state];
  let out = "";
  if (s.glow > 0) {
    out += `<circle cx="${cx}" cy="${cy}" r="${D}" fill="url(#${gradId})"/>`;
  }
  const op = s.dim ? ` opacity="${s.dim}"` : "";
  // `ring` states are hollow (`.dot.unknown`: transparent fill, 2px border scaled
  // to this art's larger dot).
  if (s.ring) {
    const lw = (2 / 13) * D;
    out += `<circle cx="${cx}" cy="${cy}" r="${(R - lw / 2).toFixed(2)}" fill="none" stroke="${s.color}" stroke-width="${lw.toFixed(2)}"${op}/>`;
  } else {
    out += `<circle cx="${cx}" cy="${cy}" r="${R}" fill="${s.color}"${op}/>`;
  }
  if (badge != null) {
    const bx = cx + R - 2;
    const by = cy - R + 2;
    out += `<circle cx="${bx}" cy="${by}" r="9.5" fill="rgba(20,22,26,0.92)"/>`;
    out += `<circle cx="${bx}" cy="${by}" r="8" fill="${ACCENT}"/>`;
    out += `<text x="${bx}" y="${by + 0.5}" fill="${ACCENT_INK}" font-family="-apple-system, system-ui, sans-serif" font-size="11" font-weight="700" text-anchor="middle" dominant-baseline="central">${badge}</text>`;
  }
  return out;
}

function backdrop(w, h, windows = false) {
  // A dark, self-contained canvas suggesting the desktop the bar floats over —
  // so the pictures read the same in GitHub's light and dark themes. The faint
  // "app window" panes (hero only) hint at the bar floating over other apps.
  const panes = windows
    ? `<rect x="${w * 0.08}" y="${h * 0.14}" width="${w * 0.5}" height="${h * 0.72}" rx="10" fill="#ffffff" opacity="0.03"/>
    <rect x="${w * 0.62}" y="${h * 0.22}" width="${w * 0.3}" height="${h * 0.56}" rx="10" fill="#ffffff" opacity="0.03"/>`
    : "";
  return `<defs>
      <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0%" stop-color="#1b2733"/>
        <stop offset="100%" stop-color="#0f1720"/>
      </linearGradient>
    </defs>
    <rect x="0" y="0" width="${w}" height="${h}" rx="18" fill="url(#bg)"/>
    ${panes}`;
}

function pill(x, y, w, h) {
  // styles.css uses `border-radius: 999px`, which clamps to half the SMALLER side —
  // a stadium (rounded end-caps, straight sides), whether the bar is a row or a column.
  const rx = Math.min(w, h) / 2;
  return `<g filter="url(#softshadow)">
      <rect x="${x}" y="${y}" width="${w}" height="${h}" rx="${rx}" fill="${PILL_FILL}" stroke="${PILL_BORDER}" stroke-width="1"/>
    </g>`;
}

const SHADOW_DEF = `<filter id="softshadow" x="-40%" y="-40%" width="180%" height="180%">
      <feDropShadow dx="0" dy="4" stdDeviation="7" flood-color="#000000" flood-opacity="0.42"/>
    </filter>`;

// ── 1. Hero: a realistic bar, mixed states ───────────────────────────────────
function hero() {
  const seq = [
    { state: "running" },
    { state: "blocked" },
    { state: "done" },
    { state: "running", badge: 2 },
    { state: "error" },
    { state: "idle" },
  ];
  const n = seq.length;
  const innerW = n * D + (n - 1) * GAP;
  const pillW = innerW + 2 * (PAD + 8);
  const pillH = D + 2 * PAD;
  const W = 620;
  const H = 200;
  const px = (W - pillW) / 2;
  const py = (H - pillH) / 2;
  const cy = py + pillH / 2;
  let grads = "";
  let dots = "";
  seq.forEach((it, i) => {
    const cx = px + PAD + 8 + R + i * (D + GAP);
    const gid = `g${i}`;
    const s = STATES[it.state];
    if (s.glow > 0) grads += haloGradient(gid, s.color, s.glow);
    dots += dot(cx, cy, it.state, gid, it.badge);
  });
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${W} ${H}" width="${W}" height="${H}" role="img" aria-label="AgentStatus light bar with six sessions: running, blocked, done, running with two subagents, error, and idle">
    ${backdrop(W, H, true)}
    <defs>${SHADOW_DEF}${grads}</defs>
    ${pill(px, py, pillW, pillH)}
    ${dots}
  </svg>\n`;
}

// ── 2. States: every state labeled ───────────────────────────────────────────
function states() {
  const keys = ["running", "blocked", "done", "idle", "error", "unknown"];
  const rowH = 46;
  const topPad = 26;
  const W = 620;
  const H = topPad * 2 + keys.length * rowH;
  const dotX = 52;
  const labelX = 92;
  let grads = "";
  let rows = "";
  keys.forEach((k, i) => {
    const s = STATES[k];
    const cy = topPad + rowH * i + rowH / 2;
    const gid = `s${i}`;
    if (s.glow > 0) grads += haloGradient(gid, s.color, s.glow);
    rows += dot(dotX, cy, k, gid, null);
    rows += `<text x="${labelX}" y="${cy - 6}" fill="#eef2f6" font-family="-apple-system, system-ui, sans-serif" font-size="16" font-weight="600">${s.label}</text>`;
    rows += `<text x="${labelX}" y="${cy + 13}" fill="#9fb0c0" font-family="-apple-system, system-ui, sans-serif" font-size="13">${s.desc}</text>`;
  });
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${W} ${H}" width="${W}" height="${H}" role="img" aria-label="The six light states and what each means">
    ${backdrop(W, H)}
    <defs>${grads}</defs>
    ${rows}
  </svg>\n`;
}

// ── 3. Hover: one light with its badge + tooltip ─────────────────────────────
function hover() {
  const W = 680;
  const H = 220;
  const cx = 150;
  const cy = 78;
  const gid = "h0";
  const grad = haloGradient(gid, STATES.running.color, STATES.running.glow);
  // Tooltip mirrors titleFor(): "label · name (app) — state", "↳ task", subagent summary.
  const tipLines = [
    { t: "AgentStatus · agentstatus-5b (Ghostty) — running", strong: true },
    { t: "↳ Wire lightbar screenshots into the README" },
    { t: "running: 2× explore" },
  ];
  const tipX = 210;
  const tipY = 46;
  const tipW = 440;
  const tipH = 96;
  let tip = `<rect x="${tipX}" y="${tipY}" width="${tipW}" height="${tipH}" rx="9" fill="rgba(20,22,26,0.95)" stroke="${PILL_BORDER}" stroke-width="1"/>`;
  tipLines.forEach((l, i) => {
    tip += `<text x="${tipX + 16}" y="${tipY + 28 + i * 24}" fill="${l.strong ? "#eef2f6" : "#b9c6d3"}" font-family="-apple-system, system-ui, sans-serif" font-size="14" font-weight="${l.strong ? 600 : 400}">${l.t}</text>`;
  });
  // A little connector from the dot to the tooltip.
  const connector = `<line x1="${cx + 22}" y1="${cy}" x2="${tipX}" y2="${tipY + tipH / 2}" stroke="${PILL_BORDER}" stroke-width="1.5"/>`;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${W} ${H}" width="${W}" height="${H}" role="img" aria-label="A running light with a blue badge showing two subagents, and its hover tooltip listing the project, session name, application, task, and subagents">
    ${backdrop(W, H)}
    <defs>${grad}</defs>
    ${connector}
    ${dot(cx, cy, "running", gid, 2)}
    <text x="${cx}" y="${cy + 58}" fill="#9fb0c0" font-family="-apple-system, system-ui, sans-serif" font-size="13" text-anchor="middle">hover a light</text>
    ${tip}
    <text x="${tipX + tipW / 2}" y="${tipY + tipH + 26}" fill="#9fb0c0" font-family="-apple-system, system-ui, sans-serif" font-size="13" text-anchor="middle">blue badge = subagents running · click jumps to the session</text>
  </svg>\n`;
}

// A standalone bar (pill + lights) in either orientation, its top-left at (x, y).
// Returns { svg, grads, w, h } so the caller can place it and collect gradient defs.
function bar(x, y, seq, vertical, idPrefix) {
  const n = seq.length;
  const inner = n * D + (n - 1) * GAP;
  const pad = vertical ? PAD : PAD + 8; // horizontal pills-out the sides by +8
  const w = vertical ? D + 2 * PAD : inner + 2 * pad;
  const h = vertical ? inner + 2 * PAD : D + 2 * PAD;
  let grads = "";
  let dots = "";
  seq.forEach((it, i) => {
    const along = (vertical ? PAD : pad) + R + i * (D + GAP);
    const cx = vertical ? x + w / 2 : x + along;
    const cy = vertical ? y + along : y + h / 2;
    const gid = `${idPrefix}${i}`;
    const s = STATES[it.state];
    if (s.glow > 0) grads += haloGradient(gid, s.color, s.glow);
    dots += dot(cx, cy, it.state, gid, it.badge);
  });
  const svg = `${pill(x, y, w, h)}${dots}`;
  return { svg, grads, w, h };
}

// ── 4. Orientation: the same bar, horizontal and vertical ────────────────────
function orientation() {
  const seq = [{ state: "running" }, { state: "blocked" }, { state: "done" }, { state: "idle" }];
  const W = 620;
  const H = 300;
  const hb = bar(0, 0, seq, false, "oh");
  const vb = bar(0, 0, seq, true, "ov");
  // Two regions (left/right); each bar is centered in its region via a group
  // transform, with a caption below it. bar() draws from its own origin.
  const midY = 138;
  const lcx = 170;
  const rcx = 480;
  const hx = lcx - hb.w / 2;
  const hy = midY - hb.h / 2;
  const vx = rcx - vb.w / 2;
  const vy = midY - vb.h / 2;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${W} ${H}" width="${W}" height="${H}" role="img" aria-label="The same light bar shown as a horizontal row and as a vertical column; the settings panel flips between them.">
    ${backdrop(W, H)}
    <defs>${SHADOW_DEF}${hb.grads}${vb.grads}</defs>
    <g transform="translate(${hx.toFixed(1)}, ${hy.toFixed(1)})">${hb.svg}</g>
    <g transform="translate(${vx.toFixed(1)}, ${vy.toFixed(1)})">${vb.svg}</g>
    <text x="${lcx}" y="${(hy + hb.h + 30).toFixed(0)}" fill="${UI_MUTED}" font-family="-apple-system, system-ui, sans-serif" font-size="15" text-anchor="middle">Horizontal</text>
    <text x="${rcx}" y="${(vy + vb.h + 30).toFixed(0)}" fill="${UI_MUTED}" font-family="-apple-system, system-ui, sans-serif" font-size="15" text-anchor="middle">Vertical</text>
    <text x="320" y="${midY + 8}" fill="${UI_MUTED}" font-family="-apple-system, system-ui, sans-serif" font-size="28" text-anchor="middle">⇄</text>
  </svg>\n`;
}

// ── 5. Settings: the right-click panel ───────────────────────────────────────
function seg(x, y, w, labels, active) {
  const h = 26;
  const sw = w / labels.length;
  let out = `<rect x="${x}" y="${y}" width="${w}" height="${h}" rx="7" fill="none" stroke="${UI_LINE}" stroke-width="1"/>`;
  labels.forEach((lab, i) => {
    const sx = x + i * sw;
    const on = i === active;
    if (on) out += `<rect x="${sx}" y="${y}" width="${sw}" height="${h}" rx="${i === 0 || i === labels.length - 1 ? 7 : 0}" fill="${ACCENT}"/>`;
    if (i > 0) out += `<line x1="${sx}" y1="${y}" x2="${sx}" y2="${y + h}" stroke="${UI_LINE}" stroke-width="1"/>`;
    out += `<text x="${sx + sw / 2}" y="${y + h / 2 + 1}" fill="${on ? ACCENT_INK : UI_MUTED}" font-family="-apple-system, system-ui, sans-serif" font-size="12.5" font-weight="${on ? 600 : 400}" text-anchor="middle" dominant-baseline="central">${lab}</text>`;
  });
  return out;
}
function slider(x, y, w, frac) {
  const cy = y + 13;
  const tx = x + w * frac;
  return `<line x1="${x}" y1="${cy}" x2="${x + w}" y2="${cy}" stroke="${UI_LINE}" stroke-width="3" stroke-linecap="round"/>
    <line x1="${x}" y1="${cy}" x2="${tx}" y2="${cy}" stroke="${ACCENT}" stroke-width="3" stroke-linecap="round"/>
    <circle cx="${tx}" cy="${cy}" r="7" fill="${ACCENT}"/>`;
}
// A checkbox (per-state chime toggle): accent-filled with a check when on.
function checkbox(x, y, on) {
  const s = 15;
  let out = `<rect x="${x}" y="${y}" width="${s}" height="${s}" rx="4" fill="${on ? ACCENT : "none"}" stroke="${UI_LINE}" stroke-width="1"/>`;
  if (on) out += `<path d="M ${x + 3.5} ${y + 7.8} L ${x + 6.2} ${y + 10.6} L ${x + 11.5} ${y + 4.4}" fill="none" stroke="${ACCENT_INK}" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>`;
  return out;
}
function settings() {
  const PW = 320;
  const P = 20; // panel padding
  const x0 = P;
  const x1 = PW - P; // right edge of content
  const ctrlW = 150; // segmented-control / slider width, right-aligned to x1
  const cx = x1 - ctrlW;
  const rows = [];
  let y = P;
  // Lights row at the top of the panel (settings grows below the bar).
  const lights = [{ state: "running" }, { state: "blocked" }, { state: "done" }, { state: "idle" }];
  let grads = "";
  let head = "";
  const lgap = 22;
  const lw = lights.length * D + (lights.length - 1) * lgap;
  lights.forEach((it, i) => {
    const dcx = PW / 2 - lw / 2 + R + i * (D + lgap);
    const gid = `pl${i}`;
    const s = STATES[it.state];
    if (s.glow > 0) grads += haloGradient(gid, s.color, s.glow);
    head += dot(dcx, y + R, it.state, gid, null);
  });
  y += D + 7;
  // Close buttons (decision 080): one red x under each light, revealed with the panel.
  // Fixed red — deliberately not the Error state color, which the user can change.
  const CLOSE_BG = "#db4241"; // oklch(60% 0.19 25)
  const CLOSE_INK = "#fff6f5"; // oklch(98% 0.01 25)
  lights.forEach((_, i) => {
    const dcx = PW / 2 - lw / 2 + R + i * (D + lgap);
    const k = R * 0.42;
    head += `<g opacity="0.62"><circle cx="${dcx}" cy="${y + R}" r="${R}" fill="${CLOSE_BG}"/>` +
      `<path d="M ${dcx - k} ${y + R - k} L ${dcx + k} ${y + R + k} M ${dcx + k} ${y + R - k} L ${dcx - k} ${y + R + k}" stroke="${CLOSE_INK}" stroke-width="2.2" stroke-linecap="round"/></g>`;
  });
  y += D + 16;
  const sep = () => { const s = `<line x1="${x0}" y1="${y}" x2="${x1}" y2="${y}" stroke="${UI_LINE}" stroke-width="1"/>`; y += 16; return s; };
  const label = (t) => `<text x="${x0}" y="${y + 13}" fill="${UI_MUTED}" font-family="-apple-system, system-ui, sans-serif" font-size="12.5" dominant-baseline="central">${t}</text>`;
  let body = sep();
  const segRow = (t, labels, active) => { const r = label(t) + seg(cx, y, ctrlW, labels, active); y += 38; return r; };
  const sliderRow = (t, frac) => { const r = label(t) + slider(cx, y, ctrlW, frac); y += 34; return r; };
  body += segRow("Orientation", ["Horizontal", "Vertical"], 0);
  body += segRow("Sort", ["Stable", "Urgency"], 0);
  body += segRow("Unknown", ["Show", "Hide"], 0);
  body += sliderRow("Size", 0.4);
  body += sliderRow("Padding", 0.55);
  body += sliderRow("Opacity", 0.82);
  body += sep();
  // Colors: 2-column grid of state swatches.
  const colStates = [["running", "Running"], ["blocked", "Blocked"], ["done", "Done"], ["idle", "Idle"], ["error", "Error"]];
  const colW = (x1 - x0) / 2;
  colStates.forEach(([st, name], i) => {
    const col = i % 2;
    const rowi = Math.floor(i / 2);
    const rx = x0 + col * colW;
    const ry = y + rowi * 26;
    body += `<text x="${rx}" y="${ry + 10}" fill="${UI_MUTED}" font-family="-apple-system, system-ui, sans-serif" font-size="12.5" dominant-baseline="central">${name}</text>`;
    body += `<rect x="${rx + colW - 34}" y="${ry + 2}" width="22" height="16" rx="4" fill="${STATES[st].color}" stroke="${UI_LINE}" stroke-width="1"/>`;
  });
  y += 3 * 26 + 6;
  body += sep();
  // Audio: master On/Off toggle + the revealed sub-panel (per-state chimes + volume).
  const checkRow = (t, on) => { const r = label(t) + checkbox(x1 - 15, y, on); y += 24; return r; };
  body += segRow("Audio", ["Off", "On"], 1);
  body += checkRow("Blocked chime", true);
  body += checkRow("Error chime", true);
  body += checkRow("Done chime", true);
  body += sliderRow("Volume", 0.6);
  body += sep();
  // Footer links.
  const footer = `<text x="${x0}" y="${y + 10}" fill="${UI_MUTED}" font-family="-apple-system, system-ui, sans-serif" font-size="12.5" text-decoration="underline" dominant-baseline="central">Reload</text>
    <text x="${PW / 2}" y="${y + 10}" fill="${UI_MUTED}" font-family="-apple-system, system-ui, sans-serif" font-size="12.5" text-decoration="underline" text-anchor="middle" dominant-baseline="central">Reset to defaults</text>
    <text x="${x1}" y="${y + 10}" fill="#e8807f" font-family="-apple-system, system-ui, sans-serif" font-size="12.5" text-decoration="underline" text-anchor="end" dominant-baseline="central">Quit</text>`;
  y += 22;
  const PH = y + P - 10;
  const W = 620;
  const H = PH + 48;
  const px = (W - PW) / 2;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${W} ${H}" width="${W}" height="${H}" role="img" aria-label="The AgentStatus settings panel, opened by right-clicking the bar: the lights with a row of red close buttons beneath them, orientation and sort toggles, an Unknown Show/Hide toggle, size/padding/opacity sliders, per-state color swatches, an Audio On/Off toggle with per-state chime checkboxes (Blocked, Error, Done) and a volume slider, and reload/reset/quit links.">
    ${backdrop(W, H)}
    <defs>${grads}</defs>
    <g transform="translate(${px}, 24)">
      <rect x="0" y="0" width="${PW}" height="${PH}" rx="15" fill="${PANEL_BG}" stroke="${UI_LINE}" stroke-width="1"/>
      ${head}${body}${footer}
    </g>
  </svg>\n`;
}

writeFileSync(join(DIR, "lightbar-hero.svg"), hero());
writeFileSync(join(DIR, "lightbar-states.svg"), states());
writeFileSync(join(DIR, "lightbar-hover.svg"), hover());
writeFileSync(join(DIR, "lightbar-orientation.svg"), orientation());
writeFileSync(join(DIR, "lightbar-settings.svg"), settings());
console.log("Wrote docs/lightbar-{hero,states,hover,orientation,settings}.svg");
