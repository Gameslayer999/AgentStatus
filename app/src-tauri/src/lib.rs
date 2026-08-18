// AgentStatus — Tauri backend.
// Reads the per-session status files written by the hook (decision 007) and
// exposes them to the frontend via the `list_sessions` command.

use serde::Serialize;
use std::path::PathBuf;
use tauri::Manager;
#[cfg(any(target_os = "macos", windows))]
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

// Release-only: the self-installer runs solely from the packaged app (see the
// `ensure_installed()` call in `run`, also `#[cfg(not(debug_assertions))]`). Gating
// the module to match keeps the whole thing out of dev builds, where it would
// otherwise compile as dead code (dev uses the repo hooks via `node hooks/setup.mjs`).
#[cfg(not(debug_assertions))]
mod install;

/// Tray icon id — used to fetch the tray (`app.tray_by_id`) from the mode/image
/// commands after it's built in `setup`.
#[cfg(any(target_os = "macos", windows))]
const TRAY_ID: &str = "agentstatus";
/// Window label of the settings window (decision 082). Also its capability name, so
/// the two must stay in step with `capabilities/settings.json`.
const SETTINGS_ID: &str = "settings";

#[derive(Serialize)]
struct SessionStatus {
    id: String,
    state: String,
    cwd: String,
    label: String,
    /// The host's own name for this session — Claude Code's session name, or Cursor's
    /// composer name. Empty when the host reports none. Tooltip only (decision 053).
    name: String,
    updated_at: i64,
    task: String,
    detail: String,
    /// Host surface ("cursor", "vscode", "cli", or "claude-desktop"), from the hook —
    /// drives click-to-focus and which liveness signal prunes the light (decision 054).
    ide: String,
    /// The application the session is running in, as the tooltip says it: "VS Code",
    /// "Cursor", "Claude Desktop", or the terminal emulator hosting a CLI session
    /// ("Ghostty", "Terminal"). Tooltip only (decision 060).
    app: String,
    /// agent_type of each currently-running subagent under this session.
    subagents: Vec<String>,
}

/// A session with no hook activity for this long is treated as dead/abandoned and
/// pruned. It self-heals: any real session re-registers on its next event. Chosen
/// long enough that a session you're actively dealing with (even blocked/errored,
/// which emit no further events while waiting) won't vanish out from under you.
const MAX_IDLE_SECS: i64 = 2 * 60 * 60;

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Epoch milliseconds — used as the focus-request token so two clicks in the same
/// second still read as distinct requests (see write_focus_request).
fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// The user's home directory. macOS and the shell hook use `HOME`; a Windows GUI process
/// has only `USERPROFILE` (`HOME` is set inside the hook, because Claude Code sets it, but
/// not for the app itself). Empty when neither is set — every caller already treats the
/// resulting path as one that simply does not exist.
fn home() -> String {
    for key in ["HOME", "USERPROFILE"] {
        if let Ok(v) = std::env::var(key) {
            if !v.is_empty() {
                return v;
            }
        }
    }
    String::new()
}

/// Root status directory (~/.claude/status), honoring $AGENTSTATUS_DIR (same
/// override the hook uses). $CLAUDESTATUS_DIR is kept as a legacy alias.
fn status_root() -> PathBuf {
    if let Ok(dir) = std::env::var("AGENTSTATUS_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(dir) = std::env::var("CLAUDESTATUS_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from(home()).join(".claude").join("status")
}

/// Directory holding one JSON file per session (status_root/sessions).
fn sessions_dir() -> PathBuf {
    status_root().join("sessions")
}

/// Hand a specific-session focus request to the per-window VS Code extension
/// (decision 015). The floating bar can raise the right *window* itself (the IDE
/// CLI, below) but cannot focus a specific session *tab* — that needs the in-editor
/// `claude-vscode.editor.open` command, which only the extension can call. The
/// `vscode://` deep link is the only external lever and it shows a consent popup on
/// every click (verified live), so instead the bar drops the target session id here
/// and the extension (which polls the status dir) focuses the tab, popup-free.
/// `requested_at` is epoch millis so each click is a distinct request.
fn write_focus_request(session_id: &str) {
    if session_id.is_empty() {
        return;
    }
    let dir = status_root();
    let _ = std::fs::create_dir_all(&dir);
    let body = serde_json::json!({
        "session_id": session_id,
        "requested_at": now_millis(),
    });
    let _ = std::fs::write(dir.join("focus-request.json"), body.to_string());
}

/// True if process `pid` currently exists. `kill(pid, 0)` delivers no signal — it
/// only probes: 0 = alive, EPERM = alive but owned by another user, ESRCH = gone.
#[cfg(target_os = "macos")]
fn pid_alive(pid: i64) -> bool {
    if pid <= 0 {
        return false;
    }
    if unsafe { libc::kill(pid as libc::pid_t, 0) } == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// Non-macOS builds have no pid-liveness probe, so they answer "alive" and the caller
/// keeps its timeout-only pruning — the same fail-open contract as `owns_terminal` and
/// `live_workspace_folders`. Never prune a light on an answer the platform cannot give
/// (UI Principle #4 cuts both ways: a missing light is as wrong as a lying one).
#[cfg(not(target_os = "macos"))]
fn pid_alive(_pid: i64) -> bool {
    true
}

/// Workspace folders of every **live** IDE window, from the lock files each IDE
/// window writes (~/.claude/ide/*.lock — `workspaceFolders` + owning `pid`). A lock
/// whose pid is dead is skipped, so a force-quit/crashed IDE that left its lock behind
/// stops keeping its sessions lit (decision 027). Returns empty when the ide dir is
/// missing/unreadable — callers read that as "no liveness signal" and fall back to the
/// idle timeout, so we never prune every light off one bad read or a no-IDE machine.
#[cfg(target_os = "macos")]
fn live_workspace_folders() -> Vec<String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let ide_dir = std::path::PathBuf::from(home).join(".claude").join("ide");
    let mut folders = Vec::new();
    let Ok(entries) = std::fs::read_dir(&ide_dir) else {
        return folders;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().and_then(|s| s.to_str()) != Some("lock") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&p) else { continue };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
        if let Some(pid) = v.get("pid").and_then(|x| x.as_i64()) {
            if !pid_alive(pid) {
                continue;
            }
        }
        if let Some(arr) = v.get("workspaceFolders").and_then(|x| x.as_array()) {
            for f in arr.iter().filter_map(|x| x.as_str()) {
                folders.push(f.to_string());
            }
        }
    }
    folders
}

/// Non-macOS builds have no IDE-lock liveness signal; the idle timeout alone prunes.
#[cfg(not(target_os = "macos"))]
fn live_workspace_folders() -> Vec<String> {
    Vec::new()
}

/// Whether `cwd` is `folder`, or a subfolder a session `cd`'d into.
///
/// On Windows the comparison is case-insensitive and accepts a backslash separator: the
/// filesystem is case-insensitive, and nothing guarantees the IDE lock file and the hook
/// payload spell the drive or folder the same way. macOS keeps the exact, `/`-only rule it
/// has always used, so its matching is unchanged (Agent Guideline #7).
fn path_within(cwd: &str, folder: &str) -> bool {
    // Normalise before comparing, or the match fails on differences that mean nothing: a
    // trailing separator in the lock file's `workspaceFolders` (which would otherwise
    // disable matching for that workspace entirely), a mix of `\` and `/` in the same pair
    // of paths, or a drive root written `C:\`. macOS keeps its exact, `/`-only rule.
    fn norm(p: &str) -> String {
        if !cfg!(windows) {
            return p.to_string();
        }
        let mut s = p.replace('\\', "/").to_ascii_lowercase();
        // Keep the slash on a drive root ("c:/"), or it becomes the bare drive letter and
        // stops looking like an absolute path.
        while s.len() > 1 && s.ends_with('/') && !s.ends_with(":/") {
            s.pop();
        }
        s
    }
    let (c, f) = (norm(cwd), norm(folder));
    if c == f {
        return true;
    }
    let Some(rest) = c.strip_prefix(&f) else { return false };
    // A drive root already ends in its separator, so the remainder starts a segment directly.
    rest.starts_with('/') || (cfg!(windows) && f.ends_with(":/") && !rest.is_empty())
}

/// True if `cwd` sits inside one of the live IDE workspace folders — an exact match,
/// or a subfolder a session `cd`'d into (same prefix rule as `workspace_root`). An
/// empty cwd matches nothing: it's an anonymous session no live window claims.
fn cwd_is_live(cwd: &str, folders: &[String]) -> bool {
    if cwd.is_empty() {
        return false;
    }
    folders.iter().any(|f| path_within(cwd, f))
}

/// agent_type of each currently-running subagent, read from the per-session
/// marker directory sessions/<id>.subagents/ (one file per subagent — race-free
/// under parallel subagents; decision 010).
fn read_subagents(dir: &std::path::Path, id: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir.join(format!("{id}.subagents"))) {
        for e in entries.flatten() {
            let t = std::fs::read_to_string(e.path()).unwrap_or_default();
            let t = t.trim();
            out.push(if t.is_empty() { "agent".to_string() } else { t.to_string() });
        }
    }
    out
}

/// How long the session-fact map is reused (seconds). This was 5 s when it carried only
/// the name — a name is fixed for the life of a session, so re-reading the directory every
/// poll would have been pointless I/O. Decision 067 added `status` to the same record, and
/// that changes on every turn boundary: the whole point is that an interrupted turn greys
/// its light *immediately*, so the read now happens at poll rate. It is a handful of small
/// JSON files in one directory — the same shape of read the bar already does every second
/// for the status files themselves.
const SESSION_FACTS_TTL: i64 = 1;

/// What Claude Code records about one of its own live sessions, in the per-process file it
/// keeps at `~/.claude/sessions/<pid>.json`.
#[derive(Clone, Debug, Default)]
struct ClaudeFact {
    /// The host's own name for the session ("agentstatus-5b"), for the tooltip (decision 053).
    name: String,
    /// What the session is doing *right now*, as Claude Code itself sees it. Observed values
    /// on 2.1.227: `busy` (burning a turn), `waiting` (stopped to ask the user), `idle` (at
    /// the prompt), `shell` (dropped to a shell). Empty when the key is absent, which is the
    /// case for every Claude Desktop session — those report no status at all.
    status: String,
    /// When `status` was last written, in **milliseconds**. This is what makes the reconcile
    /// safe: it says whether Claude Code's answer is newer than the hook event the light was
    /// drawn from, so a stale answer can never overrule an observed one.
    status_updated_ms: i64,
}

/// Claude Code's own name for each session, from the per-process record it writes at
/// startup (~/.claude/sessions/<pid>.json — `sessionId`, `name`, `nameSource`). A
/// derived name is the folder plus a short suffix ("agentstatus-5b"), which is what
/// tells two sessions in the same folder apart; a renamed session carries the user's
/// own name instead. Verified present for every live session on Claude Code 2.1.200
/// and 2.1.223. Returns empty when the directory is missing or unreadable — the
/// tooltip then just shows the folder, as before.
fn claude_session_facts() -> std::collections::HashMap<String, ClaudeFact> {
    let dir = std::path::PathBuf::from(home()).join(".claude").join("sessions");
    let mut map = std::collections::HashMap::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return map;
    };
    for e in entries.flatten() {
        let Ok(text) = std::fs::read_to_string(e.path()) else { continue };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
        let id = v.get("sessionId").and_then(|x| x.as_str()).unwrap_or("");
        if id.is_empty() {
            continue;
        }
        map.insert(
            id.to_string(),
            ClaudeFact {
                name: v.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                status: v.get("status").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                status_updated_ms: v
                    .get("statusUpdatedAt")
                    .and_then(|x| x.as_i64())
                    .unwrap_or(0),
            },
        );
    }
    map
}

/// `claude_session_facts` behind a TTL cache (same pattern as `cursor_facts`).
fn session_facts(now: i64) -> std::collections::HashMap<String, ClaudeFact> {
    type Cache = std::sync::Mutex<(i64, std::collections::HashMap<String, ClaudeFact>)>;
    static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new((0, Default::default())));
    let Ok(mut guard) = cache.lock() else {
        return Default::default();
    };
    if now - guard.0 >= SESSION_FACTS_TTL {
        *guard = (now, claude_session_facts());
    }
    guard.1.clone()
}

/// Whether a green light should be forced grey because Claude Code itself says the session
/// is no longer working (decision 067).
///
/// The hook can only write `idle` from a `Stop` event, so **any** turn that ends without one
/// leaves the light green forever — a turn the user interrupted with Ctrl-C or Esc being the
/// everyday case, a dropped end-of-life event (the observed 95-minute stuck green light) the
/// pathological one. Claude Code tracks the same fact independently, so the bar asks instead
/// of inferring — the #048 / #063 shape, applied to Claude Code's own hosts.
///
/// Two guards keep it from ever inventing a grey light (UI Principle #4):
///   * **positive evidence only** — the session must actually say `idle`. An absent status
///     (every Claude Desktop session), an unreadable record, or a session Claude Code does
///     not list changes nothing, so the failure mode is the pre-067 behaviour.
///   * **the answer must be newer than the light** — `statusUpdatedAt` (ms) has to fall in a
///     strictly later second than the hook event that drew the light, so anything the hook
///     actually observed wins over a stale answer. Strictly later, not merely greater,
///     because the hook stamps whole seconds (`date +%s`): within one shared second the two
///     clocks cannot be ordered, and the tie has to go to the hook. Costs up to 1 s of extra
///     latency to be sure a green light is never grey for a poll at the start of a turn.
///
/// `shell` is deliberately **not** treated as idle. It was observed on a live session for ten
/// unbroken minutes and plainly is not a running turn, but what produces it has not been
/// confirmed on this version, and Guideline #4 does not spend a lying light on a guess.
fn turn_ended(fact: Option<&ClaudeFact>, light_updated_at: i64) -> bool {
    fact.is_some_and(|f| {
        f.status == "idle" && f.status_updated_ms >= (light_updated_at + 1) * 1000
    })
}

