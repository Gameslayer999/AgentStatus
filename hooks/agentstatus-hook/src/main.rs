//! AgentStatus — the signal hook, as a native binary.
//!
//! A faithful port of `hooks/report.sh`. It maps one Claude Code (or Cursor) hook event
//! to a session state and records it in `$AGENTSTATUS_DIR/sessions/<session_id>.json`.
//! The event→state contract, the field precedence, and the output shape are all
//! unchanged — see `hooks/report.sh` and DECISIONS.md #006/#013/#018/#048/#054 for why
//! each rule is what it is. This file is the *implementation*, not a redesign; where the
//! two could differ, `report.sh` is the specification and the equivalence tests below
//! prove they agree.
//!
//! Why a binary instead of the shell script (decision 068): `report.sh` spawns eight
//! external commands per event (two of them `jq`). On macOS that costs ~15–25 ms and is
//! invisible; on Windows, where MSYS emulates `fork()`, the same script measures ~287 ms
//! per event — ~574 ms of added latency on every tool call, which Agent Guideline #3
//! forbids. It also removes `jq`, an undeclared runtime dependency that ships only on
//! macOS 15+ and not at all on Windows, and whose absence made the app silently inert.
//!
//! MUST be fast, non-blocking, and fail-silent (Agent Guideline #3): never write to
//! stdout, never exit non-zero, swallow every error.
//!
//! Built by `hooks/stage-hook.mjs`, which stages it as a Tauri bundle resource; the app
//! copies it to `~/.claude/status/` on first launch (`install.rs`).

// A console-subsystem binary makes Windows allocate a console for every invocation, and the
// hook is invoked on *every tool call* — which flashed a black window on and off the user's
// screen all day. Nothing here writes to stdout or stderr by contract (Agent Guideline #3:
// a hook must never surface noise into the user's session), so there is nothing to lose by
// having no console. Unconditional rather than release-only: any build that gets registered
// must be silent, and the attribute is ignored on non-Windows targets.
#![windows_subsystem = "windows"]

fn main() {
    run_from_env();
    // Always exit 0 — a hook that fails must never fail the user's session.
}

use serde::Serialize;
use serde_json::{json, Value};
use std::path::PathBuf;

/// One session's status file. A struct rather than a `serde_json::Value` because serde
/// serializes fields in declaration order while `json!` sorts them alphabetically — and
/// this order is `report.sh`'s, so the two implementations write byte-identical files.
#[derive(Debug, PartialEq, Serialize)]
pub struct Status {
    pub state: String,
    pub cwd: String,
    pub ide: String,
    pub pid: i64,
    pub label: String,
    pub updated_at: i64,
    pub task: String,
    pub detail: String,
}

/// Everything the decision logic needs from the environment, passed in rather than read,
/// so the equivalence tests can pin the two non-deterministic fields.
pub struct Env {
    /// Which Claude Code surface is running this session (decision 054), derived from
    /// `CLAUDE_CODE_ENTRYPOINT`. Empty when the value is absent or deliberately unmapped.
    pub host: String,
    /// The `claude` process that owns this session — `$PPID` in the shell version.
    pub pid: i64,
    /// Unix seconds. The hook stamps whole seconds, and `turn_ended` in lib.rs depends on
    /// that granularity, so this stays seconds and not millis.
    pub now: i64,
}

/// What the caller must do to the filesystem. Separating the decision from the IO is what
/// lets the tests replay a whole session through `decide` with no disk at all.
#[derive(Debug, PartialEq)]
pub enum Action {
    /// Event carries no state mapping — leave the status file exactly as it is.
    None,
    /// `SessionEnd`: drop the light and every subagent marker.
    RemoveSession,
    /// `SubagentStart`: one marker file per subagent, contents = agent_type (decision 010).
    SubagentStart { agent_id: String, agent_type: String },
    /// `SubagentStop`: remove that one marker.
    SubagentStop { agent_id: String },
    /// Replace the session's status file with this object.
    Write(Status),
}

