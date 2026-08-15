#!/usr/bin/env bash
# Generate the golden outputs the Rust hook port is tested against (decision 068).
#
# Replays a fixture file of real hook events through the *current* `report.sh` — the
# specification — into a throwaway status dir, and records the resulting per-session
# status file after each event. `src/hook.rs`'s equivalence test replays the same
# fixtures through the Rust implementation and must produce the same lines.
#
# Requires `jq` (a dev dependency of this script only — the shipped hook has none).
#
#   ./hooks/gen-golden.sh
#
# Re-runnable and idempotent: it rewrites the golden file from scratch every time
# (Agent Guideline #8). Re-run it whenever `report.sh` changes.
set -euo pipefail
cd "$(dirname "$0")/.."

# Two fixture files, replayed in this order — the Rust test chains them the same way.
# `captured-windows.jsonl` is a real Windows session (Guideline #4 evidence);
# `synthetic.jsonl` covers the branches a headless run cannot reach.
FIXTURE_DIR="hooks/agentstatus-hook/tests/fixtures"
CAPTURED="$FIXTURE_DIR/captured-windows.jsonl"
SYNTHETIC="$FIXTURE_DIR/synthetic.jsonl"
GOLDEN="$FIXTURE_DIR/golden.jsonl"

command -v jq >/dev/null 2>&1 || { echo "Missing: jq (needed to run report.sh)"; exit 1; }
for f in "$CAPTURED" "$SYNTHETIC"; do
  [ -f "$f" ] || { echo "Missing fixtures: $f"; exit 1; }
done

# The hook reads these; a stray value from the surrounding session would silently change
# the host tag or disable the hook entirely.
unset CLAUDE_CODE_ENTRYPOINT AGENTSTATUS_IGNORE CLAUDESTATUS_IGNORE || true

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
export AGENTSTATUS_DIR="$WORK"

: >"$GOLDEN"
n=0
while IFS= read -r line; do
  [ -z "$line" ] && continue
  n=$((n + 1))
  event="$(printf '%s' "$line" | jq -r '.event')"
  payload="$(printf '%s' "$line" | jq -c '.payload')"
  sid="$(printf '%s' "$payload" | jq -r '.session_id // empty')"

  printf '%s' "$payload" | ./hooks/report.sh "$event" || true

  f="$WORK/sessions/$sid.json"
  if [ -n "$sid" ] && [ -f "$f" ]; then
    # pid is $PPID and updated_at is wall-clock; neither is reproducible, and both are
    # asserted separately. Everything else must match exactly.
    jq -c '.pid = 0 | .updated_at = 0' <"$f" >>"$GOLDEN"
  else
    printf 'null\n' >>"$GOLDEN"
  fi
done < <(cat "$CAPTURED" "$SYNTHETIC")

echo "Wrote $n golden lines to $GOLDEN"
