// Manual prune: what a dismissed light hides, and when it comes back — decision 080.
//
//   node app/tests/dismiss-light.mjs
//
// Evals the REAL functions out of app/src/main.js (which can't be imported: it pulls in
// the Tauri APIs at load) so this tests the shipped source, not a copy of it.
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
  grab(/const UNKNOWN_KEY = .*/),
  grab(/function showUnknown\(\) \{[\s\S]*?\n\}/),
  grab(/function visibleSessions\(\) \{[\s\S]*?\n\}/),
  grab(/const DISMISS_GRACE_MS = [\s\S]*?\nfunction reapDismissed\(sessions\) \{[\s\S]*?\n\}/),
  // visibleSessions reads these two; neither is what this test is about.
  "let latestSessions = [];",
  "function displayState(s) { return s.state; }",
  "globalThis.api = { visibleSessions, reapDismissed, dismissedAt, DISMISS_GRACE_MS," +
    " set: (v) => { latestSessions = v; } };",
].join("\n");

const store = new Map();
globalThis.fakeStorage = {
  getItem: (k) => (store.has(k) ? store.get(k) : null),
  setItem: (k, v) => store.set(k, v),
};
new Function(code)();
const { visibleSessions, reapDismissed, dismissedAt, DISMISS_GRACE_MS, set } = globalThis.api;

const ids = (arr) => arr.map((s) => s.id).join(",");
const s = (id, state = "idle") => ({ id, state });
const a = s("a");
const b = s("b");

// 1. Nothing dismissed: every session is shown.
set([a, b]);
assert.equal(ids(visibleSessions()), "a,b");

// 2. Dismissing hides that light immediately — a poll still carrying the session
//    (one already in flight when the X was clicked) must not paint it back.
dismissedAt.set("a", Date.now());
assert.equal(ids(visibleSessions()), "b", "dismissed light hidden at once");

// 3. The poll agrees the session is gone: the tombstone is dropped, so nothing
//    accumulates and the id is free to light up again if it ever returns.
reapDismissed([b]);
assert.equal(dismissedAt.has("a"), false, "tombstone lifted once the session is gone");
set([b]);
assert.equal(ids(visibleSessions()), "b");

// 4. A dismissal that did not take (the status file could not be deleted) un-hides
//    the light after the grace period rather than hiding it forever — a light the bar
//    still has a live session for must not be silently withheld (UI Principle #4).
dismissedAt.set("a", Date.now() - DISMISS_GRACE_MS - 1);
set([a, b]);
assert.equal(ids(visibleSessions()), "b", "still hidden until the poll is consulted");
reapDismissed([a, b]);
assert.equal(dismissedAt.has("a"), false, "grace expired: the light comes back");
assert.equal(ids(visibleSessions()), "a,b");

// 5. Inside the grace period a session that is STILL reported stays hidden — that is
//    the in-flight-poll window the tombstone exists for.
dismissedAt.set("a", Date.now());
reapDismissed([a, b]);
assert.equal(ids(visibleSessions()), "b", "hidden for the whole grace window");

// 6. Dismissal composes with the Unknown filter rather than replacing it.
dismissedAt.clear();
store.set("agentstatus.showunknown", "false");
set([a, s("u", "unknown")]);
assert.equal(ids(visibleSessions()), "a", "unknown still hidden with no dismissals");
dismissedAt.set("a", Date.now());
assert.equal(ids(visibleSessions()), "", "both filters apply");

console.log("all checks passed");