/// The full result of one event.
#[derive(Debug)]
pub struct Outcome {
    pub session_id: String,
    pub action: Action,
    /// A line to append to `calibration.log` (failure calibration, decision 013).
    pub calibration: Option<String>,
    /// Turn boundary — clear any lingering subagent markers (`Stop`/`SessionStart`).
    pub clear_subagents: bool,
}

/// Normalize Cursor's camelCase event names to the Claude PascalCase names everything
/// else keys on (decision 018). Cursor reaches this hook two ways — its Claude-compat
/// bridge (PascalCase) and native `~/.cursor/hooks.json` entries (camelCase) — and both
/// must land on the same logic.
pub fn normalize_event(event: &str) -> &str {
    match event {
        "sessionStart" => "SessionStart",
        "sessionEnd" => "SessionEnd",
        "stop" => "Stop",
        "beforeSubmitPrompt" => "UserPromptSubmit",
        "preToolUse" => "PreToolUse",
        "postToolUse" => "PostToolUse",
        "postToolUseFailure" => "PostToolUseFailure",
        "subagentStart" => "SubagentStart",
        "subagentStop" => "SubagentStop",
        other => other,
    }
}

/// `CLAUDE_CODE_ENTRYPOINT` → host tag. `sdk-cli` (headless `claude -p`) is deliberately
/// left unmapped: those are short-lived scripted runs and lighting them re-creates the
/// decision-013 noise. An unknown or absent value is unmapped too, so VS Code and Cursor
/// keep exactly their existing behaviour.
pub fn host_from_entrypoint(entrypoint: &str) -> String {
    match entrypoint {
        "cli" => "cli".to_string(),
        "claude-desktop" => "claude-desktop".to_string(),
        _ => String::new(),
    }
}

/// jq's `clean`: collapse every run of newline/carriage-return/tab into one space, then
/// strip leading and trailing spaces. Spaces only — tabs are already gone by then.
fn clean(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_run = false;
    for c in s.chars() {
        if c == '\n' || c == '\r' || c == '\t' {
            if !in_run {
                out.push(' ');
                in_run = true;
            }
        } else {
            out.push(c);
            in_run = false;
        }
    }
    out.trim_matches(' ').to_string()
}

/// jq's `trunc($n)`: clean, then cut to `n` **codepoints** (jq's `length` and `.[:n]` are
/// both codepoint-based, so byte slicing here would disagree on any non-ASCII prompt).
fn trunc(s: &str, n: usize) -> String {
    let c = clean(s);
    if c.chars().count() > n {
        let mut t: String = c.chars().take(n).collect();
        t.push('…');
        t
    } else {
        c
    }
}

/// `.field // ""` for a string field.
fn str_or_empty(v: &Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

/// Whether the payload delivered this path in Windows form — a drive letter (`C:\…`) or a
/// UNC root (`\\server\…`). No macOS path starts either way, so treating `\` as a
/// separator for these and only these leaves macOS behaviour untouched (Agent Guideline
/// #7) while fixing Windows labels. `report.sh`'s `norm` applies the identical test, which
/// is what lets the equivalence test demand strict equality on both platforms.
fn is_windows_path(p: &str) -> bool {
    let b = p.as_bytes();
    (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':') || p.starts_with("\\\\")
}

/// report.sh's `norm`: backslashes become slashes, but only on a Windows-shaped path.
fn norm(p: &str) -> std::borrow::Cow<'_, str> {
    if is_windows_path(p) {
        std::borrow::Cow::Owned(p.replace('\\', "/"))
    } else {
        std::borrow::Cow::Borrowed(p)
    }
}

/// jq's `norm | split("/") | map(select(length > 0)) | last // ""` — the project folder.
fn label_of(cwd: &str) -> String {
    norm(cwd)
        .split('/')
        .filter(|s| !s.is_empty())
        .next_back()
        .unwrap_or("")
        .to_string()
}

/// jq's `norm | split("/") | last` — this one does *not* drop empty segments, matching the
/// file_path basename in the detail line.
fn basename_of(path: &str) -> String {
    norm(path).split('/').next_back().unwrap_or("").to_string()
}