/// Whether the Cursor app is alive. Cursor sessions are NOT tracked by the
/// `~/.claude/ide/*.lock` files (only Claude Code's VS Code extension writes those),
/// so lock-pruning would nuke every Cursor light the moment any VS Code window is
/// open. Instead, Cursor lights are dropped when Cursor itself has quit (no process).
/// Fails open (keep the lights) if pgrep can't run.
#[cfg(target_os = "macos")]
fn cursor_running() -> bool {
    std::process::Command::new("pgrep")
        .args(["-x", "Cursor"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(true)
}

/// Off macOS there is no `pgrep`, so this answered "alive" only after failing to spawn one —
/// once per poll, i.e. once a second, forever. Same answer, no process (Agent Guideline #3:
/// the bar must not busy the machine it is only supposed to be watching).
#[cfg(not(target_os = "macos"))]
fn cursor_running() -> bool {
    true
}

/// What Cursor itself says about a composer (decision 048), read from its own store.
#[derive(Clone, Default)]
struct CursorFacts {
    /// The user archived this agent — Cursor fires no `sessionEnd`, so only this says so.
    archived: bool,
    /// A subagent composer: it belongs to its parent agent's light, not one of its own.
    subagent: bool,
    /// The turn is over (`completed`/`aborted`) as far as Cursor's own record goes.
    terminal: bool,
    /// The composers this agent spawned as subagents. Cursor's `subagentStop` hook does not
    /// reliably fire, so the badge counts these (the ones still firing events) rather than
    /// the leftover marker files.
    sub_ids: Vec<String>,
    /// The composer's display name — the only handle Cursor's tray menu exposes for a row,
    /// so it's what the "is this agent still running?" veto below matches on.
    name: String,
}

/// How long a Cursor fact set is reused before re-querying (seconds). The poll runs
/// ~1×/s; a `sqlite3` spawn that often is pointless when archive/finish state changes
/// on human timescales.
const CURSOR_FACTS_TTL: i64 = 5;

/// How long a Cursor session must have gone without a hook event to count as silent
/// (seconds). A working agent fires a tool event every few seconds, and one waiting on a
/// subagent has that subagent's events standing in for it — so silence *plus* Cursor
/// calling the turn over is what makes a light reconcilable.
const CURSOR_STALE_SECS: i64 = 60;

/// Cursor's key-value store, which holds both the composer headers and the per-composer
/// records the lights are reconciled against.
#[cfg(target_os = "macos")]
fn cursor_state_db() -> Option<std::path::PathBuf> {
    Some(
        std::path::PathBuf::from(std::env::var("HOME").ok()?)
            .join("Library/Application Support/Cursor/User/globalStorage/state.vscdb"),
    )
}

/// Ask Cursor about these composers — archived, subagent, finished — in one query
/// (decision 048). `session_id` *is* `composerId` for a Cursor session, and its
/// `composerHeaders` row carries `isArchived`/`isSubagent` while `composerData` carries
/// the turn `status`. Returns None if the query can't run (no sqlite3, no db, unreadable
/// while Cursor writes), which the caller treats as "reconcile nothing" — Cursor's
/// record must never be *assumed* absent, or one bad read would clear the bar.
#[cfg(target_os = "macos")]
fn cursor_facts_query(ids: &[String]) -> Option<std::collections::HashMap<String, CursorFacts>> {
    let list = ids
        .iter()
        .filter(|id| id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        .map(|id| format!("'{id}'"))
        .collect::<Vec<_>>()
        .join(",");
    if list.is_empty() {
        return Some(std::collections::HashMap::new());
    }
    let out = std::process::Command::new("/usr/bin/sqlite3")
        .arg("-readonly")
        .arg(cursor_state_db()?)
        .arg(format!(
            "select h.composerId, h.isArchived, h.isSubagent, \
             coalesce(json_extract(d.value,'$.status'),''), \
             coalesce(json_extract(d.value,'$.subagentComposerIds'),''), \
             coalesce(replace(json_extract(d.value,'$.name'),'|',' '),'') \
             from composerHeaders h \
             left join cursorDiskKV d on d.key = 'composerData:'||h.composerId \
             where h.composerId in ({list});"
        ))
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let mut map = std::collections::HashMap::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let f: Vec<&str> = line.split('|').collect();
        if f.len() < 6 {
            continue;
        }
        map.insert(
            f[0].to_string(),
            CursorFacts {
                archived: f[1] == "1",
                subagent: f[2] == "1",
                terminal: f[3] == "completed" || f[3] == "aborted",
                // A JSON array of ids: pull the uuid-shaped tokens out of it.
                sub_ids: f[4]
                    .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
                    .filter(|s| s.len() >= 32)
                    .map(|s| s.to_string())
                    .collect(),
                name: f[5].trim().to_string(),
            },
        );
    }
    Some(map)
}

#[cfg(not(target_os = "macos"))]
fn cursor_facts_query(_ids: &[String]) -> Option<std::collections::HashMap<String, CursorFacts>> {
    None
}

/// `cursor_facts_query` behind a TTL cache, so the once-a-second poll spawns at most one
/// `sqlite3` every `CURSOR_FACTS_TTL` seconds. A failed query is cached too (as None) —
/// retrying it every poll would be the same spawn storm the cache exists to avoid.
fn cursor_facts(
    ids: &[String],
    now: i64,
) -> Option<std::collections::HashMap<String, CursorFacts>> {
    type Cache = std::sync::Mutex<(i64, Option<std::collections::HashMap<String, CursorFacts>>)>;
    static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new((0, None)));
    let Ok(mut guard) = cache.lock() else {
        return None;
    };
    if now - guard.0 >= CURSOR_FACTS_TTL {
        *guard = (now, cursor_facts_query(ids));
    }
    guard.1.clone()
}

#[tauri::command]
fn list_sessions() -> Vec<SessionStatus> {
    let mut out = Vec::new();
    let now = now_unix();
    let dir = sessions_dir();
    // Workspace folders of the currently-open IDE windows. A session whose folder
    // isn't among them has had its window closed (or never had one — an anonymous
    // ghost), so its light is stale (decision 027). Empty ⇒ no liveness signal, so
    // lock-pruning is skipped below and only the idle timeout applies.
    let live_folders = live_workspace_folders();
    let cursor_alive = cursor_running();
    // Read every status file first, so the Cursor reconciliation below can ask about all
    // of that host's composers in one query instead of one per light.
    let mut files: Vec<(std::path::PathBuf, String, serde_json::Value)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let id = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => continue,
            };
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            files.push((path, id, v));
        }
    }
    let cursor_ids: Vec<String> = files
        .iter()
        .filter(|(_, _, v)| v.get("ide").and_then(|x| x.as_str()) == Some("cursor"))
        .map(|(_, id, _)| id.clone())
        .collect();
    // When each session last reported, so a Cursor agent's subagents can be checked for
    // life below (their own lights are hidden, but their events still prove they're alive).
    let fresh_at: std::collections::HashMap<&str, i64> = files
        .iter()
        .map(|(_, id, v)| {
            (id.as_str(), v.get("updated_at").and_then(|x| x.as_i64()).unwrap_or(0))
        })
        .collect();
    let facts = if cursor_ids.is_empty() {
        None
    } else {
        cursor_facts(&cursor_ids, now)
    };
    let claude = session_facts(now);
    // Claude Code's own view of its live CLI sessions, consulted only when the bar actually
    // has one — it costs a subprocess, so a machine with no terminal sessions never pays.
    let cli_live = if files
        .iter()
        .any(|(_, _, v)| v.get("ide").and_then(|x| x.as_str()) == Some("cli"))
    {
        cli_facts(now)
    } else {
        None
    };
    {
        for (path, id, v) in &files {
            let (path, id) = (path.clone(), id.clone());
            let updated_at = v.get("updated_at").and_then(|x| x.as_i64()).unwrap_or(0);
            let cwd = v.get("cwd").and_then(|x| x.as_str()).unwrap_or("");
            // Prune dead sessions (delete the file + subagent markers, skip it; self-heals
            // on the session's next event) three ways:
            //   (a) window gone — the session's workspace maps to no live IDE lock, so its
            //       window was closed / the IDE quit. Instant, no waiting on the timer
            //       (decision 027). Skipped when no live lock exists at all so one bad read
            //       (or a no-IDE machine) never nukes every light.
            //   (b) cwd path gone — covers renamed/deleted project folders.
            //   (c) unclean death with the window still open, or a superseded session
            //       sharing a live window's lock: silent past MAX_IDLE_SECS (decision 004).
            //   (d) Cursor-specific (decision 038): Cursor writes NO ~/.claude/ide/*.lock,
            //       so it must NOT be lock-pruned (that deleted every Cursor light the
            //       moment any VS Code window was open). Its clean-close is covered by the
            //       bridged SessionEnd; unclean death by dropping all Cursor lights when
            //       Cursor has quit, plus the MAX_IDLE_SECS backstop.
            //   (e) CLI / Claude Desktop (decision 054): neither writes an IDE lock either,
            //       and both are excluded from (a) by their own `ide` value. Their liveness
            //       handle is the pid of the owning `claude` process, recorded by the hook —
            //       so a terminal closed or force-killed without a SessionEnd drops its light
            //       on the next poll instead of lingering to the backstop. `pid > 0` keeps
            //       status files written before this change on the idle timeout alone.
            let ide = v.get("ide").and_then(|x| x.as_str()).unwrap_or("vscode");
            let pid = v.get("pid").and_then(|x| x.as_i64()).unwrap_or(0);
            let window_gone = ide == "vscode" && !live_folders.is_empty() && !cwd_is_live(cwd, &live_folders);
            let cwd_gone = !cwd.is_empty() && !std::path::Path::new(cwd).exists();
            let cursor_gone = ide == "cursor" && !cursor_alive;
            let host_process_gone =
                matches!(ide, "cli" | "claude-desktop") && pid > 0 && !pid_alive(pid);
            //   (f) a pre-warmed spare (decision 054): a `claude bg-spare` process fires
            //       SessionStart, so a light appears, but it never becomes a session. Claude
            //       Code does not list it in `claude agents --json`, which is the only thing
            //       that separates it from a real background agent (identical argv, no tty
            //       either way). Only applied once the light has been silent a little while,
            //       so a genuinely new session is never raced before Claude registers it, and
            //       only when the query actually answered.
            //       Dropped **on sight** when the recorded pid also owns no terminal
            //       (decision 064): a spare never has one, so there is nothing to wait for,
            //       and the 20s grace was long enough for the phantom to be clicked — which
            //       is how one came to open Claude Code's agent view in a terminal window of
            //       its own. A pid *with* a tty is a real session however Claude Code lists
            //       it, so that case keeps the full grace period unchanged.
            let spare_light = ide == "cli"
                && cli_live
                    .as_ref()
                    .is_some_and(|facts| !facts.contains_key(id.as_str()))
                //       `pid > 0` matters: a status file written before 054 records no pid,
                //       so nothing is known about a terminal and it must keep the timeout
                //       (the same upgrade path (e) protects).
                && (now - updated_at >= CLI_UNLISTED_SECS || (pid > 0 && !owns_terminal(pid)));
            //   (g) a background agent that finished and was left behind (decision 065).
            //       Reported live: lights for sessions with "no currently active tab or
            //       terminal anywhere". A `--bg` job has no terminal by construction, and
            //       Claude Code keeps its process alive and still listed once the work is
            //       done — so (e)'s pid liveness never fires and (f) never applies, and the
            //       light sat there for the two-hour backstop. Retired on Claude Code's own
            //       word (`kind: background` + `status: idle`) once it has also been silent
            //       for CLI_BG_DONE_SECS, so "just finished" is still visible for a while.
            //       Never applied to a light the user has to act on: `blocked` and `error` go
            //       quiet by nature, which is exactly when a timeout would delete them
            //       (UI Principle #2). The hook can only say `blocked` for a permission
            //       prompt, and a `--bg` job stopping to ask the user a question fires `Stop`
            //       instead — so the light it wrote reads `idle` and this guard missed the
            //       case it exists for. Claude Code's own `state: blocked` covers it
            //       (decision 063); measured live, a job waiting on an answer lost its light
            //       five minutes in and got it back only when the answer arrived.
            let waiting = matches!(
                v.get("state").and_then(|x| x.as_str()).unwrap_or(""),
                "blocked" | "error"
            );
            let bg_abandoned = ide == "cli"
                && !waiting
                && now - updated_at >= CLI_BG_DONE_SECS
                && cli_live
                    .as_ref()
                    .and_then(|facts| facts.get(id.as_str()))
                    .is_some_and(bg_retirable);
            if window_gone || cwd_gone || cursor_gone || host_process_gone || spare_light
                || bg_abandoned
                || now - updated_at > MAX_IDLE_SECS
            {
                let _ = std::fs::remove_file(&path);
                let _ = std::fs::remove_dir_all(dir.join(format!("{id}.subagents")));
                continue;
            }
            let mut state = v.get("state").and_then(|x| x.as_str()).unwrap_or("idle").to_string();
            let mut detail = v.get("detail").and_then(|x| x.as_str()).unwrap_or("").to_string();
            // Reconcile a background agent's light against Claude Code's own job state
            // (decision 063), the #048 pattern applied to `--bg` jobs: the bar does not infer
            // what a silent job is doing, it asks. Only an `idle` light is touched, so
            // anything the hook actually observed — `running`, `blocked`, `error` — always
            // wins, and a job the user has just replied to is green from its own next event
            // rather than orange from a stale answer.
            if ide == "cli" && state == "idle" {
                if let Some(next) = cli_live
                    .as_ref()
                    .and_then(|facts| facts.get(id.as_str()))
                    .and_then(bg_light_state)
                {
                    state = next.to_string();
                }
            }
            // Grey a green light whose turn Claude Code says is over (decision 067). The
            // everyday case is a turn the user interrupted with Ctrl-C or Esc: the turn ends,
            // the session returns to the prompt, and no `Stop` fires — so the hook, whose only
            // route to `idle` is `Stop`, leaves the light green indefinitely. See `turn_ended`
            // for the two guards that keep this from ever inventing a grey light.
            //
            // Background agents are excluded: their `status` is `idle` between turns even
            // while the job is alive and working, so it does not mean the same thing there —
            // what a `--bg` light shows is decided by decisions 063 and 065 from `state`.
            // Claude Desktop is excluded by the data rather than by a rule: it writes no
            // `status`, so `turn_ended` reads no evidence and changes nothing.
            let is_background = cli_live
                .as_ref()
                .and_then(|facts| facts.get(id.as_str()))
                .is_some_and(|f| f.kind == "background");
            if ide != "cursor"
                && state == "running"
                && !is_background
                && turn_ended(claude.get(&id), updated_at)
            {
                state = "idle".to_string();
                // Grey, and specifically *not* the white "done" light. The frontend reads an
                // idle light carrying a `detail` as a finished turn with output to review
                // (decisions 014/050), and `detail` here is whatever the last tool event wrote
                // ("$ sleep 90") — so leaving it would raise an attention light on a session
                // the user is by definition already looking at, describing a tool call that was
                // cancelled. `detail` means one thing, the wrap-up message from `Stop`, and an
                // interrupted turn has none, so the honest value is empty. The `task` line (the
                // prompt) survives, which is the part still worth reading.
                detail.clear();
            }
            let mut cursor_subs: Option<Vec<String>> = None;
            // Reconcile Cursor lights against Cursor's own record (decision 048). Cursor's
            // bridged hooks are lossy at the end of a life: archiving an agent fires no
            // `sessionEnd`, and a subagent turn or an aborted one fires no `stop` — so a
            // light can sit green forever on an agent that finished, and an archived agent
            // keeps a light until the idle backstop. Only silent lights are touched
            // (CURSOR_STALE_SECS), and only when the query actually answered.
            if ide == "cursor" {
                if let Some(facts) = &facts {
                    let stale = now - updated_at >= CURSOR_STALE_SECS;
                    // The subagents Cursor says this agent spawned, keeping the ones still
                    // firing hook events of their own. `subagentStop` doesn't reliably fire
                    // — a marker for a finished subagent survived every poll and left a
                    // permanent "1 subagent running" badge on a finished agent — so for
                    // Cursor these replace the marker files, and they double as the proof
                    // that an agent sitting silent on "Running Task" is still working.
                    if let Some(f) = facts.get(&id) {
                        cursor_subs = Some(
                            f.sub_ids
                                .iter()
                                .filter(|sid| {
                                    fresh_at.get(sid.as_str()).is_some_and(|t| now - t < CURSOR_STALE_SECS)
                                })
                                .map(|_| "agent".to_string())
                                .collect(),
                        );
                    }
                    let subs_live = cursor_subs.as_ref().is_some_and(|s| !s.is_empty());
                    match facts.get(&id) {
                        // Archived, or (once silent) gone from Cursor's store entirely —
                        // the session no longer exists to go back to.
                        Some(f) if f.archived => {
                            let _ = std::fs::remove_file(&path);
                            let _ = std::fs::remove_dir_all(dir.join(format!("{id}.subagents")));
                            continue;
                        }
                        None if stale => {
                            let _ = std::fs::remove_file(&path);
                            let _ = std::fs::remove_dir_all(dir.join(format!("{id}.subagents")));
                            continue;
                        }
                        // A subagent belongs to its parent's light — which already counts it
                        // via the subagent markers — not to one of its own.
                        Some(f) if f.subagent => continue,
                        // Cursor says the turn ended and nothing — not the agent, not a
                        // subagent of its — has emitted anything since, but no `stop` ever
                        // reached us. Last check before greying a light: Cursor's tray row
                        // for this composer must not say it's *running right now*
                        // (decision 052). `status` on disk describes the composer's last
                        // *flushed* turn, so a live agent whose hooks simply went quiet —
                        // writing a big file, running a long command — reads as terminal,
                        // and greying it there is a lying light (UI Principle #4).
                        // `state != "idle"` comes before the tray read on purpose: an
                        // already-idle light cannot be changed by this arm, and the tray read
                        // is an AX walk into Cursor's live status-item menu, which **cancels
                        // that menu if the user has it open** (decision 038 saw the same on
                        // the shallower pip read and slowed it to 20s for exactly this). A
                        // settled Cursor light is terminal + stale forever, so without this
                        // the walk ran every CURSOR_FACTS_TTL seconds for as long as the
                        // light existed, and the menu became unclickable (decision 081).
                        Some(f)
                            if f.terminal
                                && stale
                                && !subs_live
                                && state != "error"
                                && state != "idle"
                                && !tray_says_running(&cursor_tray_titles_cached(now), &f.name) =>
                        {
                            state = "idle".to_string();
                        }
                        _ => {}
                    }
                }
            }
            // Cursor's badge comes from its own parent→subagent linkage (above); Claude
            // Code's from the marker files, whose Stop hook is reliable (decision 010).
            let subagents = cursor_subs.unwrap_or_else(|| read_subagents(&dir, &id));
            // The host's name for this session: Cursor's composer name comes from the
            // facts already queried above; Claude Code's from its session records.
            let name = if ide == "cursor" {
                facts.as_ref().and_then(|f| f.get(&id)).map(|f| f.name.clone()).unwrap_or_default()
            } else {
                claude.get(&id).map(|f| f.name.clone()).unwrap_or_default()
            };
            // Which application the user would be taken to — resolved here rather than in
            // the frontend because only the backend can walk a CLI session's process tree
            // to the emulator hosting it.
            let app = host_app(ide, &id, pid, cli_live.as_ref().and_then(|f| f.get(id.as_str())));
            out.push(SessionStatus {
                id,
                state,
                cwd: cwd.to_string(),
                label: v.get("label").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                name,
                updated_at,
                task: v.get("task").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                detail,
                ide: ide.to_string(),
                app,
                subagents,
            });
        }
    }
    // Stable order: by folder label, then id, so lights don't reshuffle each poll.
    out.sort_by(|a, b| a.label.cmp(&b.label).then_with(|| a.id.cmp(&b.id)));
    out
}

/// Convert the window into a non-activating NSPanel so it can float over other
/// apps' full-screen spaces without stealing focus or switching Spaces — the only
/// window type macOS lets sit over a third-party full-screen window.
#[cfg(target_os = "macos")]
fn make_overlay_panel(win: &tauri::WebviewWindow) {
    use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};
    use tauri_nspanel::WebviewWindowExt;

    let Ok(panel) = win.to_panel() else { return };

    // Above normal app windows.
    panel.set_level(4); // NSFloatingWindowLevel

    // Non-activating: showing the panel never becomes key, so it never yanks you
    // out of a full-screen app.
    #[allow(non_upper_case_globals)]
    const NS_NONACTIVATING_PANEL: i32 = 1 << 7;
    panel.set_style_mask(NS_NONACTIVATING_PANEL);

    // Appear on every Space, including over full-screen apps. Set through
    // objc2-app-kit on the underlying NSWindow rather than tauri-nspanel's
    // `set_collection_behaviour`, whose parameter is the deprecated `cocoa` crate
    // type. The panel is-a NSWindow (NSPanel inherits it), so this is the same object
    // and the same two flags — no deprecation, no behavior change.
    if let Ok(ptr) = win.ns_window() {
        // SAFETY: ns_window() hands back this window's live NSWindow (now reclassed to
        // an NSPanel, which inherits NSWindow); the app owns it for its whole lifetime.
        let ns_window: &NSWindow = unsafe { &*ptr.cast::<NSWindow>() };
        ns_window.setCollectionBehavior(
            NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::CanJoinAllSpaces,
        );
    }

    panel.show();
}

/// Resolve the IDE workspace root that contains `cwd` from the IDE lock files
/// (~/.claude/ide/*.lock), each of which lists its window's `workspaceFolders`. A
/// session that `cd`'d into a subfolder still maps back to the window that has the
/// *root* open (the raw subfolder path would otherwise open as its own new window).
/// Returns the longest matching workspace folder, or `cwd` unchanged if none match.
///
/// Not macOS-only: the Claude Code VS Code extension writes these locks on Windows too, and
/// the Windows raise (decision 070) needs the same cwd → window mapping. If the directory is
/// absent the function returns `cwd` unchanged, so a platform or setup without locks simply
/// gets the old behaviour rather than a wrong answer.
fn workspace_root(cwd: &str) -> String {
    let ide_dir = std::path::PathBuf::from(home()).join(".claude").join("ide");
    let mut best = String::new();
    if let Ok(entries) = std::fs::read_dir(&ide_dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) != Some("lock") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&p) else { continue };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
            let Some(folders) = v.get("workspaceFolders").and_then(|x| x.as_array()) else {
                continue;
            };
            for folder in folders {
                let Some(f) = folder.as_str() else { continue };
                if path_within(cwd, f) && f.len() > best.len() {
                    best = f.to_string();
                }
            }
        }
    }
    if best.is_empty() { cwd.to_string() } else { best }
}

/// Fast same-Space window raise (decision 021). The IDE CLI below is the correct,
/// cross-Space raise, but it boots a Node runtime on every click (~1.1s measured).
/// When the target window is on the *current* Space this osascript raise brings it
/// forward in ~0.2s. It goes through System Events (`set frontmost` + AXRaise), which
/// needs one permission — Accessibility — and no per-app Automation prompt. It can't
/// see full-screen windows on inactive Spaces, so it is strictly best-effort: we
/// always *also* fire the CLI, which handles the cross-Space / full-screen case.
/// Without an Accessibility grant this silently no-ops and the CLI alone runs (no
/// regression vs. the old behavior). The window is matched by the workspace-root
/// basename — the project folder, which appears in the IDE window title.
#[cfg(target_os = "macos")]
fn raise_window_fast(root: &str, ide: &str) {
    let name = std::path::Path::new(root)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if name.is_empty() {
        return;
    }
    let proc = if ide == "cursor" { "Cursor" } else { "Code" };
    // Escape for an AppleScript double-quoted string literal.
    let esc = name.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(
        "tell application \"System Events\" to tell process \"{proc}\"\n\
           set frontmost to true\n\
           set ws to (windows whose title contains \"{esc}\")\n\
           if (count of ws) > 0 then perform action \"AXRaise\" of item 1 of ws\n\
         end tell"
    );
    let _ = std::process::Command::new("osascript")
        .args(["-e", &script])
        .spawn();
}

/// Bring the first visible top-level window whose title satisfies `want` to the front, and
/// report whether one was found (decision 070).
///
/// The Windows counterpart to `raise_window_fast`'s osascript path. It needs no permission
/// grant — the Accessibility prompt macOS requires (#021/#039) has no analogue here — and it
/// is a direct Win32 call rather than a subprocess, so it is far below the ~1 s an IDE CLI
/// invocation costs. `SetForegroundWindow` is normally refused for a background process, but
/// this runs from a click on our own window, and Windows lets the foreground process hand
/// focus away. A minimised window is restored first, or it would be "raised" while staying
/// an icon.
#[cfg(windows)]
fn raise_window_titled(want: &dyn Fn(&str) -> bool) -> bool {
    match find_window(&|_pid, title| want(title)) {
        Some(hwnd) => raise(hwnd),
        None => false,
    }
}

/// Bring a window to the front, reporting whether it actually came forward.
///
/// `SetForegroundWindow` is refused — it returns 0 and merely flashes the taskbar button —
/// when the caller does not hold foreground rights: the foreground lock timeout, another app
/// grabbing focus first, or a target running at higher integrity (an editor started "as
/// administrator" while the bar is not) where UIPI blocks the activation outright. Reporting
/// that honestly is what lets `focus_session` fall through to the IDE's own CLI, which *can*
/// foreground itself. Claiming success would strand the click.
///
/// A minimised window is restored first, or it would be "raised" while staying an icon.
#[cfg(windows)]
fn raise(hwnd: windows_sys::Win32::Foundation::HWND) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE,
    };
    unsafe {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        SetForegroundWindow(hwnd) != 0
    }
}

