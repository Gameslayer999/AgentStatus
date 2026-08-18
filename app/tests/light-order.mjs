// Check that a light keeps its slot — decision 062.
//
//   node app/tests/light-order.mjs
//
// Evals the REAL ordering functions out of app/src/main.js (same trick as
// unread-light.mjs), so this tests the shipped source rather than a copy of it.
import { readFileSync } from "node:fs";
import assert from "node:assert/strict";

// Preferences live in prefs.js (shared with the settings window, decision 082) and
// the logic that reads them in main.js, so both are searched.
const src =
  readFileSync(new URL("../src/prefs.js", import.meta.url), "utf8") +
  readFileSync(new URL("../src/main.js", import.meta.url), "utf8");
const grab = (re) => {
  const m = src.match(re);
  if (!m) throw new Error(`not found: ${re}`);
  return m[0];
};
const code = [
  "const localStorage = globalThis.fakeStorage;",
  grab(/const SORT_KEY = .*/),
  grab(/function currentSort\(\) \{[\s\S]*?\n\}/),
  grab(/const URGENCY_RANK = .*/),
  grab(/const arrivalSeq = new Map\(\);[\s\S]*?\nfunction byArrival\(a, b\) \{[\s\S]*?\n\}/),
  grab(/function sortSessions\(sessions\) \{[\s\S]*?\n\}/),
  // The real displayState pulls in reviewedAt/isUnobservable/isFinishedTurn; urgency
  // ordering only needs the state it returns, so stub it at the same contract.
  "function displayState(s) { return s.state; }",
  "globalThis.sortSessions = sortSessions; globalThis.arrivalSeq = arrivalSeq;",
].join("\n");

const store = new Map();
globalThis.fakeStorage = {
  getItem: (k) => (store.has(k) ? store.get(k) : null),
  setItem: (k, v) => store.set(k, v),
};
new Function(code)();

const ids = (arr) => arr.map((s) => s.id).join(",");
// cwd deliberately differs from arrival order: the pre-062 sort keyed on it, so a test
// that agreed with cwd could not tell the two orderings apart.
const s = (id, cwd, state = "idle") => ({ id, cwd, state });
const a = s("a", "/z/repo");
const b = s("b", "/a/repo");
const c = s("c", "/m/repo");

// 1. First poll: the backend's order is the layout, whatever the ids and folders are.
assert.equal(ids(sortSessions([a, b, c])), "a,b,c");

// 2. A new session appends — it never lands in the middle, even though its id sorts
//    first and its folder groups it with an existing light.
const d = s("aa", "/a/repo");
assert.equal(ids(sortSessions([a, b, c, d])), "a,b,c,aa");

// 3. The backend re-orders (it sorts by label, which changes when a session cd's, and
//    a status file can be read in any order) — the bar does not.
assert.equal(ids(sortSessions([d, c, b, a])), "a,b,c,aa");

// 4. A light that goes away closes its gap; the rest hold their order.
assert.equal(ids(sortSessions([a, c, d])), "a,c,aa");

// 5. A session that comes back is new — it takes the next free slot, not its old one.
assert.equal(ids(sortSessions([a, c, d, b])), "a,c,aa,b");

// 6. A slot is released when its session goes, so the map can't grow without bound.
sortSessions([a]);
assert.equal(arrivalSeq.size, 1);

// 7. Urgency mode still leads with the attention states, and ties keep arrival order.
store.set("agentstatus.sort", "urgency");
// `a` is the only light still holding a slot here (6 pruned the rest), so `b` and `c`
// arrive in this call's order — which is what the tie between the two idles reads.
const order = ids(
  sortSessions([
    s("a", "/z/repo", "idle"),
    s("b", "/a/repo", "idle"),
    s("c", "/m/repo", "error"),
  ])
);
assert.equal(order, "c,a,b", "attention first, then the two idles in arrival order");

console.log("all checks passed");
