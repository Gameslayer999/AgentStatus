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
//   docs/lightbar-controls.svg     the close buttons + gear a right-click reveals
//   docs/lightbar-settings.svg     the settings window the gear opens
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
  // styles.css uses `--pill-radius`: half the thickness of the light strip itself. On a
  // one-row bar that is exactly half the smaller side (a stadium with rounded end-caps);
  // when the controls row makes the bar two rows thick, the corners stay put instead of
  // inflating into an oval (decision 082).
  const rx = Math.min(Math.min(w, h) / 2, (D + 2 * PAD) / 2);
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

// ── 5. Controls: what a right-click on the bar reveals ──────────────────────
// One close button per light plus the settings gear, on the row that appears beside the
// lights (decisions 080/082). Drawn at the same scale as the other bar art.
const CLOSE_BG = "#db4241"; // fixed red — oklch(60% 0.19 25), never the Error color
const CLOSE_INK = "#fff6f5"; // oklch(98% 0.01 25)

function closeButton(cx, cy) {
  const k = R * 0.42;
  return `<g opacity="0.62"><circle cx="${cx}" cy="${cy}" r="${R}" fill="${CLOSE_BG}"/>` +
    `<path d="M ${cx - k} ${cy - k} L ${cx + k} ${cy + k} M ${cx + k} ${cy - k} L ${cx - k} ${cy + k}" stroke="${CLOSE_INK}" stroke-width="2.2" stroke-linecap="round"/></g>`;
}

// The gear, drawn exactly as main.js draws it: eight teeth around a ring, in a 24-unit
// box scaled to 1.7 lights across, so the icon overflows its slot the way it does live.
function gearIcon(cx, cy, ink = "rgba(255,255,255,0.88)") {
  const k = (D * 1.7) / 24;
  const teeth = [0, 45, 90, 135, 180, 225, 270, 315]
    .map((a) => `<rect x="-2.1" y="-11.2" width="4.2" height="5.4" rx="1.1" transform="rotate(${a})"/>`)
    .join("");
  return `<g transform="translate(${cx}, ${cy}) scale(${k.toFixed(3)})" fill="${ink}" opacity="0.78">
      ${teeth}<circle r="6.1" fill="none" stroke="${ink}" stroke-width="3.4"/>
    </g>`;
}

function controls() {
  const seq = [{ state: "running" }, { state: "blocked" }, { state: "done" }, { state: "idle" }];
  const ROWGAP = 16; // #strip's 7px gap at this art's scale
  const pad = PAD + 8;
  // The controls row is one slot longer than the lights (the gear sits past the end), and
  // the pill grows to the longer row — the lights themselves never move.
  const inner = (seq.length + 1) * D + seq.length * GAP;
  const w = inner + 2 * pad;
  const h = 2 * D + ROWGAP + 2 * PAD;
  const W = 620;
  const H = 260;
  const x = (W - w) / 2;
  const y = (H - h) / 2 - 10;
  let grads = "";
  let dots = "";
  let ctrls = "";
  seq.forEach((it, i) => {
    const cx = x + pad + R + i * (D + GAP);
    const gid = `cb${i}`;
    const st = STATES[it.state];
    if (st.glow > 0) grads += haloGradient(gid, st.color, st.glow);
    dots += dot(cx, y + PAD + R, it.state, gid, null);
    ctrls += closeButton(cx, y + PAD + D + ROWGAP + R);
  });
  ctrls += gearIcon(x + pad + R + seq.length * (D + GAP), y + PAD + D + ROWGAP + R);
  const capY = y + h + 34;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${W} ${H}" width="${W}" height="${H}" role="img" aria-label="The light bar after a right-click: a row of four lights with a red close button under each one, and a settings gear one slot past the last close button.">
    ${backdrop(W, H)}
    <defs>${SHADOW_DEF}${grads}</defs>
    ${pill(x, y, w, h)}${dots}${ctrls}
    <text x="${W / 2}" y="${capY}" fill="${UI_MUTED}" font-family="-apple-system, system-ui, sans-serif" font-size="15" text-anchor="middle">Right-click: one close button per light, and the settings gear</text>
  </svg>\n`;
}

// ── 6. Settings: the window the gear opens ───────────────────────────────────
// A normal, decorated window that follows the system appearance (decision 082), so this
// one picture is drawn light — unlike the bar art, it is not part of the overlay.
const WIN_BG = "#ffffff";
const WIN_BAR = "#ececee";
const WIN_SIDE = "#f2f2f4";
const WIN_INK = "#1d1d1f";
const WIN_MUTED = "#6c6c70";
const WIN_LINE = "rgba(0,0,0,0.12)";
const WIN_ACCENT = "#0a6cff"; // the stock macOS accent; the real window uses AccentColor

function lseg(x, y, w, labels, active) {
  const h = 24;
  const sw = w / labels.length;
  let out = `<rect x="${x}" y="${y}" width="${w}" height="${h}" rx="7" fill="none" stroke="${WIN_LINE}" stroke-width="1"/>`;
  labels.forEach((lab, i) => {
    const sx = x + i * sw;
    const on = i === active;
    if (on) out += `<rect x="${sx}" y="${y}" width="${sw}" height="${h}" rx="7" fill="${WIN_ACCENT}"/>`;
    if (i > 0) out += `<line x1="${sx}" y1="${y}" x2="${sx}" y2="${y + h}" stroke="${WIN_LINE}" stroke-width="1"/>`;
    out += `<text x="${sx + sw / 2}" y="${y + h / 2 + 1}" fill="${on ? "#ffffff" : WIN_INK}" font-family="-apple-system, system-ui, sans-serif" font-size="12" text-anchor="middle" dominant-baseline="central">${lab}</text>`;
  });
  return out;
}