/// The first visible top-level window whose owning pid and title satisfy `want`.
#[cfg(windows)]
fn find_window(want: &dyn Fn(u32, &str) -> bool) -> Option<windows_sys::Win32::Foundation::HWND> {
    let mut found = None;
    each_window(&mut |hwnd, pid, title| {
        if want(pid, title) {
            found = Some(hwnd);
            return false; // stop
        }
        true
    });
    found
}

/// Visit every visible top-level window that has a title, passing its handle, owning pid,
/// and title. Returning `false` from `visit` stops the enumeration.
#[cfg(windows)]
fn each_window(visit: &mut dyn FnMut(windows_sys::Win32::Foundation::HWND, u32, &str) -> bool) {
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
        IsWindowVisible,
    };

    // The EnumWindows callback returns a Win32 BOOL (an i32): non-zero continues the
    // enumeration, zero stops it. Spelled as i32 because windows-sys 0.61 no longer exports
    // a `BOOL` alias from Win32::Foundation.
    const CONTINUE: i32 = 1;
    const STOP: i32 = 0;

    type Visitor<'a> = &'a mut dyn FnMut(HWND, u32, &str) -> bool;

    unsafe extern "system" fn shim(hwnd: HWND, lparam: LPARAM) -> i32 {
        // SAFETY: `lparam` is the `&mut Visitor` handed to EnumWindows below, which outlives
        // the enumeration; EnumWindows calls this synchronously on one thread. The closure
        // must not panic — a panic across this `extern "system"` boundary aborts.
        let visit = unsafe { &mut *(lparam as *mut Visitor) };
        if unsafe { IsWindowVisible(hwnd) } == 0 {
            return CONTINUE;
        }
        let len = unsafe { GetWindowTextLengthW(hwnd) };
        if len <= 0 {
            return CONTINUE;
        }
        let mut buf = vec![0u16; len as usize + 1];
        let n = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
        if n <= 0 {
            return CONTINUE;
        }
        let mut pid: u32 = 0;
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        let title = String::from_utf16_lossy(&buf[..n as usize]);
        if visit(hwnd, pid, &title) {
            CONTINUE
        } else {
            STOP
        }
    }

    let mut visitor: Visitor = visit;
    // SAFETY: `shim` matches the EnumWindows callback signature and the pointer we pass is
    // valid for the duration of the call.
    unsafe { EnumWindows(Some(shim), &mut visitor as *mut Visitor as LPARAM) };
}

/// Every process's parent pid and lowercase image name, for walking a session's ancestry.
#[cfg(windows)]
fn process_tree() -> std::collections::HashMap<u32, (u32, String)> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let mut map = std::collections::HashMap::new();
    // SAFETY: a process snapshot borrows nothing; the handle is closed below.
    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snap == INVALID_HANDLE_VALUE {
        return map;
    }
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
            map.insert(entry.th32ProcessID, (entry.th32ParentProcessID, name));
            if unsafe { Process32NextW(snap, &mut entry) } == 0 {
                break;
            }
        }
    }
    unsafe { CloseHandle(snap) };
    map
}

/// Focus the window that hosts a session with no window of its own (decision 071).
///
/// The `claude` process the hook records owns no window — it is a console program inside a
/// terminal, or a child of Claude Desktop. Measured ancestry on Windows:
/// `claude.exe → powershell.exe → WindowsTerminal.exe`, and
/// `claude.exe → claude.exe (Claude Desktop's own window)`. So walk up from the recorded pid
/// and raise the first ancestor that owns a visible titled window.
///
/// This is the window, not the tab — Windows Terminal exposes no way to select a tab, so
/// that ceiling stands (#069). But the window is far better than the nothing a `cli` light
/// did before, which is exactly the click-that-goes-nowhere UI Principle #3 forbids.
///
/// One host process can own **several** windows: Windows Terminal runs every window of an
/// instance in one `WindowsTerminal.exe`, so three terminals with a Claude session each share
/// one pid and the ancestor walk lands on all three at once. Which one comes forward is then
/// decided by title, not by enumeration order (decision 077) — see `pick_window`.
#[cfg(windows)]
fn focus_host_window(pid: i64, session_id: &str) -> bool {
    match host_window(pid, session_id) {
        Some((hwnd, _title)) => raise(hwnd),
        None => false,
    }
}

/// The window `focus_host_window` would raise, and its title. Split from the raise so the
/// choice can be checked against live sessions without `SetForegroundWindow` — which a test
/// binary cannot make succeed, since it is refused for a process that is not the foreground
/// one, and a failed raise would otherwise mask a correct choice.
#[cfg(windows)]
fn host_window(
    pid: i64,
    session_id: &str,
) -> Option<(windows_sys::Win32::Foundation::HWND, String)> {
    if pid <= 0 {
        return None;
    }
    let tree = process_tree();

    // The recorded pid must still *be* a Claude Code process. Windows recycles pids
    // aggressively and `th32ParentProcessID` is never cleared when a parent exits, so a
    // stale pid can point at something else entirely — without this check a click could
    // walk into an unrelated application's tree and raise its window.
    match tree.get(&(pid as u32)) {
        Some((_, name)) if name == "claude.exe" => {}
        _ => return None,
    }

    // The ancestor chain, nearest first. It stops at the shell: `explorer.exe` owns the
    // permanently visible, permanently titled "Program Manager" window, so walking into it
    // would match *always* — focusing the desktop and reporting success, which would also
    // suppress the callers' own fallbacks. Anything above the shell is likewise not a host.
    let mut chain = Vec::new();
    let mut cur = pid as u32;
    for _ in 0..12 {
        chain.push(cur);
        match tree.get(&cur) {
            Some(&(parent, _)) if parent != 0 && parent != cur => {
                match tree.get(&parent) {
                    Some((_, name)) if name == "explorer.exe" => break,
                    Some(_) => cur = parent,
                    None => break,
                }
            }
            _ => break,
        }
    }

    // One sweep for the whole chain rather than one per ancestor: each sweep walks every
    // visible top-level window and reads its title. Every window each ancestor owns is kept,
    // not just the first — one process can own several, and picking between them is the whole
    // problem when a machine has more than one terminal open (decision 077).
    let mut owned: std::collections::HashMap<
        u32,
        Vec<(windows_sys::Win32::Foundation::HWND, String)>,
    > = std::collections::HashMap::new();
    each_window(&mut |hwnd, owner, title| {
        if chain.contains(&owner) {
            owned.entry(owner).or_default().push((hwnd, title.to_string()));
        }
        true // visit them all: the nearest ancestor is chosen afterwards, not the first seen
    });

    // Nearest ancestor that owns a window wins — the terminal hosting this session, not
    // whatever is further up the tree.
    for candidate in &chain {
        let Some(windows) = owned.get(candidate) else {
            continue;
        };
        // Read the session title only when the choice is actually ambiguous: it costs a
        // directory scan plus a transcript read on the click path.
        let session_title = if windows.len() > 1 {
            claude_ai_title(session_id)
        } else {
            None
        };
        let titles: Vec<String> = windows.iter().map(|(_, t)| t.clone()).collect();
        return pick_window(&titles, session_title.as_deref()).map(|i| windows[i].clone());
    }
    None
}

/// Which of the windows a host process owns is showing `session_title`, as an index into
/// `titles` (enumeration order, so index 0 is the topmost window).
///
/// Split out from `focus_host_window` so the rule that decides where a click lands is
/// testable without a live terminal, exactly as `editor_title_matches` is.
///
/// Claude Code keeps the terminal's title set to its own session title while a session is
/// running, prefixed with an activity glyph — the live windows here read
/// `◐ Fix Windows orange input detection`, `✳ Extend app support for older macOS versions`.
/// So the same two grades of match as Ghostty on macOS (decision 066): a title that **ends
/// with** the session title is showing that session, while one that merely **contains** it
/// may be showing something else whose title spans it, so that grade must be unambiguous.
///
/// With nothing to disambiguate on — no `ai-title` yet, or the session sitting in a
/// background tab whose title the window does not show — this answers `None` and the click
/// does nothing, rather than raising whichever window happens to be topmost. A wrong window
/// is worse than none (UI Principle #4); it is also what the user sees as the bug.
#[cfg(windows)]
fn pick_window(titles: &[String], session_title: Option<&str>) -> Option<usize> {
    if titles.len() < 2 {
        // A single window is this host's window by construction, titled or not.
        return titles.first().map(|_| 0);
    }
    let want = session_title?;
    if let Some(i) = titles.iter().position(|t| t.trim_end().ends_with(want)) {
        return Some(i);
    }
    let mut weak = titles.iter().enumerate().filter(|(_, t)| t.contains(want));
    match (weak.next(), weak.next()) {
        (Some((i, _)), None) => Some(i),
        _ => None,
    }
}

/// Raise the IDE window that has `root` open, matched the same way as on macOS: by the
/// project folder's basename, which both editors put in the window title. The trailing app
/// name keeps a same-named window of some other application from stealing the click.
#[cfg(windows)]
fn raise_window_fast(root: &str, ide: &str) -> bool {
    let name = std::path::Path::new(root)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if name.is_empty() {
        return false;
    }
    // Editor window titles are " - "-separated: "main.rs - App - Visual Studio Code", or
    // "App - Visual Studio Code" with no editor open. Match a **whole segment**, not a
    // substring: `contains("App")` also matches a sibling project called "AppOther", and
    // since a successful raise stops the caller from running its CLI fallback, a wrong match
    // is not self-correcting here the way it is on macOS (which always fires both).
    //
    // The trailing app name keeps a same-named window of another application from taking the
    // click. Insiders spells itself "Visual Studio Code - Insiders", so match the prefix.
    let app = if ide == "cursor" { "Cursor" } else { "Visual Studio Code" };
    raise_window_titled(&|title: &str| editor_title_matches(title, name, app))
}

/// Whether an editor window title belongs to project `folder` running in `app`. Split out
/// from `raise_window_fast` so the matching rule — the part that decides where a click
/// lands — is testable without a live editor.
#[cfg(windows)]
fn editor_title_matches(title: &str, folder: &str, app: &str) -> bool {
    let app_ok = title.ends_with(app) || title.contains(&format!("{app} - "));
    app_ok && title.split(" - ").any(|segment| segment == folder)
}

/// Where VS Code installs on Windows — per-user first, since that is the default the
/// installer offers. None when it isn't installed, which callers treat as "do nothing".
#[cfg(windows)]
fn vscode_exe() -> Option<PathBuf> {
    [
        ("LOCALAPPDATA", r"Programs\Microsoft VS Code\Code.exe"),
        ("ProgramFiles", r"Microsoft VS Code\Code.exe"),
        ("ProgramFiles(x86)", r"Microsoft VS Code\Code.exe"),
        // Insiders, so an Insiders-only machine still gets a working fallback rather than a
        // light that does nothing.
        (
            "LOCALAPPDATA",
            r"Programs\Microsoft VS Code Insiders\Code - Insiders.exe",
        ),
        (
            "ProgramFiles",
            r"Microsoft VS Code Insiders\Code - Insiders.exe",
        ),
    ]
    .iter()
    .filter_map(|(var, rest)| std::env::var(var).ok().map(|p| PathBuf::from(p).join(rest)))
    .find(|p| p.is_file())
}

/// Focus `root` in VS Code through the app itself — the fallback when no window title
/// matched, because the folder is open under a title we didn't recognise or isn't open at
/// all. VS Code forwards the request to a running instance and reuses the window that
/// already has the folder open.
///
/// Deliberately the `.exe` rather than the `code` shim: the shim is `code.cmd`, which
/// `CreateProcess` cannot run directly, so it would need `cmd /c` and the nested quoting
/// that comes with a path containing spaces. This is also what macOS does — it invokes the
/// CLI binary by absolute path rather than going through a shell.
#[cfg(windows)]
fn open_in_vscode(root: &str) {
    if let Some(exe) = vscode_exe() {
        let _ = std::process::Command::new(exe).arg(root).spawn();
    }
}

/// The controlling terminal of a process, as a device path ("/dev/ttys000") — the
/// key Terminal.app publishes per tab. None when the process has no tty (`??`), which
/// is every non-CLI host.
#[cfg(target_os = "macos")]
fn tty_of(pid: i64) -> Option<String> {
    let out = std::process::Command::new("ps")
        .args(["-o", "tty=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let t = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if t.is_empty() || t == "??" {
        return None;
    }
    Some(format!("/dev/{t}"))
}

/// Whether `pid` has a controlling terminal — the one thing a pre-warmed spare never has
/// (decision 064). Off macOS there is no `tty_of`, so it answers `true` and the caller keeps
/// its timeout-only behaviour rather than pruning on an answer this platform cannot give.
#[cfg(target_os = "macos")]
fn owns_terminal(pid: i64) -> bool {
    pid > 0 && tty_of(pid).is_some()
}

#[cfg(not(target_os = "macos"))]
fn owns_terminal(_pid: i64) -> bool {
    true
}

/// Walk up from `pid` to the first ancestor that is a macOS app bundle and return its name
/// **and pid** — the terminal emulator hosting this CLI session, and which *instance* of it.
/// Verified on a live Terminal.app session: `-zsh` → `login` →
/// `…/Terminal.app/Contents/MacOS/Terminal`.
///
/// The pid matters: a terminal can have several instances running at once (Ghostty does,
/// since a background-agent attach has to launch one), and `open -a <name>` cannot say which
/// one it means — it activated the wrong window and looked like the click had failed.
#[cfg(target_os = "macos")]
fn terminal_app_of(pid: i64) -> Option<(String, i64)> {
    let mut cur = pid;
    for _ in 0..12 {
        if cur <= 1 {
            break;
        }
        let out = std::process::Command::new("ps")
            .args(["-o", "ppid=,comm=", "-p", &cur.to_string()])
            .output()
            .ok()?;
        let line = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let (ppid_s, comm) = line.split_once(char::is_whitespace)?;
        if let Some(i) = comm.trim().find(".app/Contents/MacOS/") {
            let name = std::path::Path::new(&comm.trim()[..i + 4])
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())?;
            return Some((name, cur));
        }
        cur = ppid_s.trim().parse().ok()?;
    }
    None
}

/// `terminal_app_of` memoized per session. The tooltip names the emulator on every poll
/// (1 s) while the walk costs one `ps` per process generation, so it is resolved once per
/// session and reused. Keyed by session id **and** pid: the `claude` process owning a
/// session never changes, so the only way a cached name could go stale is that pid being
/// recycled under the same session id, which cannot happen; a differing pid recomputes.
#[cfg(target_os = "macos")]
fn terminal_app_cached(session_id: &str, pid: i64) -> Option<String> {
    type Cache = std::sync::Mutex<std::collections::HashMap<String, (i64, Option<String>)>>;
    static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();
    if pid <= 0 {
        return None;
    }
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    let mut guard = cache.lock().ok()?;
    if let Some((cached_pid, name)) = guard.get(session_id) {
        if *cached_pid == pid {
            return name.clone();
        }
    }
    let name = terminal_app_of(pid).map(|(app, _)| app);
    guard.insert(session_id.to_string(), (pid, name.clone()));
    name
}

#[cfg(not(target_os = "macos"))]
fn terminal_app_cached(_session_id: &str, _pid: i64) -> Option<String> {
    None
}

/// The application a session is running in, for the tooltip (decision 060). For the IDE
/// hosts the `ide` field already is the answer and this only spells it the way the app is
/// named on screen; for a terminal session it is the emulator actually hosting it
/// ("Ghostty", "Terminal", "iTerm2"), which nothing in the status file records — the `cli`
/// tag covers every emulator at once.
///
/// A detached background agent is named as one rather than by an app: it has no terminal by
/// construction, and the ancestor walk would find Claude Code's own launcher, so any app
/// name there would be a lie (UI Principle #4). Interactive-vs-background is decided exactly
/// as `focus_session` decides where a click goes, so the tooltip and the click agree.
fn host_app(ide: &str, session_id: &str, pid: i64, fact: Option<&CliFact>) -> String {
    match ide {
        "vscode" => "VS Code".to_string(),
        "cursor" => "Cursor".to_string(),
        "claude-desktop" => "Claude Desktop".to_string(),
        "cli" => {
            let interactive = match fact {
                Some(f) => f.kind != "background",
                None => owns_terminal(pid),
            };
            if !interactive {
                return "background agent".to_string();
            }
            terminal_app_cached(session_id, pid).unwrap_or_else(|| "terminal".to_string())
        }
        other => other.to_string(),
    }
}

/// Claude Code's own title for a session — the text it puts in the terminal's title bar,
/// and the only handle that tells two Ghostty surfaces apart. It is written into the
/// session transcript as an `ai-title` record and rewritten as the subject of the session
/// changes, so the last one is the current title. Absent until Claude has titled the
/// session (verified on 2.1.231: 0 records in a session that had run one prompt, 11 in a
/// working one) — and decision 053 found *no* title record of any kind on 2.1.223, so this
/// is version-dependent and every caller must degrade when it is missing (Guideline #4).
///
/// Reading a transcript is a deliberate exception to Guideline #5, taken because the title
/// is the only per-surface identifier Ghostty publishes. Only the title is pulled out of
/// the file; nothing is stored, and the string is already on screen in the tab it names.
/// Transcripts reach a few MB, so a file past `MAX_TRANSCRIPT` is skipped rather than read
/// on the click path.
fn claude_ai_title(session_id: &str) -> Option<String> {
    const MAX_TRANSCRIPT: u64 = 16 * 1024 * 1024;
    // The id becomes a path component, so accept only the uuid alphabet.
    if session_id.is_empty() || !session_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return None;
    }
    let home = home();
    if home.is_empty() {
        return None;
    }
    let projects = std::path::PathBuf::from(home).join(".claude").join("projects");
    // One directory per project folder; the session's transcript is under whichever one
    // it belongs to, named by session id. Cheaper and more robust than re-deriving the
    // directory name from `cwd` with Claude Code's own slug rule.
    let file = format!("{session_id}.jsonl");
    let path = std::fs::read_dir(&projects)
        .ok()?
        .flatten()
        .map(|e| e.path().join(&file))
        .find(|p| p.is_file())?;
    if std::fs::metadata(&path).ok()?.len() > MAX_TRANSCRIPT {
        return None;
    }
    let text = std::fs::read_to_string(&path).ok()?;
    text.lines()
        // Cheap reject first: only the handful of title records are worth parsing.
        .filter(|l| l.contains("\"ai-title\""))
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|v| v.get("type").and_then(|x| x.as_str()) == Some("ai-title"))
        .filter_map(|v| v.get("aiTitle").and_then(|x| x.as_str()).map(str::to_string))
        .filter(|s| !s.is_empty())
        .last()
}

