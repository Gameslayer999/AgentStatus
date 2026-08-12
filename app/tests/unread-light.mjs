// Lifecycle check for the Cursor "unread" (done) light — decision 050.
//
//   node app/tests/unread-light.mjs
//
// Evals the REAL functions out of app/src/main.js (which can't be imported: it pulls in
// the Tauri APIs at load) so this tests the shipped source, not a copy of it.
import { readFileSync } from "node:fs";
import assert from "node:assert/strict";

const src = readFileSync(new URL("../src/main.js", import.meta.url), "utf8");
const grab = (re) => {
  const m = src.match(re);
  if (!m) throw new Error(`not found: ${re}`);
  return m[0];
};
const code = [
  grab(/function isFinishedTurn\(s\) \{[\s\S]*?\n\}/),
  grab(/const finishedAt = new Map\(\);[\s\S]*?\nfunction noteFinishes\(sessions\) \{[\s\S]*?\n\}/),
  "globalThis.isFinishedTurn = isFinishedTurn; globalThis.noteFinishes = noteFinishes; globalThis.finishedAt = finishedAt;",
].join("\n");
new Function(code)();

const cursor = (state, updated_at) => ({ id: "c1", ide: "cursor", state, updated_at, detail: "" });
const claude = (state, updated_at, detail) => ({ id: "v1", ide: "vscode", state, updated_at, detail });

// 1. A Cursor session already idle when the bar starts is not a fresh finish.
let s = cursor("idle", 100);
noteFinishes([s]);
assert.equal(isFinishedTurn(s), false, "pre-existing idle must not read as finished");

// 2. running -> idle is a finish, and stays one across later polls.
s = cursor("running", 110);
noteFinishes([s]);
assert.equal(isFinishedTurn(s), false, "running is not finished");
s = cursor("idle", 120);
noteFinishes([s]);
assert.equal(isFinishedTurn(s), true, "the finish poll lights up");
noteFinishes([s]);
noteFinishes([s]);
assert.equal(isFinishedTurn(s), true, "still unread on later polls");

// 3. Acknowledged (the click keys reviewedAt off updated_at) then a NEW turn re-lights.
//    displayState does the reviewedAt half; here we check the finish key advances.
s = cursor("running", 130);
noteFinishes([s]);
const s2 = cursor("idle", 140);
noteFinishes([s2]);
assert.equal(finishedAt.get("c1"), 140, "finish key follows the newest turn");

// 4. A vanished session drops its memory (no leak, no stale unread on reuse).
noteFinishes([]);
assert.equal(finishedAt.has("c1"), false, "finish memory cleaned up");

// 5. Claude Code is untouched: still driven by detail, never by transitions.
let v = claude("running", 200, "");
noteFinishes([v]);
v = claude("idle", 210, "");
noteFinishes([v]);
assert.equal(isFinishedTurn(v), false, "SessionStart-style idle (no detail) is not finished");
assert.equal(isFinishedTurn(claude("idle", 220, "wrote the file")), true, "detail still means finished");
assert.equal(finishedAt.size, 0, "no transition memory kept for non-Cursor sessions");

console.log("all checks passed");