/// jq's `test("error|fail|abort|cancel"; "i")`. The pattern is pure alternation with no
/// metacharacters, so a lowercase substring check is exactly equivalent — and saves
/// pulling a regex crate into a binary that runs on every tool call.
fn looks_failed(status: &str) -> bool {
    let s = status.to_ascii_lowercase();
    ["error", "fail", "abort", "cancel"].iter().any(|k| s.contains(k))
}

/// Map one event to what should happen. `old` is the session's current status file, if any.
/// Returns `None` when the event should be ignored outright (no session id, opt-out, or
/// Cursor's phantom draft composer).
pub fn decide(event: &str, payload: &Value, old: Option<&Value>, env: &Env) -> Option<Outcome> {
    let event = normalize_event(event);

    let sid = str_or_empty(payload, "session_id");
    if sid.is_empty() {
        return None;
    }
    // Cursor fires sessionStart for an unopened "draft" composer — skip that phantom.
    if sid == "empty-state-draft" {
        return None;
    }

    if event == "SessionEnd" {
        return Some(Outcome {
            session_id: sid,
            action: Action::RemoveSession,
            calibration: None,
            clear_subagents: true,
        });
    }

    if event == "SubagentStart" || event == "SubagentStop" {
        let aid = payload
            .get("agent_id")
            .or_else(|| payload.get("subagent_id"))
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        if aid.is_empty() {
            return None;
        }
        let action = if event == "SubagentStart" {
            let atype = payload
                .get("agent_type")
                .or_else(|| payload.get("subagent_type"))
                .and_then(|x| x.as_str())
                .unwrap_or("agent")
                .to_string();
            Action::SubagentStart { agent_id: aid, agent_type: atype }
        } else {
            Action::SubagentStop { agent_id: aid }
        };
        return Some(Outcome { session_id: sid, action, calibration: None, clear_subagents: false });
    }

    // A plan-mode approval is the one prompt Claude Code does not announce when it appears.
    // Measured live on 2.1.229 (decision 078): for `ExitPlanMode` the `PermissionRequest`
    // fires at *resolution* time, so the only event during the wait is this `PreToolUse` —
    // the light stayed green for the 48 s the approval sat on screen and turned orange for
    // under 150 ms at the end, after the user had already dealt with it. `Bash` behaves the
    // other way round (its `PermissionRequest` lands 62 ms after `PreToolUse` and holds
    // `blocked` for the whole wait), which is why only these tools need this.
    //
    // Both exist solely to stop and ask the user, so their `PreToolUse` *is* the prompt.
    // Rewriting the event gives the light exactly the state and the wording the late
    // `PermissionRequest` would have written, and `PostToolUse` turns it green again on
    // approval. When one is auto-approved and no prompt appears, the cost is a sub-second
    // orange flicker — accepted deliberately, because the alternative failure is a green
    // light on a session that is waiting for you (UI Principle #2).
    const WAITS_ON_USER: [&str; 2] = ["ExitPlanMode", "EnterPlanMode"];
    let event = if event == "PreToolUse"
        && WAITS_ON_USER.contains(&str_or_empty(payload, "tool_name").as_str())
    {
        "PermissionRequest"
    } else {
        event
    };

    // Failure calibration: a turn-level StopFailure is a real error (red); a
    // PostToolUseFailure is a recovered tool failure — log it but don't flip state
    // (decision 013).
    let mut calibration = None;
    if event == "PostToolUseFailure" || event == "StopFailure" {
        let intr = payload
            .get("is_interrupt")
            .map(|v| if v.is_boolean() { v.as_bool().unwrap().to_string() } else { v.to_string() })
            .unwrap_or_else(|| "false".to_string());
        calibration = Some(format!(
            "{}\t{}\t{}\ttool={}\tinterrupt={}",
            env.now,
            event,
            sid,
            str_or_empty(payload, "tool_name"),
            intr
        ));
        if event == "PostToolUseFailure" {
            return Some(Outcome {
                session_id: sid,
                action: Action::None,
                calibration,
                clear_subagents: false,
            });
        }
    }

    let empty = json!({});
    let old = old.unwrap_or(&empty);

    let base = match event {
        "UserPromptSubmit" | "PreToolUse" | "PostToolUse" => Some("running"),
        "PermissionRequest" => Some("blocked"),
        "Stop" | "SessionStart" => Some("idle"),
        "StopFailure" => Some("error"),
        _ => None,
    };

    let is_cursor = payload.get("cursor_version").is_some_and(|v| !v.is_null());
    let failed_stop = looks_failed(&str_or_empty(payload, "status"));
    let state = match base {
        Some(_) if event == "Stop" && failed_stop => "error",
        Some(b) => b,
        // Unmapped event: emit nothing, leave the file untouched.
        None => {
            return Some(Outcome {
                session_id: sid,
                action: Action::None,
                calibration,
                clear_subagents: false,
            })
        }
    };

    // Cursor puts the workspace in workspace_roots[]; a tool-level .cwd (e.g. /tmp) is the
    // exec dir of that tool call, not the session folder — prefer workspace_roots.
    let old_cwd = str_or_empty(old, "cwd");
    let cwd = if is_cursor {
        payload
            .get("workspace_roots")
            .and_then(|x| x.as_array())
            .and_then(|a| a.first())
            .and_then(|x| x.as_str())
            .map(|s| s.to_string())
            .unwrap_or(old_cwd)
    } else {
        let c = str_or_empty(payload, "cwd");
        if c.is_empty() { old_cwd } else { c }
    };

    // Cursor wins and stays sticky (its native camelCase hooks carry no .cursor_version),
    // then the entrypoint-derived host, then the prior value, defaulting to vscode.
    let old_ide = str_or_empty(old, "ide");
    let ide = if is_cursor {
        "cursor".to_string()
    } else if old_ide == "cursor" {
        "cursor".to_string()
    } else if !env.host.is_empty() {
        env.host.clone()
    } else if !old_ide.is_empty() {
        old_ide
    } else {
        "vscode".to_string()
    };

    let tool = str_or_empty(payload, "tool_name");

    let task = if event == "UserPromptSubmit" {
        trunc(&str_or_empty(payload, "prompt"), 160)
    } else {
        str_or_empty(old, "task")
    };

    let tool_input = payload.get("tool_input");
    let detail = match event {
        "PreToolUse" => {
            if tool == "Bash" {
                let cmd = tool_input
                    .and_then(|t| t.get("command"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("");
                format!("$ {}", trunc(cmd, 90))
            } else if matches!(tool.as_str(), "Edit" | "Write" | "Read" | "NotebookEdit") {
                let fp = tool_input
                    .and_then(|t| t.get("file_path"))
                    .and_then(|x| x.as_str())
                    .unwrap_or("");
                format!("{} {}", tool, basename_of(fp))
            } else {
                format!("Running {tool}")
            }
        }
        "PermissionRequest" => {
            if tool == "AskUserQuestion" {
                "⏸ waiting — a question for you".to_string()
            } else {
                format!("⏸ waiting — approve {tool}")
            }
        }
        "Stop" => {
            if failed_stop {
                format!("⚠ turn failed — {}", str_or_empty(payload, "status"))
            } else {
                trunc(&str_or_empty(payload, "last_assistant_message"), 160)
            }
        }
        "StopFailure" => {
            let et = str_or_empty(payload, "error_type");
            if et.is_empty() {
                "⚠ turn failed".to_string()
            } else {
                format!("⚠ turn failed — {et}")
            }
        }
        "SessionStart" => String::new(),
        _ => str_or_empty(old, "detail"),
    };

    let status = Status {
        state: state.to_string(),
        label: label_of(&cwd),
        cwd,
        ide,
        pid: env.pid,
        updated_at: env.now,
        task,
        detail,
    };

    Some(Outcome {
        session_id: sid,
        action: Action::Write(status),
        calibration,
        clear_subagents: event == "Stop" || event == "SessionStart",
    })
}

// ---------------------------------------------------------------------------
// IO layer
// ---------------------------------------------------------------------------

/// The user's home directory. `HOME` is what the shell hook used and Claude Code sets it
/// even on Windows (verified live), but a Windows process started any other way has only
/// `USERPROFILE` — so fall back rather than resolve to an empty path.
fn home() -> PathBuf {
    for key in ["HOME", "USERPROFILE"] {
        if let Ok(v) = std::env::var(key) {
            if !v.is_empty() {
                return PathBuf::from(v);
            }
        }
    }
    PathBuf::new()
}

/// `$AGENTSTATUS_DIR`, else the legacy `$CLAUDESTATUS_DIR`, else `~/.claude/status`.
pub fn status_dir() -> PathBuf {
    for key in ["AGENTSTATUS_DIR", "CLAUDESTATUS_DIR"] {
        if let Ok(v) = std::env::var(key) {
            if !v.is_empty() {
                return PathBuf::from(v);
            }
        }
    }
    home().join(".claude").join("status")
}

/// The `claude` process that owns this session — the shell hook's `$PPID`.
///
/// On Windows the immediate parent is a Git Bash shell, sometimes two deep, and those shells
/// exit the moment the hook does — recording one would hand the app a pid that is already
/// dead. Measured ancestry (decision 071):
/// `agentstatus-hook.exe → bash.exe → [bash.exe] → claude.exe`. So walk up to the nearest
/// `claude.exe`, which is exactly what `$PPID` resolves to on macOS, where bash `exec`s the
/// hook in its own place.
///
/// Returns 0 if no `claude.exe` ancestor is found rather than guessing at a shell pid:
/// every consumer treats 0 as "unknown" and falls back, which is the honest answer.
#[cfg(windows)]
fn parent_pid() -> i64 {
    use std::collections::HashMap;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    // SAFETY: a process snapshot takes no borrowed state; the handle is closed below.
    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snap == INVALID_HANDLE_VALUE {
        return 0;
    }

    let mut tree: HashMap<u32, (u32, String)> = HashMap::new();
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
    // SAFETY: `entry` is zeroed with dwSize set, as the API requires.
    if unsafe { Process32FirstW(snap, &mut entry) } != 0 {
        loop {
            let end = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..end]).to_ascii_lowercase();
            tree.insert(entry.th32ProcessID, (entry.th32ParentProcessID, name));
            if unsafe { Process32NextW(snap, &mut entry) } == 0 {
                break;
            }
        }
    }
    unsafe { CloseHandle(snap) };

    // Bounded: a corrupt or recycled tree must not spin here.
    let mut cur = std::process::id();
    for _ in 0..12 {
        let Some(&(parent, _)) = tree.get(&cur) else { return 0 };
        if parent == 0 {
            return 0;
        }
        match tree.get(&parent) {
            Some((_, name)) if name == "claude.exe" => return parent as i64,
            Some(_) => cur = parent,
            None => return 0,
        }
    }
    0
}