/// Focus the exact Ghostty surface — tab *or* split — running this session (decision 055).
/// Ghostty 1.3 ships an AppleScript dictionary whose `terminal` objects expose a title and
/// a `focus` command that selects the surface and fronts its window in one step. It
/// publishes no tty and no pid, so the session is found by its title: Claude Code writes
/// its session title into the terminal title bar, so the two join on that string —
/// `Fix Ghostty tab focus when clicking light` on the bar matched
/// `◑ Fix Ghostty tab focus when clicking light` in Ghostty, the leading glyph being the
/// activity spinner. Hence `contains`, not equality.
///
/// An untitled session (no `ai-title` yet) is never matched: its terminal reads the generic
/// "Claude Code", which names nothing.
///
/// `require_unique` decides what *several* matching surfaces mean, and the two callers want
/// opposite things (decision 061):
///
/// - **true** — the click on an interactive session's light. Exactly one surface must match,
///   because the alternative is merely fronting the app, and a wrong tab is worse than no tab
///   (UI Principle #4).
/// - **false** — the click on a background agent, where the alternative is *opening another
///   terminal*. Any surface already showing this session beats creating one more, so the first
///   hit is taken. Several hits is the normal state there: an attached agent's tab carries the
///   same session title as every other view of it.
#[cfg(target_os = "macos")]
fn focus_ghostty_surface(session_id: &str, require_unique: bool) -> bool {
    let Some(title) = claude_ai_title(session_id) else {
        return false;
    };
    // Two grades of match (decision 066). Claude Code writes the title as a trailing run —
    // "◑ Fix Ghostty tab focus", the glyph being the activity spinner — so a surface whose
    // title **ends with** the session title is showing that session, and several such
    // surfaces are several views of the *same* session; taking the first is right by
    // construction. A surface that merely *contains* it may be showing something else whose
    // title happens to span it, so that grade still has to be unambiguous to act on.
    //
    // The distinction is what stops one session's title being a substring of another's from
    // silently killing the click: on `contains` alone a bystander surface counted as a second
    // hit, the match declined, and the fallback (front Ghostty) is invisible when Ghostty is
    // already frontmost — the click simply appeared dead.
    const SCRIPT: &str = r#"on run argv
  set target to item 1 of argv
  set uniqueOnly to (item 2 of argv) is "1"
  tell application "Ghostty"
    set strong to {}
    set weak to {}
    repeat with t in terminals
      set n to (name of t)
      if n ends with target then
        set end of strong to t
      else if n contains target then
        set end of weak to t
      end if
    end repeat
    if (count of strong) > 0 then
      focus (item 1 of strong)
      return "ok"
    end if
    if (count of weak) is 0 then return "no"
    if uniqueOnly and (count of weak) > 1 then return "no"
    focus (item 1 of weak)
    return "ok"
  end tell
end run"#;
    std::process::Command::new("osascript")
        .args(["-e", SCRIPT, &title, if require_unique { "1" } else { "0" }])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "ok")
        .unwrap_or(false)
}

/// Focus a CLI session in its terminal (decision 054). Terminal.app publishes a `tty`
/// per tab, so the exact tab running the session is selected and raised. Ghostty is
/// matched by session title through its own scripting dictionary (decision 055). Every
/// other emulator gets app-level focus: iTerm2 is not installed here to verify against,
/// so it gets no tab-precise code that has not been tested with it (Guideline #4).
#[cfg(target_os = "macos")]
fn focus_terminal_session(pid: i64, tty: &str, session_id: &str) {
    let Some((app, app_pid)) = terminal_app_of(pid) else {
        return;
    };
    if app == "Terminal" {
        const SCRIPT: &str = r#"on run argv
  set target to item 1 of argv
  tell application "Terminal"
    repeat with w in windows
      repeat with t in tabs of w
        if (tty of t) is target then
          set selected of t to true
          set frontmost of w to true
          activate
          return "ok"
        end if
      end repeat
    end repeat
  end tell
  return "no"
end run"#;
        let matched = std::process::Command::new("osascript")
            .args(["-e", SCRIPT, tty])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "ok")
            .unwrap_or(false);
        if matched {
            return;
        }
    }
    if app == "Ghostty" && focus_ghostty_surface(session_id, true) {
        return;
    }
    // Another emulator, or a tab we could not match. Land in the right app — and, when the
    // emulator has several instances running, the right *instance*: activate the exact
    // process that owns this session, which `open -a <name>` cannot express.
    const ACTIVATE: &str = r#"on run argv
  tell application "System Events"
    set frontmost of (first process whose unix id is (item 1 of argv as integer)) to true
  end tell
  return "ok"
end run"#;
    let activated = std::process::Command::new("osascript")
        .args(["-e", ACTIVATE, &app_pid.to_string()])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "ok")
        .unwrap_or(false);
    if !activated {
        // No Accessibility grant (or the process vanished) — fall back to the app by name.
        let _ = std::process::Command::new("open").args(["-a", &app]).spawn();
    }
}

/// Absolute path to the `claude` binary (decision 064).
///
/// Nothing may assume `claude` is on a PATH. The bar is a GUI app: launched from Login Items
/// — the way the README tells users to run it — it inherits launchd's
/// `/usr/bin:/bin:/usr/sbin:/sbin` and nothing else, and a terminal Ghostty opens for a
/// scripted command gets the same. Going through a login shell does *not* rescue it, which is
/// what decision 054 assumed: `zsh -lc` is **non-interactive**, so it reads `.zshenv` /
/// `.zprofile` / `.zlogin` but never `.zshrc` — and `.zshrc` is where the installer's
/// `~/.local/bin` lands. Verified both ways: `env -i PATH=/usr/bin:/bin zsh -lc 'claude …'`
/// gives "command not found", while the same bare environment runs the absolute path fine.
/// The reason this was never seen is that a shell-launched app (`./install.sh` relaunches it
/// that way) inherits the developer's full PATH and works — the failure only shows up on the
/// launch method every real user has.
///
/// Order: whatever the inherited PATH resolves (correct when the app *was* started from a
/// shell, and honors a non-standard install), then the locations Claude Code installs to.
/// Falls back to the bare name, which is no worse than before.
fn claude_bin() -> String {
    static CACHE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    CACHE
        .get_or_init(|| {
            if let Ok(out) = std::process::Command::new("/usr/bin/which").arg("claude").output() {
                let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !p.is_empty() && std::path::Path::new(&p).is_file() {
                    return p;
                }
            }
            let home = std::env::var("HOME").unwrap_or_default();
            for c in [
                format!("{home}/.local/bin/claude"),
                "/opt/homebrew/bin/claude".to_string(),
                "/usr/local/bin/claude".to_string(),
            ] {
                if std::path::Path::new(&c).is_file() {
                    return c;
                }
            }
            "claude".to_string()
        })
        .clone()
}

/// Open a **detached background agent** (`claude --bg`) in a real terminal — the only way
/// to reach one, since it has no terminal of its own to focus. `claude attach` is Claude
/// Code"s own verb for this, and it is safe to use on a live agent: "Open the background
/// session in this terminal ... The session keeps running either way."
///
/// `attach` takes the **short** id — the session uuid up to the first dash. The full uuid is
/// rejected ("No job matching ..."), verified live, so the id is truncated here.
///
/// Callers must have established that a session actually exists first: `attach` on an id with
/// no live job lands in Claude Code's agent view — a list of every session — rather than
/// failing, so calling it speculatively produces a terminal full of the wrong thing
/// (decision 064).
#[cfg(target_os = "macos")]
fn attach_background_agent(session_id: &str) {
    let short = session_id.split('-').next().unwrap_or("");
    // The id is interpolated into a shell command, so accept only a plain hex id.
    if short.len() < 6 || !short.chars().all(|c| c.is_ascii_hexdigit()) {
        return;
    }
    let ghostty_up = std::process::Command::new("pgrep")
        .args(["-x", "ghostty"])
        .output()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);
    if ghostty_up {
        // Reach the agent where it is already open before opening anywhere new (decision 061).
        // Without this the light had no memory: every click ran `attach` again, so clicking a
        // background agent three times left three terminals all attached to it. A click is
        // "take me to that session", never "give me another copy of it" (UI Principle #3).
        if focus_ghostty_surface(session_id, false) {
            return;
        }
        // No surface names it — but an attach can be running in one Claude Code has not
        // titled yet, and that is exactly the young tab a second click would duplicate. Its
        // argv names the session, so ask for it rather than adding to the pile; landing on
        // Ghostty with the tab already there beats a second one.
        let already = std::process::Command::new("pgrep")
            .args(["-f", &format!("claude attach {short}")])
            .output()
            .map(|o| !o.stdout.is_empty())
            .unwrap_or(false);
        if already {
            let _ = std::process::Command::new("open").args(["-a", "Ghostty"]).spawn();
            return;
        }
        // The absolute path, not a login shell: the shell a terminal opens for a scripted
        // command carries launchd's bare PATH and cannot find `claude` (see `claude_bin`).
        // Ghostty parses this string shell-style, so the path is quoted.
        let cmd = format!("\"{}\" attach {short}", claude_bin());
        // Put the agent in a **tab of the Ghostty already running** (decision 064). Decision
        // 054 had to use `open -na Ghostty`, which starts a whole second instance of the app:
        // Ghostty 1.2.x reported success for every scripted way of starting a surface and
        // started none. Ghostty 1.3 ships a working scripting dictionary, so the tab can be
        // made where the user is already working — and the second instance is what made
        // decision 055's tab focus unreachable for anything running in it.
        const NEW_TAB: &str = r#"on run argv
  tell application "Ghostty"
    if (count of windows) is 0 then
      new window with configuration {command:(item 1 of argv)}
    else
      new tab in front window with configuration {command:(item 1 of argv)}
    end if
    activate
  end tell
  return "ok"
end run"#;
        let opened = std::process::Command::new("osascript")
            .args(["-e", NEW_TAB, &cmd])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "ok")
            .unwrap_or(false);
        if opened {
            return;
        }
        // Ghostty older than 1.3 has no dictionary to answer that. Fall back to decision
        // 054's second instance — clumsy, but it does reach the agent. `-e` takes the command
        // as argv, so the absolute path goes in as its own argument.
        let _ = std::process::Command::new("open")
            .args(["-na", "Ghostty", "--args", "-e", &claude_bin(), "attach", short])
            .spawn();
        return;
    }
    // Terminal.app is always present. `do script` runs its argument in a shell, so the path
    // is single-quoted there.
    const SCRIPT: &str = r#"on run argv
  tell application "Terminal"
    do script ("'" & (item 1 of argv) & "' attach " & (item 2 of argv))
    activate
  end tell
end run"#;
    let _ = std::process::Command::new("osascript")
        .args(["-e", SCRIPT, &claude_bin(), short])
        .spawn();
}

/// What Claude Code itself says about a live CLI session: whether it is an interactive
/// terminal session or a detached background agent, and the pid that actually owns it.
#[derive(Clone, Debug)]
struct CliFact {
    kind: String,
    /// Read on macOS, where a click walks this pid to the terminal that owns the session
    /// (decision 060/054). The Windows path resolves the pid from the status file instead
    /// (decision 071), so the field is genuinely dead there — hence the targeted allow,
    /// rather than one that would also hide it going unused on macOS.
    #[cfg_attr(windows, allow(dead_code))]
    pid: i64,
    /// Claude Code's own word on whether this session is working right now — `busy` or
    /// `idle`. Absent for a host that reports none (Claude Desktop), which reads as neither.
    status: String,
    /// A background job's own lifecycle word — `working`, `done` or `blocked` (decision 063).
    /// Independent of `status`: `status` is live (is it burning a turn right now), `job_state`
    /// is what the job last declared about itself. `blocked` means it will not move without
    /// the user, which covers *two* situations — it asked a question, and it is sitting at an
    /// empty prompt — separated by `needs` (decision 084). Empty for interactive sessions,
    /// which report none.
    job_state: String,
    /// What a `blocked` job says it is waiting for, from its own record at
    /// `~/.claude/jobs/<id>/state.json`: the question it stopped to ask, verbatim, or the
    /// literal `send a prompt to start` when nothing has been asked of the user at all
    /// (decision 084). Empty for anything that reports none.
    needs: String,
}

/// Whether a background job's light may be retired by the silence timer: Claude Code says the
/// job is idle and it is not waiting on the user. `blocked` is the one answer that must keep a
/// light — the job stopped to ask something, so it goes silent by nature, and its light is the
/// only place that question is visible (UI Principle #2) — unless that `blocked` is only an
/// empty prompt, which asks the user nothing and retires like any other finished job (#084).
fn bg_retirable(f: &CliFact) -> bool {
    f.kind == "background"
        && f.status == "idle"
        && (f.job_state != "blocked" || bg_unprompted(f))
}

/// The `needs` a background job reports when it is idle at an empty prompt: it is not asking
/// anything, it is waiting to be given work. Every other `needs` on a `blocked` job is the
/// question it stopped to ask, verbatim (both measured live — see decision 084).
const BG_NEEDS_PROMPT: &str = "send a prompt to start";

/// Whether a `blocked` background job is merely unprompted rather than waiting on an answer.
/// Deliberately an exact match against the one phrase that means "nothing is being asked of
/// you": an unrecognised `needs` — a new wording, a job with no record on disk — keeps
/// decision 063's orange, because a missed attention light is the costlier mistake.
fn bg_unprompted(f: &CliFact) -> bool {
    f.needs == BG_NEEDS_PROMPT
}

/// The state a background agent's light should show when its own hook last wrote `idle`
/// (decision 063), or None to leave the light as the hook wrote it. Hooks describe *turns*, so
/// a turn that ended because the job finished and one that ended because it stopped to ask the
/// user both land on `idle`; Claude Code separates them and this asks it which happened.
fn bg_light_state(f: &CliFact) -> Option<&'static str> {
    if f.kind != "background" {
        return None;
    }
    match (f.status.as_str(), f.job_state.as_str()) {
        // Working right now, whatever the last hook said — a job between tool calls is still
        // running, and its light is green (requested live).
        ("busy", _) => Some("running"),
        // Stopped, and waiting on the user: that is what orange means everywhere else on the
        // bar (a permission prompt, a question), and it is what this is — provided the job is
        // actually asking. A job that finished and sits at an empty prompt reports the same
        // `blocked` and asks nothing, so it keeps the `idle` its own hook wrote (#084).
        (_, "blocked") if !bg_unprompted(f) => Some("blocked"),
        // `done` and `working` both read as the hook wrote them: finished, or stale.
        _ => None,
    }
}

/// Keep a spawned console program from flashing a window on the user's desktop.
///
/// The bar is a GUI process, so it owns no console; every console program it starts therefore
/// makes Windows allocate a **new** one, and the window blinks into view and out again. That
/// is not a theoretical concern — `claude agents --json` runs every `CLI_FACTS_TTL` seconds
/// forever, so it blinked every ten seconds all day. Reported live as "a terminal window
/// keeps popping in and out of my desktop", and measured: `app` → `claude.exe` → `conhost.exe`
/// at 10-second intervals. Nothing is lost, because every one of these spawns is read through
/// a pipe rather than a terminal (Agent Guideline #3: never intrude on the user's screen).
///
/// A no-op off Windows, so call sites need no `cfg`.
#[cfg(windows)]
fn no_window(cmd: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn no_window(_cmd: &mut std::process::Command) {}

/// How long a `claude agents --json` answer is reused, and how long an unlisted CLI light is
/// tolerated before it is treated as a pre-warmed spare rather than a session.
const CLI_FACTS_TTL: i64 = 10;
const CLI_UNLISTED_SECS: i64 = 20;

/// How long a finished background agent keeps its light (decision 065). A `--bg` job's process
/// stays alive and listed after its work is done, so nothing in decision 054 ever retires it.
/// Long enough that "it just finished" is still readable on the bar — the light is the only
/// place that shows it — and short enough that an abandoned job stops occupying a slot. The
/// clock is hook silence, so a job that picks work back up resets it.
const CLI_BG_DONE_SECS: i64 = 5 * 60;

/// Ask Claude Code to enumerate its own live sessions. This is the only way to tell a real
/// session from a **pre-warmed spare**: a spare is a `claude bg-spare` process that fires
/// `SessionStart` (so the hook writes a status file and a light appears) but never becomes a
/// session, and its argv is byte-identical to that of a genuine background agent — so no
/// process inspection can separate them. Spares are started by a long-lived daemon, so they
/// do not inherit `AGENTSTATUS_IGNORE` from anyone and cannot opt themselves out.
///
/// Run by absolute path, with no shell at all (decision 064). The login shell this used to go
/// through could not find `claude` when the bar was launched the way users launch it, so this
/// query — and with it every CLI reconciliation decisions 054 and 064 rest on — silently
/// returned None and reconciled nothing. Verified: the binary runs fine on a bare
/// `PATH=/usr/bin:/bin`, so no shell is needed to reach it.
fn cli_facts_query() -> Option<std::collections::HashMap<String, CliFact>> {
    let mut cmd = std::process::Command::new(claude_bin());
    cmd.args(["agents", "--json"]).env("AGENTSTATUS_IGNORE", "1");
    no_window(&mut cmd);
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let mut map = std::collections::HashMap::new();
    for a in v.as_array()? {
        let Some(sid) = a.get("sessionId").and_then(|x| x.as_str()) else {
            continue;
        };
        map.insert(
            sid.to_string(),
            CliFact {
                kind: a.get("kind").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                pid: a.get("pid").and_then(|x| x.as_i64()).unwrap_or(0),
                status: a.get("status").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                job_state: a.get("state").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                needs: a.get("id").and_then(|x| x.as_str()).map(job_needs).unwrap_or_default(),
            },
        );
    }
    Some(map)
}

/// What a background job says it is waiting for, read from the job's own record rather than
/// taken from `claude agents --json` — which carries the job's `tempo` as its `state` but not
/// the `needs` behind it (decision 084). Only background agents have a job id, so this is read
/// only for them. A missing file, an unreadable one, or a null `needs` all read as empty, which
/// changes nothing.
fn job_needs(job_id: &str) -> String {
    let path = std::path::PathBuf::from(home())
        .join(".claude")
        .join("jobs")
        .join(job_id)
        .join("state.json");
    let Ok(text) = std::fs::read_to_string(path) else {
        return String::new();
    };
    serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|v| v.get("needs").and_then(|x| x.as_str()).map(str::to_string))
        .unwrap_or_default()
}

/// Cached `cli_facts_query`, refreshed at most every `CLI_FACTS_TTL` seconds. A failed query
/// caches as `None`, which reconciles nothing — the same fail-open contract as #048.
fn cli_facts(now: i64) -> Option<std::collections::HashMap<String, CliFact>> {
    type Cache = std::sync::Mutex<(i64, Option<std::collections::HashMap<String, CliFact>>)>;
    static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new((0, None)));
    let Ok(mut guard) = cache.lock() else {
        return None;
    };
    if now - guard.0 >= CLI_FACTS_TTL {
        *guard = (now, cli_facts_query());
    }
    guard.1.clone()
}

/// The `claude` process id the hook recorded for a session, or 0 when absent (a status
/// file written before decision 054).
fn session_pid(session_id: &str) -> i64 {
    std::fs::read_to_string(sessions_dir().join(format!("{session_id}.json")))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("pid").and_then(|x| x.as_i64()))
        .unwrap_or(0)
}

