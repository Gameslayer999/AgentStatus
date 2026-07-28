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
//   docs/lightbar-hero.svg    a realistic bar, mixed states — the hero
//   docs/lightbar-states.svg  every state labeled with its meaning
//   docs/lightbar-hover.svg   one light with its badge + hover tooltip
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
};

// Frosted pill: rgba(20,22,26,.82) fill, rgba(255,255,255,.09) hairline border.
const PILL_FILL = "rgba(20,22,26,0.82)";
const PILL_BORDER = "rgba(255,255,255,0.09)";
// Subagent badge — --ui-accent oklch(60% 0.15 262) → sRGB, and its near-white ink.
const ACCENT = "#4c7dd9";
const ACCENT_INK = "#f4f7fe";

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
  out += `<circle cx="${cx}" cy="${cy}" r="${R}" fill="${s.color}"${op}/>`;
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
  const rx = h / 2;
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
  const keys = ["running", "blocked", "done", "idle", "error"];
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
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${W} ${H}" width="${W}" height="${H}" role="img" aria-label="The five light states and what each means">
    ${backdrop(W, H)}
    <defs>${grads}</defs>
    ${rows}
  </svg>\n`;
}

// ── 3. Hover: one light with its badge + tooltip ─────────────────────────────
function hover() {
  const W = 620;
  const H = 220;
  const cx = 150;
  const cy = 78;
  const gid = "h0";
  const grad = haloGradient(gid, STATES.running.color, STATES.running.glow);
  // Tooltip mirrors titleFor(): "label — state", "↳ task", subagent summary.
  const tipLines = [
    { t: "AgentStatus — running", strong: true },
    { t: "↳ Wire lightbar screenshots into the README" },
    { t: "running: 2× explore" },
  ];
  const tipX = 210;
  const tipY = 46;
  const tipW = 360;
  const tipH = 96;
  let tip = `<rect x="${tipX}" y="${tipY}" width="${tipW}" height="${tipH}" rx="9" fill="rgba(20,22,26,0.95)" stroke="${PILL_BORDER}" stroke-width="1"/>`;
  tipLines.forEach((l, i) => {
    tip += `<text x="${tipX + 16}" y="${tipY + 28 + i * 24}" fill="${l.strong ? "#eef2f6" : "#b9c6d3"}" font-family="-apple-system, system-ui, sans-serif" font-size="14" font-weight="${l.strong ? 600 : 400}">${l.t}</text>`;
  });
  // A little connector from the dot to the tooltip.
  const connector = `<line x1="${cx + 22}" y1="${cy}" x2="${tipX}" y2="${tipY + tipH / 2}" stroke="${PILL_BORDER}" stroke-width="1.5"/>`;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${W} ${H}" width="${W}" height="${H}" role="img" aria-label="A running light with a blue badge showing two subagents, and its hover tooltip listing the project, task, and subagents">
    ${backdrop(W, H)}
    <defs>${grad}</defs>
    ${connector}
    ${dot(cx, cy, "running", gid, 2)}
    <text x="${cx}" y="${cy + 58}" fill="#9fb0c0" font-family="-apple-system, system-ui, sans-serif" font-size="13" text-anchor="middle">hover a light</text>
    ${tip}
    <text x="${tipX + tipW / 2}" y="${tipY + tipH + 26}" fill="#9fb0c0" font-family="-apple-system, system-ui, sans-serif" font-size="13" text-anchor="middle">blue badge = subagents running · click jumps to the session</text>
  </svg>\n`;
}

writeFileSync(join(DIR, "lightbar-hero.svg"), hero());
writeFileSync(join(DIR, "lightbar-states.svg"), states());
writeFileSync(join(DIR, "lightbar-hover.svg"), hover());
console.log("Wrote docs/lightbar-hero.svg, docs/lightbar-states.svg, docs/lightbar-hover.svg");
