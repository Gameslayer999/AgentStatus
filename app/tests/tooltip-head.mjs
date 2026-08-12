// Check the tooltip's identifying line — decision 053.
//
//   node app/tests/tooltip-head.mjs
//
// Evals the REAL functions out of app/src/main.js (same trick as unread-light.mjs).
import { readFileSync } from "node:fs";
import assert from "node:assert/strict";

const src = readFileSync(new URL("../src/main.js", import.meta.url), "utf8");
const grab = (re) => {
  const m = src.match(re);
  if (!m) throw new Error(`not found: ${re}`);
  return m[0];
};
const code = [
  grab(/function shortId\(id\) \{[\s\S]*?\n\}/),
  grab(/function headFor\(s\) \{[\s\S]*?\n\}/),
  "globalThis.headFor = headFor;",
].join("\n");
new Function(code)();

const id = "38d52eeb-cc17-40e1-88a1-b2c01a8b928e";

// 1. Folder + the host's session name — what tells two sessions in one folder apart.
assert.equal(
  headFor({ id, label: "AgentStatus", name: "agentstatus-5b" }),
  "AgentStatus · agentstatus-5b"
);

// 2. No name reported (no session record, or a host that has none) → folder only.
assert.equal(headFor({ id, label: "AgentStatus", name: "" }), "AgentStatus");
assert.equal(headFor({ id, label: "AgentStatus" }), "AgentStatus");

// 3. A name that repeats the folder adds nothing — don't print it twice.
assert.equal(headFor({ id, label: "AgentStatus", name: "agentstatus" }), "AgentStatus");

// 4. No folder (an anonymous session) → the name stands alone.
assert.equal(headFor({ id, label: "", name: "Fix the parser" }), "Fix the parser");

// 5. Neither → the short id, as before.
assert.equal(headFor({ id, label: "", name: "" }), "38d52eeb");

console.log("all checks passed");