/// Jump to a session's window by focusing it through the **IDE's own CLI**
/// (`code`/`cursor <folder>`). The IDE resolves the folder to its existing window
/// and focuses it — switching macOS Spaces (including a full-screen Space) because
/// the app manages its own window. It only opens a new window if the folder isn't
/// open anywhere. We focus the *workspace root* (from the lock files) so a subfolder
/// `cwd` still lands on the right window.
///
/// This replaces the old `open -a <folder>` (decision 016): `open -a` spawns a *new*
/// window whenever macOS can't match an existing one — which is exactly what happens
/// with full-screen windows in their own Spaces (the app's core use case), since
/// they aren't reachable from another Space. The IDE CLI has no such limitation and
/// needs no extra permission (unlike AppleScript window-raising, which can't even see
/// full-screen windows on inactive Spaces — verified live). If the CLI binary is
/// missing we fall back to `open -a` (Agent Guideline #3: degrade, never break). The
/// IDE is chosen from the session's `ide` field (decision 015).
///
/// Hosts with no IDE window of their own (decision 054): a `cli` session is focused in
/// its terminal, and a `claude-desktop` session activates Claude. Both must be handled
/// explicitly — falling through to the VS Code CLI would land the user in the wrong
/// application entirely.
#[tauri::command]
fn focus_session(cwd: String, ide: String, session_id: String) {
    // Focus the exact session tab via the extension relay (decision 015); the window
    // raise below only gets us to the right *window*. Written first so the extension
    // can pick it up while / right after the window comes forward.
    //
    // VS Code sessions only (decision 054) — the relay exists solely to reach the VS Code
    // extension, so writing it for a host that extension cannot serve is meaningless at
    // best. At worst it was the bug: the extension acted on any id among the sessions whose
    // `cwd` fell inside its workspace, without checking the host, so a Cursor / terminal /
    // Claude Desktop session in a folder some VS Code window happened to have open matched,
    // and `claude-vscode.editor.open` was handed an id naming no VS Code session — which
    // opens a **new Claude tab**. Observed exactly that way on a Claude Desktop session
    // still tagged `vscode`: the click focused VS Code *and* left a stray tab behind. The
    // extension now filters by host too; this guard also covers an older extension build.
    if ide == "vscode" {
        write_focus_request(&session_id);
    }
    // Windows (decision 070). Per-tab focus is already handled above by the extension relay,
    // which is plain TypeScript and needs nothing platform-specific; what is left is getting
    // the right *window* forward. There is no Spaces problem here, so the direct Win32 raise
    // is the whole answer rather than macOS's raise-plus-CLI belt and braces.
    //
    // A `cli` session has no window of its own, so it is focused through the process that
    // hosts it (decision 071, amending #069): the hook records the owning `claude` pid, and
    // the first ancestor of that pid owning a visible window is the terminal. Claude Desktop
    // takes the same route, falling back to matching its window by title when the pid is
    // missing — a status file written before the pid walk existed carries none.
    #[cfg(windows)]
    {
        if ide == "cli" || ide == "claude-desktop" {
            if focus_host_window(session_pid(&session_id), &session_id) {
                return;
            }
            if ide == "claude-desktop" {
                raise_window_titled(&|t: &str| t == "Claude" || t.ends_with(" - Claude"));
            }
            return;
        }
        if cwd.is_empty() {
            return;
        }
        let root = workspace_root(&cwd);
        if raise_window_fast(&root, &ide) {
            return;
        }
        // No matching window. Cursor stops here (decision 069 — its CLI opened a new agent
        // rather than focusing on macOS, #047, and that has not been retested here, so we do
        // not risk spawning something). VS Code falls back to its CLI, which focuses the
        // window that has the folder open, or opens one if none does.
        if ide != "cursor" {
            open_in_vscode(&root);
        }
    }
    #[cfg(target_os = "macos")]
    {
        // Cursor never goes through the IDE CLI (decision 047): with the Agent ("glass")
        // window active, `cursor <folder>` is intercepted by Cursor's main process
        // (`resolveGlassCliFolderTarget` → `vscode:createNewComposer {folderUri}`) and
        // opens a *new* agent in that folder instead of focusing anything. Press the
        // session's own row in Cursor's tray menu instead — that's the one thing that
        // opens the existing conversation. Falls back to raise + activate, never the CLI.
        // A CLI session has no window to raise. If it owns a terminal, focus the tab it runs
        // in; if it does not, it is a detached background agent, so open it in a new terminal
        // via `claude attach` — otherwise its light would be the one thing on the bar that
        // leads nowhere (UI Principle #3).
        if ide == "cli" {
            // Ask Claude Code which kind of session this is rather than inferring it from the
            // recorded pid. That pid is the hook's parent, which for a session hosted in a
            // `bg-spare` process is a helper with no controlling terminal even though the
            // session is interactive — inferring from it sent an interactive session down the
            // attach path and opened a redundant terminal tab. Claude Code reports both the
            // kind and the pid that actually owns the session.
            let listing = cli_facts(now_unix());
            let fact = listing.as_ref().and_then(|m| m.get(&session_id).cloned());
            // Claude Code answered and does not know this session: it is a pre-warmed spare
            // whose light has not been pruned yet (decision 064), or an agent that has since
            // exited. There is nothing to attach to, and `claude attach` on an id with no live
            // job does not fail quietly — it drops into Claude Code's **agent view**, which
            // lists every session, in a terminal window opened for the occasion. Reported live
            // as "a new light opened a Ghostty window with copies of my sessions". Do nothing
            // instead: the light is about to disappear, and no action beats a wrong one
            // (UI Principle #3 — a click leads to *that* session or nowhere).
            if listing.is_some() && fact.is_none() {
                return;
            }
            let pid = match &fact {
                Some(f) if f.pid > 0 => f.pid,
                _ => session_pid(&session_id),
            };
            let interactive = match &fact {
                Some(f) => f.kind == "interactive",
                // The query itself failed, so nothing is known: fall back to the terminal it
                // owns, if any. Fail-open, the same contract as #048.
                None => tty_of(pid).is_some(),
            };
            match (interactive, tty_of(pid)) {
                (true, Some(tty)) => focus_terminal_session(pid, &tty, &session_id),
                (true, None) => {}
                _ => attach_background_agent(&session_id),
            }
            return;
        }
        // Claude Code inside Claude Desktop: bring Claude forward. Desktop exposes no
        // scripting interface for selecting a specific conversation, so this is
        // app-level focus by necessity, not by choice.
        if ide == "claude-desktop" {
            let _ = std::process::Command::new("open")
                .args(["-a", "Claude"])
                .spawn();
            return;
        }
        if ide == "cursor" {
            if let Some(name) = cursor_composer_name(&session_id) {
                if cursor_press_tray_row(&|t| tray_row_is(t, &name)) {
                    activate_cursor();
                    return;
                }
            }
            // No row for this composer (unnamed, or older than the tray's 10 recents),
            // or the aggregate menu-bar pip (decision 038), which has no session at all:
            // bring Cursor forward — its window, if we can name one.
            if !cwd.is_empty() {
                raise_window_fast(&workspace_root(&cwd), &ide);
            }
            activate_cursor();
            return;
        }
        if cwd.is_empty() {
            return;
        }
        let root = workspace_root(&cwd);
        // Fast path first: raise the window in ~0.2s if it's on the current Space.
        // The CLI below always runs too, covering the cross-Space / full-screen case
        // the fast path can't reach (decision 021).
        raise_window_fast(&root, &ide);
        let (cli, app) = (
            "/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code",
            "Visual Studio Code",
        );
        if std::path::Path::new(cli).exists() {
            let _ = std::process::Command::new(cli).arg(&root).spawn();
        } else {
            let _ = std::process::Command::new("open")
                .args(["-a", app])
                .arg(&root)
                .spawn();
        }
    }
}

/// Drop one light on the user's own say-so (decision 080). The automatic prunes in
/// `list_sessions` are all evidence-based — a closed window, a dead pid, an archived
/// composer, the idle backstop — so a session whose evidence has not arrived yet keeps
/// its light until the timer catches up. This is the manual override: delete that
/// session's status file (and its subagent markers) exactly as a prune would, from a
/// close button in the settings panel.
///
/// It is a deletion, not a hide: a session that is genuinely alive re-registers on its
/// next hook event and its light comes back, which is the honest answer (UI Principle
/// #4) — the bar must not keep showing a light for a session it was told to forget, nor
/// keep hiding one that is still running.
///
/// `id` names a file, so it is checked to be a bare session id (the uuid the hook writes,
/// optionally `bc-`-prefixed) before it is joined onto a path — never a traversal.
/// Returns whether a status file was actually removed.
#[tauri::command]
fn dismiss_session(id: String) -> bool {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return false;
    }
    let dir = sessions_dir();
    let removed = std::fs::remove_file(dir.join(format!("{id}.json"))).is_ok();
    let _ = std::fs::remove_dir_all(dir.join(format!("{id}.subagents")));
    removed
}

/// Quit the whole app from the settings window. As an Accessory app (no Dock icon,
/// no app menu — see `setup`) the bar has no OS-provided Quit, so this button is the
/// only in-UI way out. `exit(0)` tears down the panel and tray and ends the process;
/// the hooks keep writing status files regardless, so relaunching repopulates the bar.
#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

/// The app's own version, for the settings window's About section. The webview cannot
/// read Cargo's version and the bundle identifier is not it.
#[tauri::command]
fn app_version(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}

/// Open the settings window (decision 082) — raising it if it already exists, so the
/// gear on the bar always lands you in front of it. Never closes: the window has a title
/// bar for that, and the gear is only reachable while the bar's controls are revealed.
///
/// Built lazily: an always-on bar should not carry a second webview it may never show.
/// It is a plain decorated window — deliberately not the `main` window's NSPanel
/// (decision 008), which exists to float over other apps without taking focus. Settings
/// wants the opposite: normal stacking, keyboard focus, and a title bar to close.
#[tauri::command]
fn open_settings(app: tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    cdbg("settings: open requested");
    if let Some(win) = app.get_webview_window(SETTINGS_ID) {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
        return;
    }
    let _ = tauri::WebviewWindowBuilder::new(
        &app,
        SETTINGS_ID,
        tauri::WebviewUrl::App("settings.html".into()),
    )
    .title("AgentStatus Settings")
    .inner_size(560.0, 430.0)
    .resizable(false)
    .maximizable(false)
    .center()
    .focused(true)
    .build();
}

/// Bring Cursor.app to the front (activating it also switches to the Space its
/// frontmost window lives on). `open -a` with no file argument only activates — it
/// never opens a window — so it is safe to call after Cursor has already put the
/// right composer's window in front.
#[cfg(target_os = "macos")]
fn activate_cursor() {
    let _ = std::process::Command::new("open")
        .args(["-a", "Cursor"])
        .spawn();
}

/// The PID of the running Cursor app (its main process is `Cursor`), or None. Used to
/// root the Accessibility query at Cursor's app element.
#[cfg(target_os = "macos")]
fn cursor_pid() -> Option<i32> {
    let out = std::process::Command::new("pgrep")
        .args(["-x", "Cursor"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .and_then(|s| s.parse::<i32>().ok())
}

/// Copy an AX attribute off an element and return it as a CoreFoundation type
/// (auto-released on drop). None if the attribute is missing or the call errors.
#[cfg(target_os = "macos")]
fn ax_attr(
    element: accessibility_sys::AXUIElementRef,
    name: &str,
) -> Option<core_foundation::base::CFType> {
    use accessibility_sys::{kAXErrorSuccess, AXUIElementCopyAttributeValue};
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::string::CFString;
    let attr = CFString::new(name);
    let mut value: core_foundation::base::CFTypeRef = std::ptr::null();
    let err = unsafe {
        AXUIElementCopyAttributeValue(element, attr.as_concrete_TypeRef(), &mut value)
    };
    if err != kAXErrorSuccess || value.is_null() {
        return None;
    }
    // CopyAttributeValue follows the CF "create" rule (+1 retain) → wrap so Drop releases it.
    Some(unsafe { CFType::wrap_under_create_rule(value) })
}

/// The count of digits found in an AX element's `AXTitle` (`" 1"` → 1), or None if the
/// title is absent or has no digits.
#[cfg(target_os = "macos")]
fn ax_title_count(element: accessibility_sys::AXUIElementRef) -> Option<i64> {
    use core_foundation::string::CFString;
    let title = ax_attr(element, "AXTitle")?.downcast::<CFString>()?.to_string();
    let digits: String = title
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse::<i64>().ok()
}

/// Ask macOS for Accessibility trust, showing the system prompt if not already granted
/// (and opening the Accessibility settings pane). Needed for the Cursor menu-bar read
/// (decision 038) and the fast window-raise (decision 021). Because the app is unsigned
/// (ad-hoc), every rebuild changes its code hash and invalidates a prior grant — so a
/// stale "AgentStatus" entry can read as checked while `AXIsProcessTrusted()` is false;
/// prompting re-registers the current bundle. Safe to call once at startup: the prompt
/// only appears while untrusted, never after the grant.
#[cfg(all(target_os = "macos", not(debug_assertions)))]
fn prompt_accessibility() {
    use accessibility_sys::{kAXTrustedCheckOptionPrompt, AXIsProcessTrustedWithOptions};
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;
    let key = unsafe { CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt) };
    let opts =
        CFDictionary::from_CFType_pairs(&[(key.as_CFType(), CFBoolean::true_value().as_CFType())]);
    unsafe { AXIsProcessTrustedWithOptions(opts.as_concrete_TypeRef()) };
}

/// Read Cursor's macOS menu-bar status item and return how many Cursor composers are
/// awaiting the user's attention (decision 038). Cursor's live running/idle status is
/// renderer-memory-only, but its menu-bar item surfaces an aggregate **unread
/// notification count** — its AX title is `" N"` (icon glyph + count), which flips on
/// when a composer finishes / awaits you. That's the one attention bit Cursor's hooks
/// don't provide (the bridged `Stop` carries no wrap-up message, so Cursor sessions
/// otherwise render as plain dim idle, never "done").
///
/// Read via the **Accessibility API directly** (not osascript → System Events): the AX
/// call only needs the Accessibility grant the app already uses for the fast
/// window-raise (decision 021), whereas the System Events route would additionally need
/// the separate Automation permission. Path: Cursor's app element → `AXExtrasMenuBar`
/// (the right-hand status-item bar) → its children → the first child whose `AXTitle`
/// carries a digit. (The status item exposes a nil `AXDescription`, so we match on the
/// numeric title, not a description — Cursor's extras bar holds only this one item.)
/// Best-effort, fail-*closed* to 0 (no cue) on any error: Cursor not running, AX not
/// granted, item absent, or a non-numeric title.
/// Marker-gated debug log: writes only when `~/.claude/status/cursor-debug` exists, so
/// it's silent in normal use but can be switched on (touch the marker) to trace the AX
/// read without a rebuild. Used to diagnose the decision-038/039 trust chain.
#[cfg(target_os = "macos")]
fn cdbg(msg: &str) {
    let home = std::env::var("HOME").unwrap_or_default();
    let dir = std::path::PathBuf::from(home).join(".claude").join("status");
    if !dir.join("cursor-debug").exists() {
        return;
    }
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("cursor-debug.log"))
    {
        let _ = writeln!(f, "{} {}", now_unix(), msg);
    }
}

#[cfg(target_os = "macos")]
#[tauri::command]
fn cursor_attention_count() -> i64 {
    let n = cursor_attention_count_inner();
    cdbg(&format!(
        "trusted={} count={n}",
        unsafe { accessibility_sys::AXIsProcessTrusted() }
    ));
    n
}

#[cfg(target_os = "macos")]
fn cursor_attention_count_inner() -> i64 {
    use accessibility_sys::{AXUIElementCreateApplication, AXUIElementRef};
    use core_foundation::array::CFArray;
    use core_foundation::base::{CFType, TCFType};

    let Some(pid) = cursor_pid() else {
        return 0;
    };
    // Root the query at Cursor's app element (create rule → wrap for auto-release).
    let app_ref = unsafe { AXUIElementCreateApplication(pid) };
    if app_ref.is_null() {
        return 0;
    }
    let app = unsafe { CFType::wrap_under_create_rule(app_ref as _) };
    let app_ref = app.as_CFTypeRef() as AXUIElementRef;

    // The status items live under the app's "extras" menu bar (AppleScript's "menu bar 2").
    let Some(extras) = ax_attr(app_ref, "AXExtrasMenuBar") else {
        return 0;
    };
    let extras_ref = extras.as_CFTypeRef() as AXUIElementRef;
    let Some(children) = ax_attr(extras_ref, "AXChildren") else {
        return 0;
    };
    let Some(items) = children.downcast::<CFArray>() else {
        return 0;
    };
    // Return the count from the first status item whose title carries a digit.
    for item in items.iter() {
        if let Some(n) = ax_title_count(*item as AXUIElementRef) {
            return n;
        }
    }
    0
}

// Non-macOS stub so the command is always registered and the frontend can call it
// unconditionally; there is no Cursor menu-bar item to read off macOS.
#[cfg(not(target_os = "macos"))]
#[tauri::command]
fn cursor_attention_count() -> i64 {
    0
}

/// The AX title prefix Cursor puts on a tray menu entry whose composer has an unread
/// notification (`"\u{2022} "` — a bullet, then the composer name).
#[cfg(target_os = "macos")]
const CURSOR_NOTIFY_PREFIX: &str = "\u{2022}";

/// The title of the one tray row that clears Cursor's notifications. Its click sends
/// `vscode:clearAllNotifications`, which is the *only* thing that marks composers read
/// (decision 083) — pressing a composer's own row does not.
#[cfg(target_os = "macos")]
const CURSOR_CLEAR_ALL_ROW: &str = "Clear All Notifications";

/// A tray row's title without the unread-notification bullet: `"• Fix the parser"` and
/// `"Fix the parser"` both compare as `"Fix the parser"`.
#[cfg(target_os = "macos")]
fn trim_bullet(title: &str) -> &str {
    title
        .strip_prefix(CURSOR_NOTIFY_PREFIX)
        .unwrap_or(title)
        .trim()
}

/// Whether a tray row is the row for the composer named `name`. Cursor decorates a row's
/// `AXTitle` on both ends: the unread bullet in front (`"• Fix the parser"`) and a live
/// status suffix behind (`"Fix the parser, Running"` — verified live against the tray of
/// Cursor 3.12). Exact-equality matching therefore missed every *running* composer, which
/// is exactly the state a light is in when the user clicks it, so the click fell through
/// to the "just activate Cursor" fallback instead of opening the conversation. Match the
/// bare name or the name plus that `", <status>"` suffix.
#[cfg(target_os = "macos")]
fn tray_row_is(title: &str, name: &str) -> bool {
    let t = trim_bullet(title);
    t == name || t.strip_prefix(name).is_some_and(|rest| rest.starts_with(", "))
}

/// The status suffix Cursor puts on the tray row of a composer whose turn is in flight
/// (`"Fix the parser, Running"`). Positive, live evidence that an agent is working — the
/// one thing Cursor's on-disk record does not give us (decision 052).
#[cfg(target_os = "macos")]
const CURSOR_TRAY_RUNNING: &str = ", Running";

/// Every row title in Cursor's tray menu, read without pressing anything: the walk in
/// `cursor_press_tray_row` only presses a row its predicate accepts, so a predicate that
/// records and always declines collects the whole menu. Empty when Cursor isn't running or
/// the app has no Accessibility grant — indistinguishable from an empty menu, which is why
/// callers may only use this as positive evidence, never as evidence of absence.
#[cfg(target_os = "macos")]
fn cursor_tray_titles() -> Vec<String> {
    let titles = std::cell::RefCell::new(Vec::new());
    cursor_press_tray_row(&|t| {
        titles.borrow_mut().push(t.to_string());
        false
    });
    let titles = titles.into_inner();
    // Marker-gated (see `cdbg`): this walk is the one thing the app does that can cancel
    // Cursor's own menu while the user has it open, so it must be traceable without a
    // rebuild (decision 081).
    cdbg(&format!("tray_walk rows={}", titles.len()));
    titles
}

#[cfg(not(target_os = "macos"))]
fn cursor_tray_titles() -> Vec<String> {
    Vec::new()
}

/// Whether Cursor's own tray says the composer named `name` is running right now.
#[cfg(target_os = "macos")]
fn tray_says_running(titles: &[String], name: &str) -> bool {
    !name.is_empty()
        && titles.iter().any(|t| {
            trim_bullet(t)
                .strip_prefix(name)
                .is_some_and(|rest| rest == CURSOR_TRAY_RUNNING)
        })
}

#[cfg(not(target_os = "macos"))]
fn tray_says_running(_titles: &[String], _name: &str) -> bool {
    false
}

/// `cursor_tray_titles` behind the same TTL as the fact query, and read lazily — the AX
/// walk only happens on a poll that actually has a light to reconcile.
fn cursor_tray_titles_cached(now: i64) -> Vec<String> {
    type Cache = std::sync::Mutex<(i64, Vec<String>)>;
    static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new((0, Vec::new())));
    let Ok(mut guard) = cache.lock() else {
        return Vec::new();
    };
    if now - guard.0 >= CURSOR_FACTS_TTL {
        *guard = (now, cursor_tray_titles());
    }
    guard.1.clone()
}

