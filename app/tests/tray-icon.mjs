// What the menu-bar icon draws: a silhouette per state, and an outline that keeps a pale
// light visible on a light menu bar — decision 086.
//
//   node app/tests/tray-icon.mjs
//
// Evals the REAL drawing helpers out of app/src/main.js (which can't be imported: it pulls
// in the Tauri APIs at load), so this tests the shipped source, not a copy of it.
import { readFileSync } from "node:fs";
import assert from "node:assert/strict";

const src = readFileSync(new URL("../src/main.js", import.meta.url), "utf8");
const grab = (re) => {
  const m = src.match(re);
  if (!m) throw new Error(`not found: ${re}`);
  return m[0];
};
const code = [
  grab(/const TRAY_SHAPE = .*/),
  grab(/function traceLight\(ctx, shape, cx, cy, r\) \{[\s\S]*?\n\}/),
  grab(/function trayOutline\(hex\) \{[\s\S]*?\n\}/),
  "globalThis.api = { TRAY_SHAPE, traceLight, trayOutline };",
].join("\n");

// The appearance the helpers read. Set per case, since that is the whole point of the
// outline: the same color needs a different answer on a light and a dark menu bar.
let dark = true;
globalThis.window = { matchMedia: () => ({ matches: dark }) };
new Function(code)();
const { TRAY_SHAPE, traceLight, trayOutline } = globalThis.api;

// A fake 2D context that records the path calls instead of rasterizing them.
const trace = (shape) => {
  const calls = [];
  const ctx = new Proxy(
    {},
    { get: (_t, name) => (...args) => calls.push([name, ...args]) },
  );
  traceLight(ctx, shape, 20, 22, 11);
  return calls;
};

// --- one silhouette per state -------------------------------------------------------
assert.equal(TRAY_SHAPE.blocked, "triangle", "blocked must not be just another circle");
assert.equal(TRAY_SHAPE.error, "square", "error must not be just another circle");
assert.equal(TRAY_SHAPE.unknown, "ring");
for (const st of ["running", "done", "idle", "empty"]) {
  assert.equal(TRAY_SHAPE[st], undefined, `${st} stays a plain circle`);
}

const circle = trace("circle");
assert.deepEqual(circle, [["arc", 20, 22, 11, 0, Math.PI * 2]], "running draws a full-radius dot");

const triangle = trace("triangle");
assert.equal(triangle[0][0], "moveTo");
assert.equal(triangle.filter((c) => c[0] === "lineTo").length, 2, "a triangle has three corners");
assert.equal(triangle.at(-1)[0], "closePath");
assert.ok(triangle[0][2] < 22, "the apex points up");

const square = trace("square");
assert.equal(square.length, 1);
assert.equal(square[0][0], "rect");
assert.equal(square[0][3], square[0][4], "the error mark is square, not a bar");

const ring = trace("ring");
assert.equal(ring[0][0], "arc");
assert.ok(ring[0][3] < 11, "the ring's stroke stays inside the dot's diameter");

// --- visible in both appearances ----------------------------------------------------
dark = false; // a light menu bar
assert.ok(trayOutline("#ecf0f1"), "done (near white) needs an outline on a light bar");
assert.ok(trayOutline("#ffffff"), "the empty placeholder needs one too");
assert.equal(trayOutline("#2ecc71"), null, "running already contrasts; leave its color alone");
assert.equal(trayOutline("#e74c3c"), null, "so does error");
assert.equal(trayOutline("#111111"), null, "a dark light is fine on a light bar");

dark = true; // a dark menu bar
assert.equal(trayOutline("#ecf0f1"), null, "white needs nothing on a dark bar");
assert.ok(trayOutline("#0a0a0a"), "a near-black color the user picked needs one here");

// A color the picker could never produce must not throw or paint a stray ring.
assert.equal(trayOutline("not a color"), null);
assert.equal(trayOutline(undefined), null);

console.log("tray-icon: ok");
