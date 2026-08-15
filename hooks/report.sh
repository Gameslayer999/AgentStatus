#!/usr/bin/env bash
# AgentStatus — the real signal hook.
#
# Maps one Claude Code hook event to a session state (and a short "what it's
# working on" description) and records it in a per-session status file:
# $AGENTSTATUS_DIR/sessions/<session_id>.json (default ~/.claude/status/sessions/).
# Legacy $CLAUDESTATUS_DIR is still honored so existing installs keep working.
# One file per session → concurrent sessions never contend (decision 007).
#
# Subagents get one marker file each, under sessions/<session_id>.subagents/<agent_id>
# (contents = agent_type). Parallel subagents therefore never race (decision 010).
#
# Opt-out: if $AGENTSTATUS_IGNORE is set, the session is not tracked at all — for
# programmatic/headless agent calls (e.g. an app classifying text) that shouldn't
# appear as lights. Legacy $CLAUDESTATUS_IGNORE is still honored.
#
# Contract (Claude Code 2.1.201 — DECISIONS.md #006; Cursor 3.10.11 — #018):
#   running  <- UserPromptSubmit | PreToolUse | PostToolUse
#   blocked  <- PermissionRequest  (Claude only; Cursor has no such event)
#            <- PreToolUse for ExitPlanMode/EnterPlanMode — those prompts fire their
#               PermissionRequest only when the user answers, so PreToolUse is the
#               only event that lands while they are on screen (decision 078)
#   idle     <- Stop | SessionStart
#   error    <- StopFailure  (a real turn/API failure; PostToolUseFailure is a
#               recovered tool failure and does NOT flip the light — decision 013)
#            <- Stop with a failed .status  (Cursor turn-level error — #018, interim)
#   remove   <- SessionEnd
#   subagent <- SubagentStart (add marker) | SubagentStop (remove marker)
#
# Cursor support (decision 018): Cursor natively runs this hook via its Claude-compat
# bridge; a per-payload `ide` field ("cursor" when .cursor_version is present, else
# "vscode") drives click-to-focus. Cursor sends the workspace in .workspace_roots[]
# (not .cwd) and uses camelCase event names (normalized below).
#
# MUST be fast, non-blocking, and fail-silent (Agent Guideline #3): never write
# to stdout, never exit non-zero, swallow every error. Invoked as:
#   report.sh <EventName>          (event JSON arrives on stdin)
#
# Dependency: jq (present on this machine; the Milestone 5 installer will verify it).

STATUS_DIR="${AGENTSTATUS_DIR:-${CLAUDESTATUS_DIR:-$HOME/.claude/status}}"
SESSIONS_DIR="$STATUS_DIR/sessions"
EVENT="${1:-}"

# Which Claude Code surface is running this session (decision 054). The same
# ~/.claude/settings.json is read by every surface, so the hook already fires for
# all of them; CLAUDE_CODE_ENTRYPOINT is what tells them apart, and it costs a
# variable read (no process spawn — Agent Guideline #3).
#
# Observed values on 2.1.200: "cli" (interactive terminal), "sdk-cli" (headless
# `claude -p`), "claude-desktop" (Claude Code inside Claude Desktop).
# `sdk-cli` is deliberately left unmapped: those are short-lived scripted runs and
# lighting them re-creates the decision-013 noise. An unknown or absent value is
# also unmapped, so VS Code and Cursor keep exactly their current behaviour.
case "${CLAUDE_CODE_ENTRYPOINT:-}" in
  cli)            HOST=cli;;
  claude-desktop) HOST=claude-desktop;;
  *)              HOST="";;
esac

# Normalize Cursor's camelCase event names to the Claude PascalCase names the
# rest of this script keys on (decision 018). Cursor runs this same hook two ways:
# via its Claude-compat bridge (reads ~/.claude/settings.json, passes PascalCase)
# and via native ~/.cursor/hooks.json entries (camelCase) for the events the bridge
# drops — subagents and tool failures. Both paths land on the same logic below.
case "$EVENT" in
  sessionStart) EVENT=SessionStart;;   sessionEnd) EVENT=SessionEnd;;
  stop) EVENT=Stop;;                   beforeSubmitPrompt) EVENT=UserPromptSubmit;;
  preToolUse) EVENT=PreToolUse;;       postToolUse) EVENT=PostToolUse;;
  postToolUseFailure) EVENT=PostToolUseFailure;;
  subagentStart) EVENT=SubagentStart;; subagentStop) EVENT=SubagentStop;;