#[cfg(unix)]
fn parent_pid() -> i64 {
    // SAFETY: getppid() takes no arguments, cannot fail, and touches no memory.
    unsafe { libc::getppid() as i64 }
}

/// Read the event name from argv, the payload from stdin, and apply the outcome. Every
/// failure is swallowed: a hook that errors must not surface noise into the user's
/// session (Agent Guideline #3).
pub fn run_from_env() {
    // Opt-out for programmatic/headless sessions (decision 013).
    for key in ["AGENTSTATUS_IGNORE", "CLAUDESTATUS_IGNORE"] {
        if std::env::var(key).is_ok_and(|v| !v.is_empty()) {
            return;
        }
    }

    let event = std::env::args().nth(1).unwrap_or_default();

    let mut input = String::new();
    if std::io::Read::read_to_string(&mut std::io::stdin(), &mut input).is_err() {
        return;
    }
    let Ok(payload) = serde_json::from_str::<Value>(&input) else { return };

    let env = Env {
        host: host_from_entrypoint(
            &std::env::var("CLAUDE_CODE_ENTRYPOINT").unwrap_or_default(),
        ),
        pid: parent_pid(),
        now: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
    };

    let dir = status_dir();
    let sessions = dir.join("sessions");

    let old = std::fs::read_to_string(sessions.join(format!(
        "{}.json",
        str_or_empty(&payload, "session_id")
    )))
    .ok()
    .and_then(|t| serde_json::from_str::<Value>(&t).ok());

    let Some(outcome) = decide(&event, &payload, old.as_ref(), &env) else { return };
    apply(&dir, &outcome);
}