/// The name Cursor shows for a composer, looked up by id (decision 047). A Cursor
/// session's `session_id` **is** its `composerId` — Cursor's Claude-compat bridge passes
/// it straight through — and its record lives in Cursor's own key-value store under
/// `composerData:<composerId>`. We pull one field, `.name`, because that string is the
/// only handle Cursor's tray menu exposes for a row (the composerId is in the click
/// handler's closure, not the AX tree), and pressing that row is what opens the
/// conversation. Read-only `sqlite3` (on every macOS) against the live db, one field, no
/// message content (Agent Guideline #5). None if the id isn't a plain uuid (this string
/// goes into SQL), the db/binary is missing, or the composer has no name yet.
#[cfg(target_os = "macos")]
fn cursor_composer_name(session_id: &str) -> Option<String> {
    if session_id.is_empty()
        || !session_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return None;
    }
    let db = std::path::PathBuf::from(std::env::var("HOME").ok()?)
        .join("Library/Application Support/Cursor/User/globalStorage/state.vscdb");
    let out = std::process::Command::new("/usr/bin/sqlite3")
        .arg("-readonly")
        .arg(&db)
        .arg(format!(
            "select json_extract(value,'$.name') from cursorDiskKV \
             where key='composerData:{session_id}';"
        ))
        .output()
        .ok()?;
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
    cdbg(&format!("composer_name: {session_id} -> {name:?}"));
    if name.is_empty() || name == "null" {
        None
    } else {
        Some(name)
    }
}

/// AXPress the first row of Cursor's tray menu whose title satisfies `want`, and report
/// whether one was pressed. Shared by the pip's "next waiting composer" click (decision
/// 045) and a session light's "this composer" click (decision 047) — they differ only in
/// the predicate. Path: Cursor's app element → `AXExtrasMenuBar` → the status item →
/// `AXChildren[0]` (its `AXMenu`) → the rows, none of which requires opening the menu.
#[cfg(target_os = "macos")]
fn cursor_press_tray_row(want: &dyn Fn(&str) -> bool) -> bool {
    use accessibility_sys::{AXUIElementCreateApplication, AXUIElementRef};
    use core_foundation::array::CFArray;
    use core_foundation::base::{CFType, TCFType};

    let Some(pid) = cursor_pid() else {
        cdbg("press: no cursor pid");
        return false;
    };
    let app_ref = unsafe { AXUIElementCreateApplication(pid) };
    if app_ref.is_null() {
        return false;
    }
    let app = unsafe { CFType::wrap_under_create_rule(app_ref as _) };
    let app_ref = app.as_CFTypeRef() as AXUIElementRef;

    let Some(extras) = ax_attr(app_ref, "AXExtrasMenuBar") else {
        return false;
    };
    let Some(items) = ax_attr(extras.as_CFTypeRef() as AXUIElementRef, "AXChildren")
        .and_then(|c| c.downcast::<CFArray>())
    else {
        return false;
    };
    for item in items.iter() {
        // Each array must stay *bound* while its elements are used: the CFArray owns the
        // only retain on the children, so pulling an element out of a temporary and letting
        // the array drop leaves a dangling AXUIElementRef — which crashed the app inside
        // AXUIElementCopyAttributeValue (EXC_BREAKPOINT in _AXUIElementValidate).
        let Some(menus) = ax_attr(*item as AXUIElementRef, "AXChildren")
            .and_then(|c| c.downcast::<CFArray>())
        else {
            continue;
        };
        let Some(menu) = menus.iter().next().map(|m| *m as AXUIElementRef) else {
            continue;
        };
        if press_in_menu(menu, want) {
            return true;
        }
    }
    cdbg("press: no matching entry");
    false
}

/// Press the first row of `menu` matching `want`, descending into a row's submenu
/// (Cursor's "View More (N)", which holds recents 6–10) when the row itself doesn't
/// match. Rows are walked in menu order, so a main-list match always wins over a
/// submenu one.
#[cfg(target_os = "macos")]
fn press_in_menu(menu: accessibility_sys::AXUIElementRef, want: &dyn Fn(&str) -> bool) -> bool {
    use accessibility_sys::{kAXErrorSuccess, AXUIElementPerformAction, AXUIElementRef};
    use core_foundation::array::CFArray;
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;

    let Some(rows) = ax_attr(menu, "AXChildren").and_then(|c| c.downcast::<CFArray>()) else {
        return false;
    };
    for row in rows.iter() {
        let row = *row as AXUIElementRef;
        let title = ax_attr(row, "AXTitle")
            .and_then(|t| t.downcast::<CFString>())
            .map(|t| t.to_string())
            .unwrap_or_default();
        if want(&title) {
            let action = CFString::new("AXPress");
            let err = unsafe { AXUIElementPerformAction(row, action.as_concrete_TypeRef()) };
            cdbg(&format!("press: pressed {title:?} err={err}"));
            return err == kAXErrorSuccess;
        }
        // A submenu row's AXMenu is its only child; leaf rows have none.
        let Some(subs) = ax_attr(row, "AXChildren").and_then(|c| c.downcast::<CFArray>()) else {
            continue;
        };
        if let Some(sub) = subs.iter().next().map(|m| *m as AXUIElementRef) {
            if press_in_menu(sub, want) {
                return true;
            }
        }
    }
    false
}

/// Open the next Cursor composer that's awaiting the user and clear Cursor's menu-bar
/// notifications (decisions 045, 083). Returns true if a composer entry was pressed.
///
/// Cursor's tray menu (`TrayMainService.createContextMenu`, verified in the Cursor
/// 3.15.6 bundle) is a native Electron `Menu`, so every entry is a real `AXMenuItem`
/// reachable from the status item *without opening the menu* — status item →
/// `AXChildren[0]` (its `AXMenu`) → the item rows. Entries for composers with an unread
/// notification are titled `"• <name>"`; pressing one sends `vscode:openComposer` to
/// that composer's window and focuses it, so the press still jumps the user to the
/// waiting composer.
///
/// It no longer *clears* that composer's notification, though. A row's bullet is
/// `hasUnreadMessages || badgeCount > 0`, and the `vscode:openComposer` handler touches
/// neither: only `vscode:clearAllNotifications` — the "Clear All Notifications" row —
/// calls `markAgentRead` and `clearAllBadges`. In Glass mode the per-composer badge
/// listener that used to clear a badge on focus is never even registered
/// (`isGlass || this.setupFocusListener()`), so a bullet outlives opening the composer
/// by up to its 1 h auto-clear timer. Verified live on 3.15.6: pressing the composer row
/// left the count at 2, pressing the clear-all row took it 2 → 0. So press both — the
/// composer row to navigate, then the clear-all row to actually dismiss the pip
/// (decision 083). Cursor exposes no per-composer clear, so this necessarily clears the
/// other waiting composers' bullets too.
///
/// Presses the *first* bulleted entry, which is the one Cursor itself ranks highest:
/// its menu is sorted notification-first, then in-progress, then most-recently-updated.
/// Same Accessibility grant as the count read (decision 039); fails silently (false) if
/// Cursor isn't running, AX isn't granted, or no entry carries a notification.
#[cfg(target_os = "macos")]
#[tauri::command]
fn cursor_open_next_attention() -> bool {
    if !cursor_press_tray_row(&|t| t.starts_with(CURSOR_NOTIFY_PREFIX)) {
        return false;
    }
    // Best-effort: the row is disabled when Cursor has no notifications left, and a
    // localized Cursor would title it differently — either way the composer is open.
    cursor_press_tray_row(&|t| t.trim() == CURSOR_CLEAR_ALL_ROW);
    // The presses open the composer and clear the notifications, but they do not bring
    // Cursor forward: an AXPress issued from a background app leaves the frontmost app
    // unchanged (observed — the badge cleared, the user stayed put). Activating
    // afterwards completes the click-through (decision 046).
    activate_cursor();
    true
}

// Non-macOS stub (see cursor_attention_count).
#[cfg(not(target_os = "macos"))]
#[tauri::command]
fn cursor_open_next_attention() -> bool {
    false
}

/// Switch the bar between its two presentations (decision 024). Floating = the
/// always-visible NSPanel (default); menu-bar = a tray item that shows the lights as
/// a generated image (`set_tray_image`) and reveals the panel as a popover on click.
/// The frontend owns the persisted preference (`localStorage`) and calls this on load
/// and on toggle; here we only flip the tray's visibility and hide/show the panel.
/// Which platform the bar is running on, so the frontend can drop controls the backend
/// cannot honour and shape the tray image to what this platform's tray actually accepts
/// (decision 072). One call at startup, not a per-poll check.
#[tauri::command]
fn platform() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "other"
    }
}

/// Fit a tray image into a square.
///
/// A Windows notification-area icon **is** square — 16x16 logical, scaled by DPI — and a
/// non-square image is stretched to fill it. The bar's dot strip is 170x44 for five
/// sessions, so stretching would squash each dot to a couple of pixels wide: an unreadable
/// smear. Centring the pixels on a transparent square of the longer side lets Windows scale
/// it down without distorting it. (The frontend also forces the single-dot condensed shape
/// on Windows, so in practice this squares a 34x44 image, not a long strip.)
#[cfg(windows)]
fn square_icon(rgba: Vec<u8>, width: u32, height: u32) -> (Vec<u8>, u32) {
    let side = width.max(height);
    if width == side && height == side {
        return (rgba, side);
    }
    let mut out = vec![0u8; (side as usize) * (side as usize) * 4];
    let ox = ((side - width) / 2) as usize;
    let oy = ((side - height) / 2) as usize;
    let row = (width as usize) * 4;
    for y in 0..height as usize {
        let src = y * row;
        let dst = ((y + oy) * side as usize + ox) * 4;
        out[dst..dst + row].copy_from_slice(&rgba[src..src + row]);
    }
    (out, side)
}

/// Returns whether the requested mode could actually be applied — specifically, whether a
/// tray item exists to represent the bar in menu-bar mode.
///
/// The frontend needs this answer: if it switches to menu-bar mode where no tray was built,
/// the panel it hides on the next light click has nothing to bring it back — no tray icon,
/// no taskbar button (`skipTaskbar`), no Dock icon — and the app becomes unreachable without
/// killing the process. So a `false` here tells the frontend to stay floating.
#[tauri::command]
fn set_mode(app: tauri::AppHandle, mode: String) -> bool {
    #[cfg(any(target_os = "macos", windows))]
    {
        let menubar = mode == "menubar";
        // Answered here rather than inside the closure: the caller needs it synchronously,
        // and looking a tray up by id is a registry read, not a UI call.
        let has_tray = app.tray_by_id(TRAY_ID).is_some();
        // Tells the dismissal handlers whether the panel is a popover (close it) or the
        // floating bar (leave it alone).
        TRAY_MODE.store(menubar && has_tray, std::sync::atomic::Ordering::Relaxed);
        let app2 = app.clone();
        // NSStatusItem must be manipulated on the main thread; Tauri commands run on a
        // background thread, so marshal there. Window show/hide is marshaled by Tauri
        // internally, but we do it here too so it stays ordered with the tray change.
        let _ = app.run_on_main_thread(move || {
            if let Some(tray) = app2.tray_by_id(TRAY_ID) {
                let _ = tray.set_visible(menubar);
            }
            if let Some(win) = app2.get_webview_window("main") {
                // Hide the panel only when there's actually a tray to represent it —
                // otherwise keep it visible so a tray failure never strands the user
                // with no UI at all.
                if menubar && has_tray {
                    let _ = win.hide();
                } else {
                    let _ = win.show();
                }
            }
        });
        return !menubar || has_tray;
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = app;
        mode != "menubar"
    }
}

/// Paint the tray icon from RGBA pixels the webview rendered (the row of colored dots,
/// or a single summary dot when condensed). Reusing the webview's canvas keeps one
/// source of truth for the per-state colors (decision 017) instead of redrawing them
/// in Rust. Called only in menu-bar mode, and only when the image actually changed
/// (the frontend signature-skips unchanged frames), so this is cheap at the 1 Hz poll.
#[tauri::command]
fn set_tray_image(app: tauri::AppHandle, rgba: Vec<u8>, width: u32, height: u32) {
    #[cfg(any(target_os = "macos", windows))]
    {
        if width == 0 || height == 0 || rgba.len() != (width as usize) * (height as usize) * 4 {
            return;
        }
        #[cfg(windows)]
        let (rgba, width, height) = {
            let (pixels, side) = square_icon(rgba, width, height);
            (pixels, side, side)
        };
        let app2 = app.clone();
        // set_icon touches the NSStatusItem → main thread only (see set_mode).
        let _ = app.run_on_main_thread(move || {
            if let Some(tray) = app2.tray_by_id(TRAY_ID) {
                let img = tauri::image::Image::new_owned(rgba, width, height);
                let _ = tray.set_icon(Some(img));
                // Force color rendering: a template icon is drawn as a monochrome
                // alpha mask (all opaque pixels → black/white), which swallows our
                // per-state colors. The builder flag doesn't survive set_icon, so
                // re-assert it on every image. Template icons are a macOS concept;
                // Windows notification-area icons are always drawn in colour.
                #[cfg(target_os = "macos")]
                let _ = tray.set_icon_as_template(false);
            }
        });
    }
}

/// Whether the bar is currently presenting as a tray item, so the dismissal handler knows
/// to close the popover — and does nothing at all in floating mode, where the bar is
/// supposed to stay on screen. Read by the focus-loss handler on Windows and by the
/// click-outside monitor on macOS (decision 086).
#[cfg(any(target_os = "macos", windows))]
static TRAY_MODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// When the popover was last auto-hidden by losing focus. See `toggle_popover`.
#[cfg(windows)]
static LAST_AUTO_HIDE: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

/// The usable area of the monitor containing a point — the screen minus the taskbar and any
/// other appbars. `Monitor::size()` is the *full* screen, so anchoring to it puts the tray
/// popover underneath the taskbar, which is exactly where the tray icon the user just
/// clicked lives (decision 073).
#[cfg(windows)]
fn work_area_at(x: i32, y: i32) -> Option<(i32, i32, i32, i32)> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    // SAFETY: `info` is zeroed with cbSize set, as the API requires; the monitor handle is
    // borrowed for the duration of the call and needs no release.
    let monitor = unsafe { MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST) };
    if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
        return None;
    }
    let w = info.rcWork;
    Some((w.left, w.top, w.right, w.bottom))
}

/// Toggle the panel as a popover anchored at the tray icon. A left-click on the tray item
/// shows the panel centred on the click point; a second click hides it. The panel keeps its
/// window properties across hide/show, so per-light click, hover, and badges work exactly as
/// in floating mode. `cx`/`cy` are the click's physical screen coordinates (the cursor is
/// over the tray item on click).
///
/// The panel opens *away* from the edge the tray lives on: down from the macOS menu bar at
/// the top of the screen, up from the Windows notification area at the bottom. Choosing by
/// which half of the monitor the click landed in rather than by platform also covers a
/// Windows taskbar the user has docked to the top.
#[cfg(any(target_os = "macos", windows))]
fn toggle_popover(win: &tauri::WebviewWindow, cx: f64, cy: f64) {
    if matches!(win.is_visible(), Ok(true)) {
        let _ = win.hide();
        return;
    }
    // A click on the tray icon while the popover is open arrives *after* the focus loss that
    // already hid it. Without this the icon could never close the popover: it would hide on
    // blur and immediately reopen, so the popover would appear stuck. A click landing this
    // soon after an auto-hide is the closing click, and is consumed.
    #[cfg(windows)]
    {
        use std::sync::atomic::Ordering;
        if now_millis() - LAST_AUTO_HIDE.load(Ordering::Relaxed) < 400 {
            return;
        }
    }

    let size = win.outer_size().unwrap_or(tauri::PhysicalSize::new(0, 0));
    let (win_w, win_h) = (size.width as f64, size.height as f64);

    // Anchor to the monitor's *work area*, not the click point. The tray icon sits inside the
    // taskbar, so offsetting from the click leaves the popover overlapping it — appearing
    // "underneath the tray". Sitting the popover against the work-area edge instead puts it
    // clear of the taskbar wherever the user has docked it.
    #[cfg(windows)]
    let (x, y) = {
        match work_area_at(cx as i32, cy as i32) {
            Some((left, top, right, bottom)) => {
                let (left, top) = (left as f64, top as f64);
                let (right, bottom) = (right as f64, bottom as f64);
                // Whichever edge the tray is against: below a top-docked taskbar, above a
                // bottom-docked one.
                let y = if cy > (top + bottom) / 2.0 { bottom - win_h - 8.0 } else { top + 8.0 };
                let x = (cx - win_w / 2.0).clamp(left, (right - win_w).max(left));
                (x, y.max(top))
            }
            None => (cx - win_w / 2.0, cy - win_h - 8.0),
        }
    };

    // macOS: the menu bar is at the top of the screen, so the panel drops below the click.
    #[cfg(target_os = "macos")]
    let (x, y) = {
        let monitor = win.current_monitor().ok().flatten();
        let mut x = cx - win_w / 2.0;
        if let Some(m) = &monitor {
            let left = m.position().x as f64;
            let right = left + m.size().width as f64 - win_w;
            if right > left {
                x = x.clamp(left, right);
            }
        }
        let _ = win_h;
        (x, cy + 8.0)
    };

    let _ = win.set_position(tauri::PhysicalPosition::new(x.max(0.0), y.max(0.0)));
    let _ = win.show();
    // Take focus, so that losing it is what dismisses the popover. The window is configured
    // `focus: false` and `show()` does not activate it, so without this it never holds focus
    // and never fires the `Focused(false)` the dismissal depends on — the popover would sit
    // on top of whatever the user clicked next. Windows only: the macOS panel is deliberately
    // non-activating (#008), and focusing it would defeat that.
    #[cfg(windows)]
    let _ = win.set_focus();

    // Tell the frontend the popover was revealed, so it can open the settings panel with it
    // (decision 074). `visibilitychange` cannot carry this: WebView2 keeps the document
    // "visible" while the window is hidden, so that event simply never fires on Windows —
    // the first attempt relied on it and did nothing at all. The frontend decides what to do
    // with the signal, so emitting it on every platform costs nothing.
    use tauri::Emitter;
    let _ = win.emit("popover-shown", ());
}

/// Pull the popover back inside the work area after its own content resized it.
///
/// Opening the settings panel grows the window from ~31px tall to ~390px. Anchored just
/// above the taskbar, that growth runs straight off the bottom of the screen and takes the
/// settings with it. The frontend calls this after any resize while in tray mode; it only
/// moves the window when the window would otherwise hang off an edge.
#[tauri::command]
fn fit_popover(window: tauri::WebviewWindow) {
    #[cfg(windows)]
    {
        let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) else {
            return;
        };
        let Some((left, top, right, bottom)) = work_area_at(pos.x, pos.y) else {
            return;
        };
        let (w, h) = (size.width as i32, size.height as i32);
        let x = pos.x.clamp(left, (right - w).max(left));
        let y = pos.y.clamp(top, (bottom - h).max(top));
        if x != pos.x || y != pos.y {
            let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
        }
    }
    #[cfg(not(windows))]
    let _ = window;
}