esac

{
  payload="$(cat)"
  # Opt-out for programmatic/headless sessions (decision 013).
  { [ -n "$AGENTSTATUS_IGNORE" ] || [ -n "$CLAUDESTATUS_IGNORE" ]; } && exit 0
  sid="$(printf '%s' "$payload" | jq -r '.session_id // empty' 2>/dev/null)"
  [ -z "$sid" ] && exit 0
  # Cursor fires sessionStart for an unopened "draft" composer — skip that phantom.
  [ "$sid" = "empty-state-draft" ] && exit 0
  subdir="$SESSIONS_DIR/$sid.subagents"

  # SessionEnd: drop this session's light and its subagent markers.
  if [ "$EVENT" = "SessionEnd" ]; then
    rm -f "$SESSIONS_DIR/$sid.json" 2>/dev/null
    rm -rf "$subdir" 2>/dev/null
    exit 0
  fi

  # Subagents: one marker file per subagent — race-free under parallel subagents.
  if [ "$EVENT" = "SubagentStart" ] || [ "$EVENT" = "SubagentStop" ]; then
    aid="$(printf '%s' "$payload" | jq -r '.agent_id // .subagent_id // empty' 2>/dev/null)"
    [ -z "$aid" ] && exit 0
    if [ "$EVENT" = "SubagentStart" ]; then
      atype="$(printf '%s' "$payload" | jq -r '.agent_type // .subagent_type // "agent"' 2>/dev/null)"
      mkdir -p "$subdir" 2>/dev/null
      printf '%s' "$atype" >"$subdir/$aid" 2>/dev/null
    else
      rm -f "$subdir/$aid" 2>/dev/null
    fi
    exit 0
  fi

  ts="$(date +%s)"

  # Failure calibration: a turn-level StopFailure is a real error (red); a
  # PostToolUseFailure is a recovered tool failure — log it but don't flip state.
  if [ "$EVENT" = "PostToolUseFailure" ] || [ "$EVENT" = "StopFailure" ]; then
    tool="$(printf '%s' "$payload" | jq -r '.tool_name // ""' 2>/dev/null)"
    intr="$(printf '%s' "$payload" | jq -r '.is_interrupt // false' 2>/dev/null)"
    printf '%s\t%s\t%s\ttool=%s\tinterrupt=%s\n' "$ts" "$EVENT" "$sid" "$tool" "$intr" \
      >>"$STATUS_DIR/calibration.log" 2>/dev/null
    [ "$EVENT" = "PostToolUseFailure" ] && exit 0
  fi

  old_json=""
  [ -f "$SESSIONS_DIR/$sid.json" ] && old_json="$(cat "$SESSIONS_DIR/$sid.json" 2>/dev/null)"

  # One jq pass: map event -> state, carry forward task, compute a fresh detail,
  # and emit the merged status object (or empty to skip unmapped events).
  obj="$(printf '%s' "$payload" | jq -c \
      --arg event "$EVENT" --argjson ts "$ts" --arg oldjson "$old_json" \
      --arg host "$HOST" --argjson pid "${PPID:-0}" '
    def clean: (. // "") | gsub("[\n\r\t]+";" ") | gsub("^ +| +$";"");
    def trunc($n): clean | if (length > $n) then (.[:$n] + "…") else . end;
    # Windows sends backslash paths ("C:\\Users\\x\\proj", verified live), which a
    # "/"-only split leaves whole — the label became the entire path and the file detail
    # showed a full path instead of a filename. Only a drive letter or a UNC root counts
    # as Windows, and no macOS path starts either way, so macOS splitting is unchanged.
    def norm: if (test("^[A-Za-z]:") or startswith("\\\\")) then (split("\\") | join("/")) else . end;
    ($oldjson | if . == "" then {} else (fromjson? // {}) end) as $old
    | . as $p
    # A plan-mode approval is the one prompt Claude Code does not announce when it
    # appears: PermissionRequest fires at resolution time, so the only event during the
    # wait is this PreToolUse and the light stays green throughout (decision 078,
    # measured at 48 seconds). Both tools exist solely to stop and ask, so their
    # PreToolUse is the prompt; rewriting the event gives the light the same state and
    # the same wording the late event would have written.
    | (if $event == "PreToolUse"
         and (($p.tool_name // "") | test("^(ExitPlanMode|EnterPlanMode)$"))
       then "PermissionRequest" else $event end) as $ev
    | ({ "UserPromptSubmit":"running", "PreToolUse":"running", "PostToolUse":"running",
         "PermissionRequest":"blocked", "Stop":"idle", "SessionStart":"idle",
         "StopFailure":"error" }[$ev]) as $base
    | ($p.cursor_version != null) as $isCursor
    | (($p.status // "") | test("error|fail|abort|cancel"; "i")) as $failedStop
    | (if $ev == "Stop" and $failedStop then "error" else $base end) as $state
    | select($state != null)
    # Cursor puts the workspace in workspace_roots[]; a tool-level .cwd (e.g. /tmp) is
    # the exec dir of that tool call, not the session folder — prefer workspace_roots.
    | (if $isCursor then (($p.workspace_roots // [])[0] // $old.cwd)
       else ($p.cwd // $old.cwd) end // "") as $cwd
    # Cursor wins and stays sticky (its native camelCase hooks carry no
    # .cursor_version), then the entrypoint-derived host, then the prior default.
    | (if $isCursor then "cursor"
       elif ($old.ide // "") == "cursor" then "cursor"
       elif $host != "" then $host
       else ($old.ide // "vscode") end) as $ide
    | ($p.tool_name // "") as $tool
    | (if $ev == "UserPromptSubmit" then ($p.prompt | trunc(160)) else ($old.task // "") end) as $task
    | (if $ev == "PreToolUse" then
         (if $tool == "Bash" then "$ " + ($p.tool_input.command | trunc(90))
          elif ($tool | test("^(Edit|Write|Read|NotebookEdit)$")) then
            $tool + " " + (($p.tool_input.file_path // "") | norm | split("/") | last)
          else "Running " + $tool end)
       elif $ev == "PermissionRequest" then
         (if $tool == "AskUserQuestion" then "⏸ waiting — a question for you"
          else "⏸ waiting — approve " + $tool end)
       elif $ev == "Stop" then
         (if $failedStop then ("⚠ turn failed — " + ($p.status // "")) else (($p.last_assistant_message // "") | trunc(160)) end)
       elif $ev == "StopFailure" then ("⚠ turn failed" + (if ($p.error_type // "") != "" then " — " + $p.error_type else "" end))
       elif $ev == "SessionStart" then ""
       else ($old.detail // "") end) as $detail
    # pid = the parent of this hook process, i.e. the claude process itself. A CLI
    # session has no IDE lock file to prove it is still alive, so the app checks
    # this pid instead (decision 054). No apostrophes in this comment: the jq
    # program is single-quoted, so one would terminate it.
    | { state: $state, cwd: $cwd, ide: $ide, pid: $pid,
        label: ($cwd | norm | split("/") | map(select(length > 0)) | last // ""),
        updated_at: $ts, task: $task, detail: $detail }
  ' 2>/dev/null)"

  [ -z "$obj" ] && exit 0

  # Atomic write: temp file in the same dir, then rename.
  mkdir -p "$SESSIONS_DIR" 2>/dev/null
  tmp="$SESSIONS_DIR/.$sid.$$.tmp"
  printf '%s\n' "$obj" >"$tmp" 2>/dev/null && mv -f "$tmp" "$SESSIONS_DIR/$sid.json" 2>/dev/null
  rm -f "$tmp" 2>/dev/null

  # Turn boundary: clear any lingering subagent markers.
  if [ "$EVENT" = "Stop" ] || [ "$EVENT" = "SessionStart" ]; then
    rm -rf "$subdir" 2>/dev/null
  fi
} >/dev/null 2>&1

exit 0
