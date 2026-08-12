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
    updated_at: i64,
    task: String,
    detail: String,
    /// Host surface ("cursor" or "vscode"), from the hook — drives click-to-focus.
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
             coalesce(json_extract(d.value,'$.subagentComposerIds'),'') \
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
        if f.len() < 5 {
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
            let ide = v.get("ide").and_then(|x| x.as_str()).unwrap_or("vscode");
            let window_gone = ide == "vscode" && !live_folders.is_empty() && !cwd_is_live(cwd, &live_folders);
            let cwd_gone = !cwd.is_empty() && !std::path::Path::new(cwd).exists();
            let cursor_gone = ide == "cursor" && !cursor_alive;
            if window_gone || cwd_gone || cursor_gone || now - updated_at > MAX_IDLE_SECS {
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
                        // reached us.
                        Some(f) if f.terminal && stale && !subs_live && state != "error" => {
                            state = "idle".to_string();
                        }
                        _ => {}
                    }
                }
            }
            // Cursor's badge comes from its own parent→subagent linkage (above); Claude
            // Code's from the marker files, whose Stop hook is reliable (decision 010).
            let subagents = cursor_subs.unwrap_or_else(|| read_subagents(&dir, &id));
            out.push(SessionStatus {
                id,
                state,
                cwd: cwd.to_string(),
                label: v.get("label").and_then(|x| x.as_str()).unwrap_or("").to_string(),
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
#[tauri::command]
fn focus_session(cwd: String, ide: String, session_id: String) {
    // Focus the exact session tab via the extension relay (decision 015); the window
    // raise below only gets us to the right *window*. Written first so the extension
    // can pick it up while / right after the window comes forward.
    write_focus_request(&session_id);
    #[cfg(target_os = "macos")]
    {
        // Cursor never goes through the IDE CLI (decision 047): with the Agent ("glass")
        // window active, `cursor <folder>` is intercepted by Cursor's main process
        // (`resolveGlassCliFolderTarget` → `vscode:createNewComposer {folderUri}`) and
        // opens a *new* agent in that folder instead of focusing anything. Press the
        // session's own row in Cursor's tray menu instead — that's the one thing that
        // opens the existing conversation. Falls back to raise + activate, never the CLI.
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

    /// The tray row for a running composer carries a status suffix, and a notified one a
    /// bullet; both must still resolve to the composer's name (decision 049).
    #[test]
    fn tray_row_matching() {
        assert!(super::tray_row_is("Folder upload", "Folder upload"));
        assert!(super::tray_row_is("Folder upload, Running", "Folder upload"));
        assert!(super::tray_row_is("\u{2022} Folder upload, Running", "Folder upload"));
        assert!(!super::tray_row_is("Folder upload functionality", "Folder upload"));
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
        match super::cursor_facts_query(&ids) {
            None => println!("query failed"),
            Some(m) => {
                for id in &ids {
                    match m.get(id) {
                        None => println!("{id}: not in Cursor's store"),
                        Some(f) => println!(
                            "{id}: archived={} subagent={} terminal={} subagents={:?}",
                            f.archived, f.subagent, f.terminal, f.sub_ids
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