/// Dismiss the tray popover when the user clicks anywhere outside this app (macOS).
///
/// Windows gets this from `Focused(false)` (#073), which macOS cannot use: the panel is
/// deliberately non-activating (#008), so it never takes focus and so never loses any. A
/// global NSEvent monitor sees only mouse-downs delivered to *other* applications, which is
/// exactly the "clicked away" gesture. A click on our own lights or on the status item is a
/// local event and never reaches this handler, so the tray icon keeps toggling the popover
/// as before and the Windows 400 ms debounce has no counterpart here.
///
/// Installed once at startup and left in place: it does nothing unless the bar is presenting
/// as a tray popover, and the check is one relaxed atomic load per outside click.
#[cfg(target_os = "macos")]
fn dismiss_popover_on_outside_click(app: &tauri::AppHandle) {
    use objc2_app_kit::{NSEvent, NSEventMask};
    use std::sync::atomic::Ordering;

    let app = app.clone();
    let handler = block2::RcBlock::new(move |_event: std::ptr::NonNull<NSEvent>| {
        if !TRAY_MODE.load(Ordering::Relaxed) {
            return;
        }
        if let Some(win) = app.get_webview_window("main") {
            if matches!(win.is_visible(), Ok(true)) {
                let _ = win.hide();
            }
        }
    });
    let monitor = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(
        NSEventMask::LeftMouseDown | NSEventMask::RightMouseDown | NSEventMask::OtherMouseDown,
        &handler,
    );
    // The monitor must outlive this call for the whole life of the process; there is no
    // point at which we would remove it, so hand the token to the runtime and forget it.
    std::mem::forget(monitor);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // `mut` is used only by the release-gated single-instance block below; in a
    // debug build nothing reassigns it, so allow the otherwise-unused `mut`.
    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default();

    // Single-instance guard (release only). A second launch of the app — the
    // installed /Applications copy or a dev build, both sharing the identifier
    // com.agentstatus.app — pings the already-running instance and exits, instead
    // of drawing a second overlapping bar off the same status dir. Must be the first
    // plugin registered. Gated off in dev so `npm run tauri dev` still runs while the
    // installed copy is up.
    #[cfg(not(debug_assertions))]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // The one legitimate copy stays; the newcomer already exited. Make sure
            // the survivor's bar is visible in case it was hidden.
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
            }
        }));
    }

    let builder = builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_window_state::Builder::default().build());

    // The overlay panel is an NSPanel, so the plugin is macOS-only (decision 069). On
    // Windows the same always-on-top/transparent/skip-taskbar window comes from the
    // plain Tauri window config, with no plugin involved.
    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());

    // Dismiss the tray popover when it loses focus, the way every other tray popover on
    // Windows behaves — otherwise it stays on top of whatever the user clicked next and has
    // to be dismissed from the tray icon (decision 073). Windows only: the macOS panel is
    // non-activating, so it never takes focus in the first place and its behaviour is
    // deliberately unchanged.
    #[cfg(windows)]
    let builder = builder.on_window_event(|window, event| {
        use std::sync::atomic::Ordering;
        if let tauri::WindowEvent::Focused(false) = event {
            // Only when the popover is actually on screen. A hidden window still reports
            // focus changes — opening the tray's own hidden-icons flyout produces one — and
            // stamping the debounce then made the *next* tray click look like a close and be
            // swallowed, so the icon did nothing.
            let showing = matches!(window.is_visible(), Ok(true));
            if window.label() == "main" && TRAY_MODE.load(Ordering::Relaxed) && showing {
                LAST_AUTO_HIDE.store(now_millis(), Ordering::Relaxed);
                let _ = window.hide();
            }
        }
    });

    builder
        .invoke_handler(tauri::generate_handler![
            list_sessions,
            focus_session,
            platform,
            fit_popover,
            set_mode,
            set_tray_image,
            cursor_attention_count,
            cursor_open_next_attention,
            dismiss_session,
            quit_app,
            app_version,
            open_settings
        ])
        .setup(|app| {
            // Accessory (agent) app: no Dock icon, not space-managed.
            #[cfg(target_os = "macos")]
            let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            // Request Accessibility trust (shows the system prompt only while untrusted).
            // Needed for the Cursor menu-bar read (decision 038) and the fast window-raise
            // (decision 021); an unsigned rebuild invalidates the prior grant, so prompting
            // re-registers the current bundle. Release only — dev builds aren't the copy the
            // user grants, and prompting from `tauri dev` would nag on every run.
            #[cfg(all(target_os = "macos", not(debug_assertions)))]
            prompt_accessibility();
            // Click-away dismissal for the menu-bar popover (decision 086). Installed here
            // because `setup` runs on the main thread, which AppKit requires for this call.
            #[cfg(target_os = "macos")]
            dismiss_popover_on_outside_click(app.handle());
            // Tray item — the macOS menu bar, or the Windows notification area (decision
            // 024, extended to Windows by #072). Built once here (on the main thread) but
            // hidden until the frontend switches to menu-bar mode via `set_mode`. Colored
            // (not template) so the status dots show in color; left-click is handled by us
            // (popover), not a menu. Placeholder icon until the webview pushes the first
            // dot image.
            #[cfg(any(target_os = "macos", windows))]
            {
                // A tooltip names the icon on hover, which is how Windows expects a
                // notification-area item to identify itself — without one it is an
                // anonymous dot in a row of anonymous dots (and nothing, including
                // accessibility tools, can tell which icon it is). Harmless on macOS,
                // where menu-bar items are not hover-labelled.
                let tb = TrayIconBuilder::with_id(TRAY_ID)
                    .tooltip("AgentStatus")
                    .show_menu_on_left_click(false);
                // Template icons are macOS-only, and would render our colored dots as a
                // monochrome alpha mask.
                #[cfg(target_os = "macos")]
                let tb = tb.icon_as_template(false);
                // `mut` starts here, at the first binding something actually reassigns: on
                // macOS the earlier ones are shadowed before that, so marking them mutable
                // warned on every build.
                let mut tb = tb
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            position,
                            ..
                        } = event
                        {
                            if let Some(win) = tray.app_handle().get_webview_window("main") {
                                toggle_popover(&win, position.x, position.y);
                            }
                        }
                    });
                if let Some(icon) = app.default_window_icon().cloned() {
                    tb = tb.icon(icon);
                }
                match tb.build(app) {
                    Ok(tray) => {
                        let _ = tray.set_visible(false);
                    }
                    Err(e) => eprintln!("[agentstatus] tray build failed: {e}"),
                }
            }
            // Packaged app self-installs its hooks. In dev we keep the repo hooks
            // (via `node hooks/setup.mjs`) so hook edits are live without a rebuild.
            #[cfg(not(debug_assertions))]
            install::ensure_installed(app.handle());
            if let Some(win) = app.get_webview_window("main") {
                #[cfg(target_os = "macos")]
                make_overlay_panel(&win);
                let _ = win.show();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Manual AX check for the Cursor pip's click path (decision 045). Ignored by default:
/// it drives another app's real menu — it presses the top waiting composer, so Cursor
/// opens it, exactly as a pip click does. Run it, don't hand-probe, whenever this code
/// or Cursor's tray changes (Guideline #8):
///
///   cargo test --release -- --ignored --nocapture cursor_press
///
/// Prints `pressed=true` when a notified entry existed and was pressed, `false` when
/// Cursor isn't running, has nothing waiting, or the terminal lacks Accessibility. The
/// point is that it completes at all: the first cut walked the AX tree through elements
/// whose owning CFArray had already been dropped and hard-crashed the app inside
/// `AXUIElementCopyAttributeValue`.
#[cfg(all(test, target_os = "macos"))]
mod tests {
    fn fact(kind: &str, status: &str, job_state: &str) -> super::CliFact {
        needy(kind, status, job_state, "")
    }

    /// The same, for a job whose own record says what it is waiting for (decision 084).
    fn needy(kind: &str, status: &str, job_state: &str, needs: &str) -> super::CliFact {
        super::CliFact {
            kind: kind.to_string(),
            pid: 1,
            status: status.to_string(),
            job_state: job_state.to_string(),
            needs: needs.to_string(),
        }
    }

    /// Decision 063, both halves, against the four (`status`, `state`) pairs Claude Code
    /// actually reports for a `--bg` job — measured from live jobs while writing it:
    /// busy+working (running), idle+blocked (stopped, asking the user), idle+done (finished),
    /// idle+working (spawned and never advanced — the stale job 065 exists to clear).
    #[test]
    fn bg_job_state_reconciliation() {
        // Orange means one thing on this bar: this session is waiting on you. So `blocked` is
        // the only pair that produces it, and the only pair the silence timer must not touch.
        assert_eq!(super::bg_light_state(&fact("background", "idle", "blocked")), Some("blocked"));
        assert!(!super::bg_retirable(&fact("background", "idle", "blocked")));
        // Working: green, whatever the last hook wrote, and never retirable (status is busy).
        assert_eq!(super::bg_light_state(&fact("background", "busy", "working")), Some("running"));
        assert!(!super::bg_retirable(&fact("background", "busy", "working")));
        // Finished, and stale-and-never-started: the light stays as the hook wrote it and the
        // 5-minute timer still clears both, so decision 065's phantom lights do not come back.
        assert_eq!(super::bg_light_state(&fact("background", "idle", "done")), None);
        assert_eq!(super::bg_light_state(&fact("background", "idle", "working")), None);
        assert!(super::bg_retirable(&fact("background", "idle", "done")));
        assert!(super::bg_retirable(&fact("background", "idle", "working")));
        // Interactive sessions are untouched by either half (decision 065's own promise): a
        // terminal session idling in an open tab keeps its own light and its own state.
        assert_eq!(super::bg_light_state(&fact("interactive", "busy", "")), None);
        assert_eq!(super::bg_light_state(&fact("interactive", "idle", "blocked")), None);
        assert!(!super::bg_retirable(&fact("interactive", "idle", "")));
    }

    /// Decision 084: `state: "blocked"` reaches the bar for two different situations, and only
    /// one of them is a light the user must act on. Both `needs` values were measured from live
    /// jobs — a job told to ask a question, and a job left at an empty prompt.
    #[test]
    fn bg_blocked_needs_a_question() {
        // Asking something: orange, and the silence timer must not touch it (as before).
        let asking = needy("background", "idle", "blocked", "Should the fallback be red or blue?");
        assert_eq!(super::bg_light_state(&asking), Some("blocked"));
        assert!(!super::bg_retirable(&asking));
        // Sitting at an empty prompt: asks nothing, so the light stays as the hook wrote it and
        // the 5-minute timer clears it like any other finished job.
        let unprompted = needy("background", "idle", "blocked", "send a prompt to start");
        assert_eq!(super::bg_light_state(&unprompted), None);
        assert!(super::bg_retirable(&unprompted));
        // Still working, whatever it last said it needed: green, and never retirable.
        let busy = needy("background", "busy", "blocked", "send a prompt to start");
        assert_eq!(super::bg_light_state(&busy), Some("running"));
        assert!(!super::bg_retirable(&busy));
        // Unrecognised or absent `needs` keeps the orange: a missed attention light costs more
        // than a light that lingers (this is also the pre-084 behaviour, unchanged).
        assert_eq!(super::bg_light_state(&fact("background", "idle", "blocked")), Some("blocked"));
        assert!(!super::bg_retirable(&fact("background", "idle", "blocked")));
    }

    /// The first live pid that owns a controlling terminal, or None on a machine with no
    /// terminal session open at all. Used to model a *real* interactive CLI session, which
    /// is the thing decision 064's spare rule has to keep telling apart from a spare.
    fn pid_with_tty() -> Option<i64> {
        let out = std::process::Command::new("ps").args(["-eo", "pid=,tty="]).output().ok()?;
        String::from_utf8_lossy(&out.stdout).lines().find_map(|l| {
            let mut f = l.split_whitespace();
            let pid: i64 = f.next()?.parse().ok()?;
            (f.next()? != "??").then_some(pid)
        })
    }

    /// Decision 054: a CLI light lives and dies with its `claude` process — the liveness
    /// signal that replaces the IDE lock file a terminal session never writes. Also pins
    /// the upgrade path: a status file written before 054 carries no `pid` and must fall
    /// back to the idle timeout rather than be deleted on sight. And decision 064: an
    /// unlisted light whose pid owns no terminal is a pre-warmed spare and goes on sight,
    /// while the same light with a terminal behind it keeps the full grace period.
    /// Ignored because it sets AGENTSTATUS_DIR, which is process-global:
    ///
    ///   cargo test -- --ignored --nocapture cli_liveness_pruning
    #[test]
    #[ignore]
    fn cli_liveness_pruning() {
        let tmp = std::env::temp_dir().join(format!("agentstatus-054-{}", std::process::id()));
        let sessions = tmp.join("sessions");
        std::fs::create_dir_all(&sessions).unwrap();
        std::env::set_var("AGENTSTATUS_DIR", &tmp);

        let now = super::now_unix();
        let write = |id: &str, pid: Option<i64>| {
            let mut v = serde_json::json!({
                "state": "running", "cwd": env!("CARGO_MANIFEST_DIR"), "ide": "cli",
                "label": "src-tauri", "updated_at": now, "task": "", "detail": ""
            });
            if let Some(p) = pid {
                v["pid"] = serde_json::json!(p);
            }
            std::fs::write(sessions.join(format!("{id}.json")), v.to_string()).unwrap();
        };
        // None of these ids is one Claude Code knows, so every one of them is "unlisted" and
        // decision 064's rule is what decides between them — exactly the situation a spare
        // creates. A pid that owns a tty stands in for a real interactive session; pid 1
        // (launchd) is always alive and never has one, which is a spare's signature.
        // macOS caps pids at 99999, so 999999 can never name a live process.
        let real = pid_with_tty();
        write("alive", Some(real.unwrap_or(std::process::id() as i64)));
        write("spare", Some(1));
        write("dead", Some(999_999));
        write("legacy", None);

        let ids: Vec<String> = super::list_sessions().into_iter().map(|s| s.id).collect();
        assert!(!ids.contains(&"dead".to_string()), "dead CLI session survived: {ids:?}");
        assert!(ids.contains(&"legacy".to_string()), "pre-054 file pruned: {ids:?}");
        // The spare rule can only fire when the query actually answered; a machine where
        // `claude agents --json` fails reconciles nothing, by design (fail-open).
        if super::cli_facts(now).is_some() {
            assert!(!ids.contains(&"spare".to_string()), "spare light survived: {ids:?}");
        } else {
            println!("`claude agents --json` unavailable — spare assertion skipped");
        }
        match real {
            Some(_) => assert!(ids.contains(&"alive".to_string()), "live CLI pruned: {ids:?}"),
            // Every process here is detached (a background agent runs the suite this way),
            // so there is no interactive session to model and nothing to assert.
            None => println!("no terminal session on this machine — 'alive' assertion skipped"),
        }
        std::fs::remove_dir_all(&tmp).ok();
    }

    /// Decision 080: the manual prune deletes exactly what an automatic prune deletes —
    /// the session's status file and its subagent markers — and nothing else, and a id
    /// that is not a bare session id never reaches the filesystem at all.
    /// Ignored because it sets AGENTSTATUS_DIR, which is process-global:
    ///
    ///   cargo test -- --ignored --nocapture dismiss_deletes_one_session
    #[test]
    #[ignore]
    fn dismiss_deletes_one_session() {
        let tmp = std::env::temp_dir().join(format!("agentstatus-080-{}", std::process::id()));
        let sessions = tmp.join("sessions");
        std::fs::create_dir_all(&sessions).unwrap();
        std::env::set_var("AGENTSTATUS_DIR", &tmp);

        for id in ["keep", "drop"] {
            std::fs::write(sessions.join(format!("{id}.json")), "{}").unwrap();
            std::fs::create_dir_all(sessions.join(format!("{id}.subagents"))).unwrap();
            std::fs::write(sessions.join(format!("{id}.subagents/a1")), "Explore").unwrap();
        }

        assert!(super::dismiss_session("drop".into()), "dismiss reported no deletion");
        assert!(!sessions.join("drop.json").exists(), "status file survived");
        assert!(!sessions.join("drop.subagents").exists(), "subagent markers survived");
        assert!(sessions.join("keep.json").exists(), "dismiss touched another session");
        assert!(sessions.join("keep.subagents/a1").exists(), "dismiss touched another session");

        // Unknown id: nothing to delete, and it says so rather than claiming success.
        assert!(!super::dismiss_session("never-existed".into()), "unknown id reported a deletion");
        // A path, not a session id — rejected before it is joined onto the status dir.
        assert!(!super::dismiss_session("../keep".into()), "traversal accepted");
        assert!(!super::dismiss_session("".into()), "empty id accepted");
        assert!(sessions.join("keep.json").exists(), "traversal deleted a file");

        std::fs::remove_dir_all(&tmp).ok();
    }

    /// Exercise the real click path for a CLI light and report what it resolved, so a
    /// "clicking does nothing" report can be pinned to a cause instead of guessed at:
    ///
    ///   AGENTSTATUS_TEST_PID=<pid> AGENTSTATUS_TEST_SESSION=<id> \
    ///     cargo test -- --ignored --nocapture focus_terminal_live
    ///
    /// A background agent prints tty=None and is expected to do nothing.
    #[test]
    #[ignore]
    fn focus_terminal_live() {
        let pid: i64 = std::env::var("AGENTSTATUS_TEST_PID")
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .expect("set AGENTSTATUS_TEST_PID");
        let sid = std::env::var("AGENTSTATUS_TEST_SESSION").unwrap_or_default();
        println!("pid          = {pid}");
        let tty = super::tty_of(pid);
        println!("tty          = {tty:?}");
        println!("terminal app = {:?}", super::terminal_app_of(pid));
        println!("session title= {:?}", super::claude_ai_title(&sid));
        match &tty {
            Some(t) => super::focus_terminal_session(pid, t, &sid),
            None => {
                println!("detached -> claude attach {}", sid.split('-').next().unwrap_or(""));
                super::attach_background_agent(&sid);
            }
        }
        println!("(done)");
    }

    /// Click a light, exactly as the bar does — `focus_session` itself, with the routing and
    /// the guards, not the leaf it happens to pick:
    ///
    ///   AGENTSTATUS_TEST_SESSION=<id> cargo test -- --ignored --nocapture focus_click_live
    ///
    /// Run it twice against a background session: the second click must reach the terminal the
    /// first one opened, not open a second (decision 061).
    #[test]
    #[ignore]
    fn focus_click_live() {
        let sid = std::env::var("AGENTSTATUS_TEST_SESSION").expect("set AGENTSTATUS_TEST_SESSION");
        let path = super::sessions_dir().join(format!("{sid}.json"));
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("no status file")).unwrap();
        let cwd = v.get("cwd").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let ide = v.get("ide").and_then(|x| x.as_str()).unwrap_or("").to_string();
        println!("clicking {} (ide={ide})", &sid[..8.min(sid.len())]);
        super::focus_session(cwd, ide, sid);
        println!("(done)");
    }

    /// Print the resolved `claude` binary and what it answers — the query every CLI light
    /// and every CLI click is reconciled against (decisions 054, 064):
    ///
    ///   cargo test --release -- --ignored --nocapture dump_cli_facts
    ///
    /// `query FAILED` means the binary could not be reached, which is fail-open: no CLI light
    /// is ever pruned and no click is ever suppressed. Run it under a stripped environment to
    /// reproduce how the app starts from Login Items:
    ///
    ///   env -i HOME="$HOME" PATH=/usr/bin:/bin cargo test --release -- --ignored dump_cli_facts
    #[test]
    #[ignore]
    fn dump_cli_facts() {
        println!("claude bin = {}", super::claude_bin());
        match super::cli_facts_query() {
            Some(m) => {
                println!("{} session(s)", m.len());
                for (id, f) in m {
                    println!(
                        "  {} kind={} pid={} status={} state={} needs={:?} -> light={:?} retirable={}",
                        &id[..8.min(id.len())],
                        f.kind,
                        f.pid,
                        f.status,
                        f.job_state,
                        f.needs,
                        super::bg_light_state(&f),
                        super::bg_retirable(&f)
                    );
                }
            }
            None => println!("query FAILED"),
        }
    }

    /// Resolve a live session to the Ghostty surface a click would focus, and focus it —
    /// the whole of decision 055's join in one command, so a miss can be pinned to the
    /// title lookup or to the AppleScript match rather than guessed at:
    ///
    ///   AGENTSTATUS_TEST_SESSION=<id> cargo test -- --ignored --nocapture focus_ghostty_live
    ///
    /// `title = None` means Claude has not titled the session yet (nothing to match on, so
    /// a click correctly falls back to fronting Ghostty). `focused = false` with a title
    /// present means no surface, or more than one, contained it.
    #[test]
    #[ignore]
    fn focus_ghostty_live() {
        let sid = std::env::var("AGENTSTATUS_TEST_SESSION").expect("set AGENTSTATUS_TEST_SESSION");
        println!("title   = {:?}", super::claude_ai_title(&sid));
        println!("focused = {}", super::focus_ghostty_surface(&sid, true));
    }

    /// Print every row title in Cursor's tray menu without pressing anything — the ground
    /// truth for the title format the click path matches against (decision 049):
    ///
    ///   cargo test --release -- --ignored --nocapture cursor_dump_tray
    ///
    /// No output at all means the AX read failed (Cursor not running, or the terminal
    /// lacks Accessibility), not that the menu is empty.
    #[test]
    #[ignore]
    fn cursor_dump_tray() {
        super::cursor_press_tray_row(&|t| {
            println!("row {t:?}");
            false
        });
    }

    /// Print the session-id → session-name map the tooltip's identifying line is built
    /// from (decision 053). Ground truth for the join, since these names come from a
    /// directory Claude Code owns and the hook never writes:
    ///
    ///   cargo test --release -- --ignored --nocapture dump_session_names
    ///
    /// Read-only. An id on the bar that's missing here has no record, and its tooltip
    /// falls back to the project folder alone.
    #[test]
    #[ignore]
    fn dump_session_names() {
        for (id, f) in super::claude_session_facts() {
            println!("{id} -> name={} status={} statusUpdatedAt={}", f.name, f.status, f.status_updated_ms);
        }
    }

    /// For a "the light is stuck green" report: print, per light, what the hook wrote, what
    /// Claude Code says, and whether decision 067's reconcile fires. A stuck green light shows
    /// as `hook=running` + `status=idle` + `reconcile=GREY`; if it says `reconcile=-` while
    /// `status=idle`, the guard declined and the two timestamps printed say why.
    ///
    ///   cargo test -- --ignored --nocapture dump_turn_reconcile
    #[test]
    #[ignore]
    fn dump_turn_reconcile() {
        let facts = super::claude_session_facts();
        let dir = super::sessions_dir();
        let Ok(entries) = std::fs::read_dir(&dir) else { return };
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|s| s.to_str()) else { continue };
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
            let hook = v.get("state").and_then(|x| x.as_str()).unwrap_or("-");
            let upd = v.get("updated_at").and_then(|x| x.as_i64()).unwrap_or(0);
            let f = facts.get(id);
            let greys = hook == "running" && super::turn_ended(f, upd);
            println!(
                "{:.8}  hook={:<8} at={}  status={:<7} at={}  reconcile={}",
                id,
                hook,
                upd,
                f.map(|f| f.status.as_str()).unwrap_or("(none)"),
                f.map(|f| f.status_updated_ms).unwrap_or(0),
                if greys { "GREY" } else { "-" },
            );
        }
    }

    fn cfact(status: &str, ms: i64) -> super::ClaudeFact {
        super::ClaudeFact { name: String::new(), status: status.into(), status_updated_ms: ms }
    }

    /// Decision 067: the two guards on greying a green light from Claude Code's own status.
    #[test]
    fn interrupted_turn_greys_its_light() {
        // The case this exists for, in the numbers actually measured off the live Ctrl-C that
        // motivated it (session b7a8404f): last hook event at 1786653780, user interrupts, and
        // Claude Code writes `idle` 3.84 s later. No `Stop` ever fires.
        assert!(super::turn_ended(Some(&cfact("idle", 1_786_653_783_840)), 1_786_653_780));
        // The same turn 20 s earlier, mid-run: the light must stay green.
        assert!(!super::turn_ended(Some(&cfact("busy", 1_786_653_761_098)), 1_786_653_773));
        // Positive evidence only. `busy` and `waiting` are the session still working or still
        // asking; an absent status is every Claude Desktop session; no record at all is a host
        // that reports nothing. None of them may grey a light.
        assert!(!super::turn_ended(Some(&cfact("busy", 104_000)), 100));
        assert!(!super::turn_ended(Some(&cfact("waiting", 104_000)), 100));
        assert!(!super::turn_ended(Some(&cfact("", 104_000)), 100));
        assert!(!super::turn_ended(None, 100));
        // `shell` is not idle here — observed live, but its cause is unconfirmed (Guideline #4).
        assert!(!super::turn_ended(Some(&cfact("shell", 104_000)), 100));
        // The answer must be newer than the light, or a turn that has just started would be
        // grey for a poll: the prompt lands at t=100 and Claude Code's last word is still the
        // `idle` it wrote at t=97.
        assert!(!super::turn_ended(Some(&cfact("idle", 97_000)), 100));
        // Same second = not newer. The hook stamps whole seconds, so an `idle` written at
        // t=100.9 cannot be ordered against a hook event stamped t=100, and the tie goes to
        // the hook — the light stays green until Claude Code says so in a later second.
        assert!(!super::turn_ended(Some(&cfact("idle", 100_900)), 100));
        assert!(super::turn_ended(Some(&cfact("idle", 101_000)), 100));
    }

    /// Print the application every current light is attributed to — the exact string the
    /// tooltip shows, for the sessions actually running right now (decision 060):
    ///
    ///   cargo test -- --ignored --nocapture dump_host_apps
    #[test]
    #[ignore]
    fn dump_host_apps() {
        for s in super::list_sessions() {
            println!("{} ide={:<15} app={}", &s.id[..8.min(s.id.len())], s.ide, s.app);
        }
    }

    /// The tray row for a running composer carries a status suffix, and a notified one a
    /// bullet; both must still resolve to the composer's name (decision 049).
    #[test]
    fn tray_row_matching() {
        assert!(super::tray_row_is("Folder upload", "Folder upload"));
        assert!(super::tray_row_is("Folder upload, Running", "Folder upload"));
        assert!(super::tray_row_is("\u{2022} Folder upload, Running", "Folder upload"));
        assert!(!super::tray_row_is("Folder upload functionality", "Folder upload"));
    }

    /// The veto that keeps a working Cursor agent's light green (decision 052). Only a row
    /// that says this composer is running counts; a plain row, another composer's row, or
    /// no row at all leaves the reconciler to its own judgement.
    #[test]
    fn tray_running_veto() {
        let rows: Vec<String> = ["Folder upload, Running", "\u{2022} Fix the parser", "New Agent"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(super::tray_says_running(&rows, "Folder upload"));
        // Idle rows carry no suffix, and a notified one only a bullet — neither is running.
        assert!(!super::tray_says_running(&rows, "Fix the parser"));
        // A composer too old for the tray, and an unnamed one, are both "no evidence".
        assert!(!super::tray_says_running(&rows, "Some older agent"));
        assert!(!super::tray_says_running(&rows, ""));
        // Never let one composer's row speak for another whose name it merely prefixes.
        assert!(!super::tray_says_running(&rows, "Folder"));
        // A bullet and the suffix can appear together: notified *and* still running.
        let both = vec!["\u{2022} Folder upload, Running".to_string()];
        assert!(super::tray_says_running(&both, "Folder upload"));
    }

    #[test]
    #[ignore]
    fn cursor_press_next_attention() {
        println!("pressed={}", super::cursor_open_next_attention());
    }

    /// Manual check for the Cursor reconciliation (decision 048): prints what Cursor says
    /// about every Cursor session currently on the bar, i.e. the input the light logic
    /// runs on. Run it whenever Cursor's store layout or this query changes:
    ///
    ///   cargo test --release -- --ignored --nocapture cursor_facts
    ///
    /// `None` means the query itself failed (no sqlite3/db) — the app then reconciles
    /// nothing. An id missing from the map is a composer Cursor no longer has.
    #[test]
    #[ignore]
    fn cursor_facts_live() {
        let dir = super::sessions_dir();
        let mut ids = Vec::new();
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let Ok(v) = std::fs::read_to_string(&p)
                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).map_err(Into::into))
            else {
                continue;
            };
            if v.get("ide").and_then(|x| x.as_str()) == Some("cursor") {
                if let Some(id) = p.file_stem().and_then(|s| s.to_str()) {
                    println!(
                        "session {id} state={:?} age={}s",
                        v.get("state").and_then(|x| x.as_str()).unwrap_or(""),
                        super::now_unix() - v.get("updated_at").and_then(|x| x.as_i64()).unwrap_or(0)
                    );
                    ids.push(id.to_string());
                }
            }
        }
        // What the tray says right now — the veto's whole input (decision 052).
        let titles = super::cursor_tray_titles();
        match super::cursor_facts_query(&ids) {
            None => println!("query failed"),
            Some(m) => {
                for id in &ids {
                    match m.get(id) {
                        None => println!("{id}: not in Cursor's store"),
                        Some(f) => println!(
                            "{id}: archived={} subagent={} terminal={} subagents={:?} name={:?} \
                             tray_says_running={}",
                            f.archived,
                            f.subagent,
                            f.terminal,
                            f.sub_ids,
                            f.name,
                            super::tray_says_running(&titles, &f.name)
                        ),
                    }
                }
            }
        }
    }

    /// Manual check for a Cursor session light's click path (decision 047): id → name →
    /// tray row → press, i.e. exactly what `focus_session` does for `ide == "cursor"`.
    /// Pass a Cursor session id from `~/.claude/status/sessions/`:
    ///
    ///   AGENTSTATUS_TEST_SESSION=<composer-uuid> \
    ///     cargo test --release -- --ignored --nocapture cursor_press_composer
    ///
    /// Prints the resolved name and whether its row was pressed (Cursor then opens that
    /// conversation). `name=None` means the lookup missed; `pressed=false` means no tray
    /// row carries that name (older than the 10 recents, or Cursor isn't running).
    #[test]
    #[ignore]
    fn cursor_press_composer() {
        let id = std::env::var("AGENTSTATUS_TEST_SESSION").unwrap_or_default();
        let name = super::cursor_composer_name(&id);
        println!("id={id} name={name:?}");
        let pressed = name
            .map(|n| super::cursor_press_tray_row(&|t| super::tray_row_is(t, &n)))
            .unwrap_or(false);
        println!("pressed={pressed}");
    }
}