/// Perform an outcome's filesystem effects. Best-effort throughout.
pub fn apply(dir: &std::path::Path, outcome: &Outcome) {
    let sessions = dir.join("sessions");
    let sid = &outcome.session_id;
    let subdir = sessions.join(format!("{sid}.subagents"));

    if let Some(line) = &outcome.calibration {
        let _ = std::fs::create_dir_all(dir);
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("calibration.log"))
        {
            let _ = std::io::Write::write_all(&mut f, format!("{line}\n").as_bytes());
        }
    }

    match &outcome.action {
        Action::None => {}
        Action::RemoveSession => {
            let _ = std::fs::remove_file(sessions.join(format!("{sid}.json")));
            let _ = std::fs::remove_dir_all(&subdir);
        }
        Action::SubagentStart { agent_id, agent_type } => {
            let _ = std::fs::create_dir_all(&subdir);
            let _ = std::fs::write(subdir.join(agent_id), agent_type);
        }
        Action::SubagentStop { agent_id } => {
            let _ = std::fs::remove_file(subdir.join(agent_id));
        }
        Action::Write(obj) => {
            let _ = std::fs::create_dir_all(&sessions);
            let Ok(text) = serde_json::to_string(obj) else { return };
            // Atomic write: temp file in the same dir, then rename over the target.
            let tmp = sessions.join(format!(".{sid}.{}.tmp", std::process::id()));
            if std::fs::write(&tmp, format!("{text}\n")).is_ok()
                && std::fs::rename(&tmp, sessions.join(format!("{sid}.json"))).is_err()
            {
                let _ = std::fs::remove_file(&tmp);
            }
        }
    }

    if outcome.clear_subagents && !matches!(outcome.action, Action::RemoveSession) {
        let _ = std::fs::remove_dir_all(&subdir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // A real Windows session (Guideline #4 evidence) followed by synthetic cases covering
    // the branches a headless run cannot reach — PermissionRequest, StopFailure, Cursor,
    // truncation, and the malformed-input paths.
    const CAPTURED: &str = include_str!("../tests/fixtures/captured-windows.jsonl");
    const SYNTHETIC: &str = include_str!("../tests/fixtures/synthetic.jsonl");
    // Produced by `hooks/gen-golden.sh`, which replays the same fixtures through the
    // *current* report.sh. report.sh is the specification; this file is its recorded output.
    const GOLDEN: &str = include_str!("../tests/fixtures/golden.jsonl");

    fn fixtures() -> Vec<Value> {
        CAPTURED
            .lines()
            .chain(SYNTHETIC.lines())
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).expect("fixture line is not valid JSON"))
            .collect()
    }

    /// The port must produce byte-identical status files to `report.sh` for every fixture,
    /// replayed in sequence so task/detail/ide carry-forward is exercised too.
    #[test]
    fn matches_report_sh_on_every_fixture() {
        let env = Env { host: String::new(), pid: 0, now: 0 };
        let fx = fixtures();
        let golden: Vec<Value> = GOLDEN
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).expect("golden line is not valid JSON"))
            .collect();
        assert_eq!(
            fx.len(),
            golden.len(),
            "fixture/golden count mismatch — re-run hooks/gen-golden.sh"
        );

        let mut store: HashMap<String, Value> = HashMap::new();
        for (i, (wrapper, want_raw)) in fx.iter().zip(golden.iter()).enumerate() {
            let event = wrapper["event"].as_str().expect("fixture has no event");
            let payload = &wrapper["payload"];
            let sid = payload
                .get("session_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if let Some(outcome) = decide(event, payload, store.get(&sid), &env) {
                let Outcome { session_id, action, .. } = outcome;
                match action {
                    Action::Write(status) => {
                        store.insert(session_id, serde_json::to_value(status).unwrap());
                    }
                    Action::RemoveSession => {
                        store.remove(&session_id);
                    }
                    _ => {}
                }
            }

            let got = store.get(&sid).cloned().unwrap_or(Value::Null);
            assert_eq!(
                got,
                *want_raw,
                "fixture #{} ({event}) diverged from report.sh",
                i + 1
            );
        }
    }

    /// Serialization must match jq's key order and compact form, or the files differ even
    /// when the values agree.
    #[test]
    fn writes_keys_in_report_sh_order() {
        let env = Env { host: String::new(), pid: 7, now: 42 };
        let payload = json!({"session_id": "s", "cwd": "/a/b", "hook_event_name": "SessionStart"});
        let outcome = decide("SessionStart", &payload, None, &env).unwrap();
        let Action::Write(status) = outcome.action else { panic!("expected a write") };
        assert_eq!(
            serde_json::to_string(&status).unwrap(),
            r#"{"state":"idle","cwd":"/a/b","ide":"vscode","pid":7,"label":"b","updated_at":42,"task":"","detail":""}"#
        );
    }

    /// The rule is the path's *shape*, not the build target, so these expectations hold
    /// identically on macOS and Windows — which is what makes the golden comparison strict
    /// on both.
    #[test]
    fn label_splits_windows_shaped_paths_only() {
        assert_eq!(label_of("/Users/x/proj/Widget"), "Widget");
        assert_eq!(label_of("/Users/x/proj/Trailing/"), "Trailing");
        assert_eq!(label_of(""), "");
        assert_eq!(label_of("/"), "");
        assert_eq!(label_of("C:\\proj\\AgentStatus"), "AgentStatus");
        assert_eq!(label_of("C:\\proj\\AgentStatus\\"), "AgentStatus");
        assert_eq!(label_of("\\\\server\\share\\proj"), "proj");
        // A POSIX path that merely contains a backslash is NOT a Windows path: the
        // backslash stays part of the filename, exactly as report.sh has always treated it.
        assert_eq!(label_of("/a/weird\\name"), "weird\\name");
    }

    #[test]
    fn basename_keeps_empty_segments_like_jq() {
        assert_eq!(basename_of("/a/b/c.rs"), "c.rs");
        assert_eq!(basename_of("c.rs"), "c.rs");
        assert_eq!(basename_of("C:\\proj\\src\\main.rs"), "main.rs");
        // jq's `split("/") | last` does not drop a trailing empty segment.
        assert_eq!(basename_of("/a/b/"), "");
    }

    #[test]
    fn windows_path_detection_is_shape_based() {
        assert!(is_windows_path("C:\\proj"));
        assert!(is_windows_path("c:/proj"));
        assert!(is_windows_path("\\\\server\\share"));
        assert!(!is_windows_path("/Users/x"));
        assert!(!is_windows_path("/a/weird\\name"));
        assert!(!is_windows_path(""));
    }

    #[test]
    fn clean_collapses_whitespace_runs_then_trims() {
        assert_eq!(clean("a\n\n\tb"), "a b");
        assert_eq!(clean("  padded  "), "padded");
        assert_eq!(clean("\n\nlead"), "lead");
        assert_eq!(clean(""), "");
    }

    /// jq's `length` and `.[:n]` are codepoint-based; byte slicing would both mis-count and
    /// panic on a multi-byte boundary.
    #[test]
    fn trunc_counts_codepoints() {
        assert_eq!(trunc("abc", 10), "abc");
        assert_eq!(trunc("abcdef", 3), "abc…");
        let unicode = "ünïcödé".repeat(30);
        assert_eq!(trunc(&unicode, 5).chars().count(), 6); // 5 + the ellipsis
    }

    #[test]
    fn failed_status_matches_the_jq_pattern() {
        for s in ["error", "ERROR", "failed", "aborted", "user cancelled"] {
            assert!(looks_failed(s), "{s} should read as failed");
        }
        for s in ["", "completed", "running", "ok"] {
            assert!(!looks_failed(s), "{s} should not read as failed");
        }
    }

    #[test]
    fn subagent_events_use_both_field_spellings() {
        let env = Env { host: String::new(), pid: 0, now: 0 };
        let claude = json!({"session_id": "s", "agent_id": "a1", "agent_type": "Explore"});
        let outcome = decide("SubagentStart", &claude, None, &env).unwrap();
        assert_eq!(
            outcome.action,
            Action::SubagentStart { agent_id: "a1".into(), agent_type: "Explore".into() }
        );

        let cursor = json!({"session_id": "s", "subagent_id": "a2"});
        let outcome = decide("subagentStart", &cursor, None, &env).unwrap();
        // No type given: report.sh defaults to "agent".
        assert_eq!(
            outcome.action,
            Action::SubagentStart { agent_id: "a2".into(), agent_type: "agent".into() }
        );

        let outcome = decide("SubagentStop", &claude, None, &env).unwrap();
        assert_eq!(outcome.action, Action::SubagentStop { agent_id: "a1".into() });
    }

    #[test]
    fn turn_boundaries_clear_subagent_markers() {
        let env = Env { host: String::new(), pid: 0, now: 0 };
        let p = json!({"session_id": "s", "cwd": "/a"});
        for event in ["Stop", "SessionStart"] {
            assert!(decide(event, &p, None, &env).unwrap().clear_subagents, "{event}");
        }
        assert!(!decide("PreToolUse", &p, None, &env).unwrap().clear_subagents);
        assert!(decide("SessionEnd", &p, None, &env).unwrap().clear_subagents);
    }

    /// A recovered tool failure is logged but must not flip the light (decision 013).
    #[test]
    fn post_tool_use_failure_logs_without_changing_state() {
        let env = Env { host: String::new(), pid: 0, now: 99 };
        let p = json!({"session_id": "s", "tool_name": "Bash", "is_interrupt": false});
        let outcome = decide("PostToolUseFailure", &p, None, &env).unwrap();
        assert_eq!(outcome.action, Action::None);
        assert_eq!(
            outcome.calibration.unwrap(),
            "99\tPostToolUseFailure\ts\ttool=Bash\tinterrupt=false"
        );
    }

    /// A plan-mode prompt is orange for the whole time it is on screen (decision 078).
    /// Measured live: `ExitPlanMode`'s `PermissionRequest` fires when the user *answers*,
    /// so `PreToolUse` is the only event during the wait and the light was green for 48 s.
    /// Every other tool keeps `PreToolUse` → running, which is what the light means.
    #[test]
    fn plan_mode_tools_block_from_pre_tool_use() {
        let env = Env { host: String::new(), pid: 0, now: 0 };
        for tool in ["ExitPlanMode", "EnterPlanMode"] {
            let p = json!({"session_id": "s", "cwd": "/a", "tool_name": tool});
            let Action::Write(st) = decide("PreToolUse", &p, None, &env).unwrap().action else {
                panic!("{tool} produced no write");
            };
            assert_eq!(st.state, "blocked", "{tool}");
            assert_eq!(st.detail, format!("⏸ waiting — approve {tool}"), "{tool}");

            // Answering it is a PostToolUse, which returns the light to green.
            let old = serde_json::to_value(&st).unwrap();
            let Action::Write(st) = decide("PostToolUse", &p, Some(&old), &env).unwrap().action
            else {
                panic!("{tool} answer produced no write");
            };
            assert_eq!(st.state, "running", "{tool}");
        }

        // Not a blanket rule on PreToolUse — an ordinary tool is still running.
        let p = json!({"session_id": "s", "cwd": "/a", "tool_name": "Bash",
                       "tool_input": {"command": "ls"}});
        let Action::Write(st) = decide("PreToolUse", &p, None, &env).unwrap().action else {
            panic!("no write");
        };
        assert_eq!(st.state, "running");
        assert_eq!(st.detail, "$ ls");
    }

    #[test]
    fn host_tag_only_maps_the_two_verified_entrypoints() {
        assert_eq!(host_from_entrypoint("cli"), "cli");
        assert_eq!(host_from_entrypoint("claude-desktop"), "claude-desktop");
        // Deliberately unmapped — short-lived scripted runs (decision 013).
        assert_eq!(host_from_entrypoint("sdk-cli"), "");
        assert_eq!(host_from_entrypoint(""), "");
    }

    #[test]
    fn ignores_sessions_it_must_not_track() {
        let env = Env { host: String::new(), pid: 0, now: 0 };
        assert!(decide("SessionStart", &json!({"cwd": "/a"}), None, &env).is_none());
        assert!(decide(
            "sessionStart",
            &json!({"session_id": "empty-state-draft"}),
            None,
            &env
        )
        .is_none());
    }
}
