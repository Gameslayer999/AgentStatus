// AgentStatus — Tauri backend.
// Reads the per-session status files written by the hook (decision 007) and
// exposes them to the frontend via the `list_sessions` command.

use serde::Serialize;
use std::path::PathBuf;
use tauri::Manager;
#[cfg(target_os = "macos")]
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

// Release-only: the self-installer runs solely from the packaged app (see the
// `ensure_installed()` call in `run`, also `#[cfg(not(debug_assertions))]`). Gating
// the module to match keeps the whole thing out of dev builds, where it would
// otherwise compile as dead code (dev uses the repo hooks via `node hooks/setup.mjs`).
#[cfg(not(debug_assertions))]
mod install;

/// Tray icon id — used to fetch the tray (`app.tray_by_id`) from the mode/image
/// commands after it's built in `setup`.
#[cfg(target_os = "macos")]
const TRAY_ID: &str = "agentstatus";

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

/// Root status directory (~/.claude/status), honoring $AGENTSTATUS_DIR (same
/// override the hook uses). $CLAUDESTATUS_DIR is kept as a legacy alias.
fn status_root() -> PathBuf {
    if let Ok(dir) = std::env::var("AGENTSTATUS_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(dir) = std::env::var("CLAUDESTATUS_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".claude").join("status")
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

/// True if `cwd` sits inside one of the live IDE workspace folders — an exact match,
/// or a subfolder a session `cd`'d into (same prefix rule as `workspace_root`). An
/// empty cwd matches nothing: it's an anonymous session no live window claims.
fn cwd_is_live(cwd: &str, folders: &[String]) -> bool {
    if cwd.is_empty() {
        return false;
    }
    folders
        .iter()
        .any(|f| cwd == f || cwd.starts_with(&format!("{f}/")))
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

/// How long the session-name map is reused (seconds). A name is fixed for the life of
/// a session (it changes only when one starts, ends, or is renamed), so re-reading the
/// directory on every ~1 s poll would be pointless I/O.
const SESSION_NAMES_TTL: i64 = 5;

/// Claude Code's own name for each session, from the per-process record it writes at
/// startup (~/.claude/sessions/<pid>.json — `sessionId`, `name`, `nameSource`). A
/// derived name is the folder plus a short suffix ("agentstatus-5b"), which is what
/// tells two sessions in the same folder apart; a renamed session carries the user's
/// own name instead. Verified present for every live session on Claude Code 2.1.200
/// and 2.1.223. Returns empty when the directory is missing or unreadable — the
/// tooltip then just shows the folder, as before.
fn claude_session_names() -> std::collections::HashMap<String, String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let dir = std::path::PathBuf::from(home).join(".claude").join("sessions");
    let mut map = std::collections::HashMap::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return map;
    };
    for e in entries.flatten() {
        let Ok(text) = std::fs::read_to_string(e.path()) else { continue };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
        let id = v.get("sessionId").and_then(|x| x.as_str()).unwrap_or("");
        let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("");
        if !id.is_empty() && !name.is_empty() {
            map.insert(id.to_string(), name.to_string());
        }
    }
    map
}

/// `claude_session_names` behind a TTL cache (same pattern as `cursor_facts`).
fn session_names(now: i64) -> std::collections::HashMap<String, String> {
    type Cache = std::sync::Mutex<(i64, std::collections::HashMap<String, String>)>;
    static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new((0, Default::default())));
    let Ok(mut guard) = cache.lock() else {
        return Default::default();
    };
    if now - guard.0 >= SESSION_NAMES_TTL {
        *guard = (now, claude_session_names());
    }
    guard.1.clone()
}