/// Path matching is shared by the live-window pruning (#027) and the click-to-focus
/// workspace lookup, and its rules differ per platform (decision 070), so it is pinned here
/// rather than only exercised through them.
#[cfg(test)]
mod path_matching {
    use super::path_within;

    #[test]
    fn matches_a_folder_and_its_subfolders() {
        assert!(path_within("/Users/x/proj", "/Users/x/proj"));
        assert!(path_within("/Users/x/proj/src", "/Users/x/proj"));
        assert!(!path_within("/Users/x/project", "/Users/x/proj"));
        assert!(!path_within("/Users/x", "/Users/x/proj"));
    }

    #[test]
    #[cfg(windows)]
    fn windows_matching_ignores_case_and_takes_either_separator() {
        assert!(path_within(r"C:\Code\AgentStatus", r"c:\code\agentstatus"));
        assert!(path_within(r"C:\Code\AgentStatus\app", r"C:\Code\AgentStatus"));
        assert!(path_within("C:/Code/AgentStatus/app", "C:/Code/AgentStatus"));
        // Still a prefix check, not a substring one.
        assert!(!path_within(r"C:\Code\AgentStatusOther", r"C:\Code\AgentStatus"));
    }

    /// Differences that carry no meaning must not defeat the match. A trailing separator in
    /// the IDE lock file's `workspaceFolders` used to disable matching for that workspace
    /// outright, and a drive-root workspace never matched at all.
    #[test]
    #[cfg(windows)]
    fn windows_matching_survives_meaningless_spelling_differences() {
        assert!(path_within(r"C:\Code\App", r"C:\Code\App\"));
        assert!(path_within(r"C:\Code\App\src", r"C:\Code\App\"));
        // Mixed separators across the two sides of the comparison.
        assert!(path_within("C:/Code/App/src", r"C:\Code\App"));
        assert!(path_within(r"C:\Code\App\src", "C:/Code/App"));
        // A drive root is a real workspace.
        assert!(path_within(r"C:\Code", r"C:\"));
        assert!(path_within(r"C:\", r"C:\"));
        // And it still must not swallow a different drive.
        assert!(!path_within(r"D:\Code", r"C:\"));
    }

    #[test]
    #[cfg(not(windows))]
    fn macos_matching_stays_exact_and_slash_only() {
        assert!(!path_within("/Users/X/Proj", "/users/x/proj"));
        assert!(!path_within("/Users/x/proj\\src", "/Users/x/proj"));
    }
}

/// The Windows click-to-focus plumbing (decision 070).
#[cfg(all(test, windows))]
mod windows_focus {
    /// Proves the raise path really finds a window by title — EnumWindows, UTF-16 decoding,
    /// and predicate matching — without needing VS Code installed, which is what makes it
    /// runnable on a bare machine. Whether Windows then *honours* `SetForegroundWindow` is
    /// OS focus policy, not something this code decides, so the assertion is on the match.
    ///
    /// Ignored by default because it opens a real Notepad window on the tester's screen —
    /// the same reason `cursor_press` is ignored:
    ///
    ///   cargo test --lib -- --ignored --nocapture raises_a_window_by_title
    #[test]
    #[ignore]
    fn raises_a_window_by_title() {
        // A title no other window can plausibly carry, so a match is unambiguous.
        let unique = format!("agentstatus_raise_probe_{}", std::process::id());
        let path = std::env::temp_dir().join(format!("{unique}.txt"));
        std::fs::write(&path, "AgentStatus window-raise probe").expect("write probe file");

        let mut child = std::process::Command::new("notepad.exe")
            .arg(&path)
            .spawn()
            .expect("could not start notepad");

        let mut matched = false;
        for _ in 0..40 {
            std::thread::sleep(std::time::Duration::from_millis(250));
            if super::raise_window_titled(&|t: &str| t.contains(&unique)) {
                matched = true;
                break;
            }
        }

        let _ = child.kill();
        let _ = std::fs::remove_file(&path);

        assert!(matched, "never matched a window whose title contains {unique}");
        // And it must not claim success when nothing matches, or `focus_session` would skip
        // its CLI fallback and the click would lead nowhere.
        assert!(
            !super::raise_window_titled(&|t: &str| t.contains("agentstatus_no_such_window_zz")),
            "reported a match for a title no window has"
        );
    }

    /// The ancestry walk must reach a real window for a live terminal session (decision
    /// 071) — that walk is the whole of click-to-focus for a `cli` light, and a silent
    /// failure there is a light that leads nowhere (UI Principle #3).
    ///
    /// Ignored: it needs a live session and it raises a real window on the tester's screen.
    ///
    ///   cargo test --lib -- --ignored --nocapture focus_host_window_reaches_a_window
    #[test]
    #[ignore]
    fn focus_host_window_reaches_a_window() {
        let Ok(entries) = std::fs::read_dir(super::sessions_dir()) else {
            println!("no status directory — skipped");
            return;
        };
        let mut found = None;
        for e in entries.flatten() {
            let path = e.path();
            let Some(sid) = path.file_stem().and_then(|s| s.to_str()).map(str::to_string) else {
                continue;
            };
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
            let ide = v.get("ide").and_then(|x| x.as_str()).unwrap_or("");
            let pid = v.get("pid").and_then(|x| x.as_i64()).unwrap_or(0);
            if matches!(ide, "cli" | "claude-desktop") && pid > 0 {
                found = Some((ide.to_string(), pid, sid));
                break;
            }
        }
        let Some((ide, pid, sid)) = found else {
            println!("no live cli/desktop session with a recorded pid — skipped");
            return;
        };
        let title = super::claude_ai_title(&sid);
        println!("ide={ide} pid={pid} session={sid} title={title:?}");
        let picked = super::host_window(pid, &sid);
        println!("picked window = {:?}", picked.as_ref().map(|(_, t)| t));
        let (_, window_title) = picked.unwrap_or_else(|| {
            panic!("no ancestor of pid {pid} owns a visible window for session {sid} — its light would click into nothing")
        });
        // And it must be *that* session's window, not merely one of the host's (decision 077).
        if let Some(title) = title {
            assert!(
                window_title.contains(&title),
                "picked {window_title:?}, which is not the window showing {title:?}"
            );
        }
    }

    /// The rule that picks between the several windows one host process owns (decision 077).
    /// Titles as the live Windows Terminal windows carry them: the session title behind an
    /// activity glyph.
    #[cfg(windows)]
    #[test]
    fn pick_window_matches_the_session_title() {
        let t = |s: &str| s.to_string();
        let three = vec![
            t("◐ Fix Windows orange input detection"),
            t("◑ Fix Claude terminal window focus on light click"),
            t("✳ Extend app support for older macOS versions"),
        ];
        assert_eq!(super::pick_window(&three, Some("Fix Claude terminal window focus on light click")), Some(1));
        assert_eq!(super::pick_window(&three, Some("Extend app support for older macOS versions")), Some(2));
        // Nothing to match on, or nothing matching: no window rather than the topmost one.
        assert_eq!(super::pick_window(&three, None), None);
        assert_eq!(super::pick_window(&three, Some("Some other session")), None);
        // One window is this host's window whatever it is titled — the ordinary single-terminal
        // case, which must keep working when Claude has not titled the session yet.
        assert_eq!(super::pick_window(&[t("PowerShell")], None), Some(0));
        assert_eq!(super::pick_window(&[], Some("anything")), None);
        // Containing the title without ending in it is a weak match: taken when unique,
        // declined when two windows share it.
        let spanning = vec![t("◐ Ship it now — building"), t("◐ Ship it later"), t("Terminal")];
        assert_eq!(super::pick_window(&spanning, Some("Ship it now")), Some(0));
        assert_eq!(super::pick_window(&spanning, Some("Ship it later")), Some(1));
        assert_eq!(super::pick_window(&spanning, Some("Ship it")), None);
    }

    /// The title rule decides where a click lands, and a wrong match is not self-correcting
    /// on Windows: a successful raise stops `focus_session` before its CLI fallback. So a
    /// sibling project whose name merely *contains* this one must not match.
    #[test]
    fn editor_titles_match_whole_segments_only() {
        const VSC: &str = "Visual Studio Code";
        assert!(super::editor_title_matches("main.rs - App - Visual Studio Code", "App", VSC));
        // No editor open: the folder is the first segment.
        assert!(super::editor_title_matches("App - Visual Studio Code", "App", VSC));
        // Insiders spells its app name with a suffix.
        assert!(super::editor_title_matches(
            "main.rs - App - Visual Studio Code - Insiders",
            "App",
            VSC
        ));
        assert!(super::editor_title_matches("proj - Cursor", "proj", "Cursor"));

        // The bug this rule exists for: a sibling folder that shares a prefix.
        assert!(!super::editor_title_matches(
            "main.rs - AppOther - Visual Studio Code",
            "App",
            VSC
        ));
        // Right project, wrong application.
        assert!(!super::editor_title_matches("App - Cursor", "App", VSC));
        // A folder name appearing only inside a filename is not the project.
        assert!(!super::editor_title_matches("App.rs - Other - Visual Studio Code", "App", VSC));
    }

    /// `vscode_exe` must answer None rather than panic or guess when VS Code is absent —
    /// that is what makes `open_in_vscode` a safe fallback on a machine without it.
    #[test]
    fn vscode_lookup_is_total() {
        match super::vscode_exe() {
            Some(p) => assert!(p.is_file(), "returned a path that is not a file: {p:?}"),
            None => {}
        }
    }
}
