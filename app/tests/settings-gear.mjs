// The settings gear in the bar's controls row — decision 082.
//
//   node app/tests/settings-gear.mjs
//
// Evals the REAL `ensureGear` out of app/src/main.js (which can't be imported: it pulls
// in the Tauri APIs at load) against a minimal DOM, so this tests the shipped source.
// What it pins down is the wiring a live click can't be scripted onto reliably: the gear
// is built once, always ends up last in the row, and clicking it invokes `open_settings`.
import { readFileSync } from "node:fs";
import assert from "node:assert/strict";

const src = readFileSync(new URL("../src/main.js", import.meta.url), "utf8");
const grab = (re) => {
  const m = src.match(re);
  if (!m) throw new Error(`not found: ${re}`);
  return m[0];
};

// A DOM with just enough of an element to build and place one icon.
function makeEl() {
  const el = {
    className: "",
    innerHTML: "",
    title: "",
    children: [],
    listeners: {},
    addEventListener(type, fn) {
      (this.listeners[type] ||= []).push(fn);
    },
    appendChild(child) {
      this.children = this.children.filter((c) => c !== child);
      this.children.push(child);
      return child;
    },
    get lastElementChild() {
      return this.children[this.children.length - 1] ?? null;
    },
  };
  return el;
}

const invoked = [];
const code = [
  "const document = { createElement: () => globalThis.makeEl() };",
  "const invoke = (cmd) => { globalThis.invoked.push(cmd); return Promise.resolve(); };",
  "function suppressHover() {}",
  grab(/let gearEl = null;[\s\S]*?\nfunction ensureGear\(closerRow\) \{[\s\S]*?\n\}/),
  "globalThis.api = { ensureGear, gear: () => gearEl };",
].join("\n");
globalThis.makeEl = makeEl;
globalThis.invoked = invoked;
new Function(code)();
const { ensureGear, gear } = globalThis.api;

const row = makeEl();

// 1. The row gets exactly one gear, and it is the gear element.
ensureGear(row);
assert.equal(row.children.length, 1, "one gear added");
assert.equal(row.children[0].className, "gear");
assert.equal(row.children[0], gear());

// 2. Re-rendering (every poll) reuses it rather than stacking copies.
ensureGear(row);
ensureGear(row);
assert.equal(row.children.length, 1, "the gear is built once, not once per poll");

// 3. It is drawn, not typed — a text gear renders as a color emoji in some system
//    fonts, which is why this is an SVG (decision 082).
assert.match(gear().innerHTML, /^<svg /, "the icon is an inline SVG");
assert.equal((gear().innerHTML.match(/<rect /g) || []).length, 8, "eight teeth");
assert.match(gear().innerHTML, /<circle /, "and a ring");

// 4. It always ends up last, however many close buttons arrived before it — the close
//    buttons are aligned one-for-one with the lights, so the gear may only ever sit one
//    slot past the end of them (decision 080).
const x1 = makeEl();
const x2 = makeEl();
row.children = [x1, x2];
ensureGear(row);
assert.deepEqual(row.children, [x1, x2, gear()], "gear last, close buttons undisturbed");

// 5. Clicking it opens the settings window — the whole point of the icon.
assert.equal(invoked.length, 0);
for (const fn of gear().listeners.click) fn();
assert.deepEqual(invoked, ["open_settings"], "a click invokes open_settings");

console.log("all checks passed");