function lslider(x, y, w, frac) {
  const cy = y + 12;
  const tx = x + w * frac;
  return `<line x1="${x}" y1="${cy}" x2="${x + w}" y2="${cy}" stroke="${WIN_LINE}" stroke-width="3" stroke-linecap="round"/>
    <line x1="${x}" y1="${cy}" x2="${tx}" y2="${cy}" stroke="${WIN_ACCENT}" stroke-width="3" stroke-linecap="round"/>
    <circle cx="${tx}" cy="${cy}" r="6.5" fill="#ffffff" stroke="rgba(0,0,0,0.22)" stroke-width="1"/>`;
}

function settings() {
  const PW = 560;
  const PH = 430; // the window's real size
  const TITLE = 28;
  const RAIL = 148;
  const W = 620;
  const H = PH + 52;
  const px = (W - PW) / 2;
  const win = (t) => `<text font-family="-apple-system, system-ui, sans-serif" ${t}`;

  // Sidebar.
  const panes = ["General", "Lights", "Colors", "Audio", "About"];
  let rail = `<rect x="0" y="${TITLE}" width="${RAIL}" height="${PH - TITLE}" fill="${WIN_SIDE}"/>
    <line x1="${RAIL}" y1="${TITLE}" x2="${RAIL}" y2="${PH}" stroke="${WIN_LINE}" stroke-width="1"/>`;
  panes.forEach((name, i) => {
    const y = TITLE + 12 + i * 28;
    const on = i === 0;
    if (on) rail += `<rect x="8" y="${y}" width="${RAIL - 16}" height="26" rx="6" fill="${WIN_ACCENT}"/>`;
    rail += win(`x="18" y="${y + 13}" fill="${on ? "#ffffff" : WIN_INK}" font-size="12.5" dominant-baseline="central">${name}</text>`);
  });

  // The General pane.
  const cx0 = RAIL + 18;
  const cx1 = PW - 18;
  const ctrlW = 170;
  let y = TITLE + 16;
  let body = "";
  const row = (label, control) => {
    body += win(`x="${cx0}" y="${y + 12}" fill="${WIN_MUTED}" font-size="12.5" dominant-baseline="central">${label}</text>`) + control;
    y += 34;
  };
  row("Mode", lseg(cx1 - ctrlW, y, ctrlW, ["Floating", "Menu bar"], 0));
  row("Orientation", lseg(cx1 - ctrlW, y, ctrlW, ["Horizontal", "Vertical"], 0));
  row("Light size", lslider(cx1 - 150, y, 150, 0.4));
  row("Padding", lslider(cx1 - 150, y, 150, 0.55));
  row("Opacity", lslider(cx1 - 150, y, 150, 0.82));
  y += 6;
  body += win(`x="${cx0}" y="${y + 10}" fill="${WIN_MUTED}" font-size="11">Right-click the bar to reveal its controls: a close</text>`);
  body += win(`x="${cx0}" y="${y + 25}" fill="${WIN_MUTED}" font-size="11">button for each light, and the gear that opens this window.</text>`);

  // Footer.
  const fy = PH - 40;
  let footer = `<line x1="${RAIL}" y1="${fy}" x2="${PW}" y2="${fy}" stroke="${WIN_LINE}" stroke-width="1"/>`;
  const btns = [["Reload", 62], ["Reset to defaults", 118], ["Quit AgentStatus", 116]];
  let bx = cx1;
  for (const [label, bw] of btns.reverse()) {
    bx -= bw;
    footer += `<rect x="${bx}" y="${fy + 8}" width="${bw}" height="24" rx="6" fill="none" stroke="${WIN_LINE}" stroke-width="1"/>`;
    footer += win(`x="${bx + bw / 2}" y="${fy + 21}" fill="${WIN_INK}" font-size="12" text-anchor="middle" dominant-baseline="central">${label}</text>`);
    bx -= 8;
  }

  const lights = ["#ff5f57", "#febc2e", "#28c840"]
    .map((c, i) => `<circle cx="${16 + i * 18}" cy="${TITLE / 2}" r="6" fill="${c}"/>`)
    .join("");
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${W} ${H}" width="${W}" height="${H}" role="img" aria-label="The AgentStatus settings window: a sidebar listing General, Lights, Colors, Audio and About, with the General pane showing Mode and Orientation toggles and sliders for light size, padding and opacity, and Reload, Reset to defaults and Quit AgentStatus buttons along the bottom.">
    ${backdrop(W, H)}
    <defs>${SHADOW_DEF}</defs>
    <clipPath id="winclip"><rect x="0" y="0" width="${PW}" height="${PH}" rx="10"/></clipPath>
    <g transform="translate(${px}, 26)" filter="url(#softshadow)">
      <g clip-path="url(#winclip)">
      <rect x="0" y="0" width="${PW}" height="${PH}" rx="10" fill="${WIN_BG}"/>
      <path d="M 0 10 a 10 10 0 0 1 10 -10 h ${PW - 20} a 10 10 0 0 1 10 10 v ${TITLE - 10} h -${PW} Z" fill="${WIN_BAR}"/>
      <line x1="0" y1="${TITLE}" x2="${PW}" y2="${TITLE}" stroke="${WIN_LINE}" stroke-width="1"/>
      ${lights}
      ${win(`x="${PW / 2}" y="${TITLE / 2 + 1}" fill="#3c3c43" font-size="12.5" font-weight="600" text-anchor="middle" dominant-baseline="central">AgentStatus Settings</text>`)}
      ${rail}${body}${footer}
      </g>
      <rect x="0.5" y="0.5" width="${PW - 1}" height="${PH - 1}" rx="10" fill="none" stroke="${WIN_LINE}" stroke-width="1"/>
    </g>
  </svg>\n`;
}

writeFileSync(join(DIR, "lightbar-hero.svg"), hero());
writeFileSync(join(DIR, "lightbar-states.svg"), states());
writeFileSync(join(DIR, "lightbar-hover.svg"), hover());
writeFileSync(join(DIR, "lightbar-orientation.svg"), orientation());
writeFileSync(join(DIR, "lightbar-controls.svg"), controls());
writeFileSync(join(DIR, "lightbar-settings.svg"), settings());
console.log("Wrote docs/lightbar-{hero,states,hover,orientation,controls,settings}.svg");