/// Whether the Cursor app is alive. Cursor sessions are NOT tracked by the
/// `~/.claude/ide/*.lock` files (only Claude Code's VS Code extension writes those),
/// so lock-pruning would nuke every Cursor light the moment any VS Code window is
/// open. Instead, Cursor lights are dropped when Cursor itself has quit (no process).
/// Fails open (keep the lights) if pgrep can't run.
fn cursor_running() -> bool {
    std::process::Command::new("pgrep")
        .args(["-x", "Cursor"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(true)
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
    let names = session_names(now);
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
            //       (decision 056): a spare never has one, so there is nothing to wait for,
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
            if window_gone || cwd_gone || cursor_gone || host_process_gone || spare_light
                || now - updated_at > MAX_IDLE_SECS
            {
                let _ = std::fs::remove_file(&path);
                let _ = std::fs::remove_dir_all(dir.join(format!("{id}.subagents")));
                continue;
            }
            let mut state = v.get("state").and_then(|x| x.as_str()).unwrap_or("idle").to_string();
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
                        Some(f)
                            if f.terminal
                                && stale
                                && !subs_live
                                && state != "error"
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
                names.get(&id).cloned().unwrap_or_default()
            };
            out.push(SessionStatus {
                id,
                state,
                cwd: cwd.to_string(),
                label: v.get("label").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                name,
                updated_at,
                task: v.get("task").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                detail: v.get("detail").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                ide: ide.to_string(),
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
#[cfg(target_os = "macos")]
fn workspace_root(cwd: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let ide_dir = std::path::PathBuf::from(home).join(".claude").join("ide");
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
                let matches = cwd == f || cwd.starts_with(&format!("{f}/"));
                if matches && f.len() > best.len() {
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
/// (decision 056). Off macOS there is no `tty_of`, so it answers `true` and the caller keeps
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
    let home = std::env::var("HOME").ok()?;
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
/// Only an unambiguous match acts: exactly one surface must contain the title, and an
/// untitled session (no `ai-title` yet) is not matched at all — its terminal reads the
/// generic "Claude Code", which names nothing. Everything else returns false and the
/// caller falls back to fronting the app, which is what every Ghostty click did before.
/// A wrong tab would be worse than no tab (UI Principle #4).
///
/// Known limit: with two Ghostty *instances* running (a background-agent attach starts
/// one), AppleScript reaches only one of them and a session in the other simply does not
/// match — that click degrades to app-level focus.
#[cfg(target_os = "macos")]
fn focus_ghostty_surface(session_id: &str) -> bool {
    let Some(title) = claude_ai_title(session_id) else {
        return false;
    };
    const SCRIPT: &str = r#"on run argv
  set target to item 1 of argv
  tell application "Ghostty"
    set hits to {}
    repeat with t in terminals
      if (name of t) contains target then set end of hits to t
    end repeat
    if (count of hits) is 1 then
      focus (item 1 of hits)
      return "ok"
    end if
  end tell
  return "no"
end run"#;
    std::process::Command::new("osascript")
        .args(["-e", SCRIPT, &title])
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
    if app == "Ghostty" && focus_ghostty_surface(session_id) {
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

/// Absolute path to the `claude` binary (decision 056).
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
/// (decision 056).
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
        // The absolute path, not a login shell: the shell a terminal opens for a scripted
        // command carries launchd's bare PATH and cannot find `claude` (see `claude_bin`).
        // Ghostty parses this string shell-style, so the path is quoted.
        let cmd = format!("\"{}\" attach {short}", claude_bin());
        // Put the agent in a **tab of the Ghostty already running** (decision 056). Decision
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
    pid: i64,
}

/// How long a `claude agents --json` answer is reused, and how long an unlisted CLI light is
/// tolerated before it is treated as a pre-warmed spare rather than a session.
const CLI_FACTS_TTL: i64 = 10;
const CLI_UNLISTED_SECS: i64 = 20;

/// Ask Claude Code to enumerate its own live sessions. This is the only way to tell a real
/// session from a **pre-warmed spare**: a spare is a `claude bg-spare` process that fires
/// `SessionStart` (so the hook writes a status file and a light appears) but never becomes a
/// session, and its argv is byte-identical to that of a genuine background agent — so no
/// process inspection can separate them. Spares are started by a long-lived daemon, so they
/// do not inherit `AGENTSTATUS_IGNORE` from anyone and cannot opt themselves out.
///
/// Run by absolute path, with no shell at all (decision 056). The login shell this used to go
/// through could not find `claude` when the bar was launched the way users launch it, so this
/// query — and with it every CLI reconciliation decisions 054 and 056 rest on — silently
/// returned None and reconciled nothing. Verified: the binary runs fine on a bare
/// `PATH=/usr/bin:/bin`, so no shell is needed to reach it.
fn cli_facts_query() -> Option<std::collections::HashMap<String, CliFact>> {
    let out = std::process::Command::new(claude_bin())
        .args(["agents", "--json"])
        .env("AGENTSTATUS_IGNORE", "1")
        .output()
        .ok()?;
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
            },
        );
    }
    Some(map)
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
            // whose light has not been pruned yet (decision 056), or an agent that has since
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

/// Quit the whole app from the settings panel. As an Accessory app (no Dock icon,
/// no app menu — see `setup`) the bar has no OS-provided Quit, so this button is the
/// only in-UI way out. `exit(0)` tears down the panel and tray and ends the process;
/// the hooks keep writing status files regardless, so relaunching repopulates the bar.
#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
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
    titles.into_inner()
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

/// Open the next Cursor composer that's awaiting the user, clearing that one
/// notification (decision 045). Returns true if an entry was pressed.
///
/// Cursor's tray menu (`TrayMainService.createContextMenu`, verified in the Cursor
/// 3.12.10 bundle) is a native Electron `Menu`, so every entry is a real `AXMenuItem`
/// reachable from the status item *without opening the menu* — status item →
/// `AXChildren[0]` (its `AXMenu`) → the item rows. Entries for composers with an unread
/// notification are titled `"• <name>"`; pressing one sends `vscode:openComposer` to
/// that composer's window and focuses it, which marks it read. So one `AXPress` both
/// jumps the user to the waiting composer and decrements Cursor's own count — verified
/// live: count `" 2"` → `" 1"` with the pressed entry's bullet gone.
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
    // The press opens the composer and clears its notification, but it does not bring
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
#[tauri::command]
fn set_mode(app: tauri::AppHandle, mode: String) {
    #[cfg(target_os = "macos")]
    {
        let menubar = mode == "menubar";
        let app2 = app.clone();
        // NSStatusItem must be manipulated on the main thread; Tauri commands run on a
        // background thread, so marshal there. Window show/hide is marshaled by Tauri
        // internally, but we do it here too so it stays ordered with the tray change.
        let _ = app.run_on_main_thread(move || {
            let has_tray = match app2.tray_by_id(TRAY_ID) {
                Some(tray) => {
                    let _ = tray.set_visible(menubar);
                    true
                }
                None => false,
            };
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
    }
}

/// Paint the tray icon from RGBA pixels the webview rendered (the row of colored dots,
/// or a single summary dot when condensed). Reusing the webview's canvas keeps one
/// source of truth for the per-state colors (decision 017) instead of redrawing them
/// in Rust. Called only in menu-bar mode, and only when the image actually changed
/// (the frontend signature-skips unchanged frames), so this is cheap at the 1 Hz poll.
#[tauri::command]
fn set_tray_image(app: tauri::AppHandle, rgba: Vec<u8>, width: u32, height: u32) {
    #[cfg(target_os = "macos")]
    {
        if width == 0 || height == 0 || rgba.len() != (width as usize) * (height as usize) * 4 {
            return;
        }
        let app2 = app.clone();
        // set_icon touches the NSStatusItem → main thread only (see set_mode).
        let _ = app.run_on_main_thread(move || {
            if let Some(tray) = app2.tray_by_id(TRAY_ID) {
                let img = tauri::image::Image::new_owned(rgba, width, height);
                let _ = tray.set_icon(Some(img));
                // Force color rendering: a template icon is drawn as a monochrome
                // alpha mask (all opaque pixels → black/white), which swallows our
                // per-state colors. The builder flag doesn't survive set_icon, so
                // re-assert it on every image.
                let _ = tray.set_icon_as_template(false);
            }
        });
    }
}

/// Toggle the panel as a popover anchored under the tray icon. A left-click on the
/// tray item shows the panel centered below the click point (just under the menu bar);
/// a second click hides it. The panel keeps its NSPanel properties across hide/show, so
/// per-light click, hover, and badges work exactly as in floating mode. `cx`/`cy` are
/// the click's physical screen coordinates (the cursor sits in the menu bar on click).
#[cfg(target_os = "macos")]
fn toggle_popover(win: &tauri::WebviewWindow, cx: f64, cy: f64) {
    if matches!(win.is_visible(), Ok(true)) {
        let _ = win.hide();
        return;
    }
    let win_w = win.outer_size().map(|s| s.width as f64).unwrap_or(0.0);
    let x = (cx - win_w / 2.0).max(0.0);
    let y = cy + 8.0; // just below the menu bar the cursor is in
    let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
    let _ = win.show();
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

    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_nspanel::init())
        .invoke_handler(tauri::generate_handler![
            list_sessions,
            focus_session,
            set_mode,
            set_tray_image,
            cursor_attention_count,
            cursor_open_next_attention,
            quit_app
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
            // Menu-bar tray item (decision 024). Built once here (on the main thread)
            // but hidden until the frontend switches to menu-bar mode via `set_mode`.
            // Colored (not template) so the status dots show in color; left-click is
            // handled by us (popover), not a menu. Placeholder icon until the webview
            // pushes the first dot image.
            #[cfg(target_os = "macos")]
            {
                let mut tb = TrayIconBuilder::with_id(TRAY_ID)
                    .icon_as_template(false)
                    .show_menu_on_left_click(false)
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
            install::ensure_installed();
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
    /// The first live pid that owns a controlling terminal, or None on a machine with no
    /// terminal session open at all. Used to model a *real* interactive CLI session, which
    /// is the thing decision 056's spare rule has to keep telling apart from a spare.
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
    /// back to the idle timeout rather than be deleted on sight. And decision 056: an
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
        // decision 056's rule is what decides between them — exactly the situation a spare
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

    /// Print the resolved `claude` binary and what it answers — the query every CLI light
    /// and every CLI click is reconciled against (decisions 054, 056):
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
                    println!("  {} kind={} pid={}", &id[..8.min(id.len())], f.kind, f.pid);
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
        println!("focused = {}", super::focus_ghostty_surface(&sid));
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
        for (id, name) in super::claude_session_names() {
            println!("{id} -> {name}");
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
