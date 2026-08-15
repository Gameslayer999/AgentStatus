// AgentStatus — self-installer (packaged app).
//
// On launch the bundled app makes itself work with zero external steps: it puts the status
// hook in a stable location and registers it in the user's Claude hook config. Idempotent
// (safe every launch), reversible (a one-time backup), and non-clobbering (only touches its
// own hook entries).
//
// **Which hook depends on the platform (decision 068).** Windows installs the native
// `agentstatus-hook` binary, shipped as a Tauri bundle resource: `report.sh` costs ~210 ms
// per event there against the binary's ~26 ms, and needs a `jq` that Windows does not have.
// macOS keeps writing the embedded `report.sh` until the port has been diffed against a live
// macOS session — there are no Windows users to regress and macOS users to protect.
//
// Either way the installed hook lives *outside* the app bundle, so it keeps working if the
// app is moved. Gated to release builds — in dev we point at the repo's hooks/ via
// `node hooks/setup.mjs`.

use std::path::{Path, PathBuf};

#[cfg(not(windows))]
const REPORT_SH: &str = include_str!("../../../hooks/report.sh");

/// Hook entries this app owns, in **any** version it has ever shipped. Matching both is what
/// makes an upgrade *replace* its own registration rather than sit alongside the old one —
/// without this, a Windows user upgrading from a `report.sh` build would have two hooks
/// firing per event, each writing the same status file.
const HOOK_MARKERS: &[&str] = &["report.sh", "agentstatus-hook"];

/// The staged resource path (see `hooks/stage-hook.mjs` and `tauri.windows.conf.json`) and
/// the name it is installed under.
#[cfg(windows)]
const HOOK_BIN: &str = "agentstatus-hook.exe";
#[cfg(windows)]
const HOOK_RESOURCE: &str = "resources/agentstatus-hook.exe";

// Same event set as hooks/setup.mjs. Tool events take a "*" matcher.
const SIMPLE_EVENTS: &[&str] = &[
    "SessionStart", "UserPromptSubmit", "Stop", "SessionEnd", "StopFailure",
    "SubagentStart", "SubagentStop",
];
const TOOL_EVENTS: &[&str] = &["PreToolUse", "PostToolUse", "PostToolUseFailure", "PermissionRequest"];

/// `HOME` is what the shell hook and macOS use; a Windows GUI process has only
/// `USERPROFILE` (`HOME` is unset unless something like Git Bash sets it), so fall back
/// rather than resolve every config path against an empty string.
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

fn claude_dir() -> PathBuf {
    home().join(".claude")
}

fn status_dir() -> PathBuf {
    match std::env::var("AGENTSTATUS_DIR") {
        Ok(d) if !d.is_empty() => PathBuf::from(d),
        _ => match std::env::var("CLAUDESTATUS_DIR") {
            Ok(d) if !d.is_empty() => PathBuf::from(d),
            _ => claude_dir().join("status"),
        },
    }
}

/// Best-effort: never panics, never blocks the app if it fails.
pub fn ensure_installed(app: &tauri::AppHandle) {
    if let Err(e) = try_install(app) {
        eprintln!("AgentStatus: self-install skipped: {e}");
    }
}

/// Put the platform's hook in `~/.claude/status/` and return the command Claude Code should
/// run for it.
///
/// The command string is handed to a **shell** — Git Bash on Windows — so on that platform
/// the path is written with forward slashes (a backslash is an escape inside a bash word)
/// and quoted (a home directory can contain spaces). Verified live: a forward-slash Windows
/// path executes correctly as a hook command. macOS keeps the exact unquoted form it has
/// always registered, so nothing about its settings.json changes.
#[cfg(windows)]
fn install_hook(app: &tauri::AppHandle, status: &Path) -> std::io::Result<String> {
    use tauri::Manager;

    let src = app
        .path()
        .resolve(HOOK_RESOURCE, tauri::path::BaseDirectory::Resource)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e.to_string()))?;
    let dst = status.join(HOOK_BIN);
    install_binary(&src, &dst)?;
    sweep_replaced_binaries(status);
    Ok(format!("\"{}\"", dst.to_string_lossy().replace('\\', "/")))
}

#[cfg(not(windows))]
fn install_hook(_app: &tauri::AppHandle, status: &Path) -> std::io::Result<String> {
    let script = status.join("report.sh");
    std::fs::write(&script, REPORT_SH)?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))?;
    Ok(script.to_string_lossy().to_string())
}

/// Copy the bundled hook binary into place, skipping the write when the bytes already match
/// — which is every launch after the first, and keeps us from rewriting a file that hook
/// processes are actively executing.
///
/// When it *has* changed, the destination may still be locked by a hook mid-run (they fire
/// on every tool call), so fall back to the standard Windows replace-a-running-executable
/// move: rename the old one aside, copy, and let `sweep_replaced_binaries` collect it later
/// once no process holds it.
#[cfg(windows)]
fn install_binary(src: &Path, dst: &Path) -> std::io::Result<()> {
    if let (Ok(a), Ok(b)) = (std::fs::read(src), std::fs::read(dst)) {
        if a == b {
            return Ok(());
        }
    }
    if std::fs::copy(src, dst).is_ok() {
        return Ok(());
    }
    let aside = dst.with_extension(format!("old-{}", std::process::id()));
    std::fs::rename(dst, &aside)?;
    let result = std::fs::copy(src, dst).map(|_| ());
    let _ = std::fs::remove_file(&aside);
    result
}

/// Delete any `agentstatus-hook.old-*` left behind by a previous upgrade. Best-effort: one
/// still held by a running process simply stays until the next launch.
#[cfg(windows)]
fn sweep_replaced_binaries(status: &Path) {
    let Ok(entries) = std::fs::read_dir(status) else { return };
    for e in entries.flatten() {
        let name = e.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("agentstatus-hook.old-") {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

fn try_install(app: &tauri::AppHandle) -> std::io::Result<()> {
    // 1. Put the hook somewhere stable and app-independent.
    let status = status_dir();
    std::fs::create_dir_all(status.join("sessions"))?;
    let command = install_hook(app, &status)?;

    // 2. Merge our hooks into the Claude user-level hook config.
    merge_hooks(
        claude_dir().join("settings.json"),
        claude_dir().join("settings.json.agentstatus-bak"),
        &command,
        SIMPLE_EVENTS,
        TOOL_EVENTS,
    )?;

    // 3. Strip hooks a past version registered into Codex and Antigravity
    //    (decision 040). Without this, upgrading leaves orphaned entries that keep
    //    invoking report.sh from an unsupported host.
    cleanup_legacy_hosts();

    Ok(())
}

/// Remove the Codex and Antigravity hook entries earlier versions installed. Runs on
/// every launch (cheap: two `exists` checks when there's nothing to clean) and never
/// creates a file that isn't already there. Best-effort throughout — a failure here
/// must not stop the app from starting.
fn cleanup_legacy_hosts() {
    // Codex: Claude-shaped `hooks` map — drop our entries, keep everyone else's.
    let codex = home().join(".codex").join("hooks.json");
    if codex.exists() {
        if let Ok(txt) = std::fs::read_to_string(&codex) {
            if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&txt) {
                let mut changed = false;
                if let Some(hooks) = v.get_mut("hooks").and_then(|h| h.as_object_mut()) {
                    for list in hooks.values_mut() {
                        if let Some(arr) = list.as_array_mut() {
                            let before = arr.len();
                            arr.retain(|e| !is_ours(e));
                            changed |= arr.len() != before;
                        }
                    }
                    hooks.retain(|_, list| list.as_array().is_none_or(|a| !a.is_empty()));
                }
                if changed {
                    if let Ok(out) = serde_json::to_string_pretty(&v) {
                        let _ = std::fs::write(&codex, out + "\n");
                    }
                }
            }
        }
    }

    // Antigravity: everything of ours lives under the top-level `agentstatus` key.
    let antigravity = home().join(".gemini").join("config").join("hooks.json");
    if antigravity.exists() {
        if let Ok(txt) = std::fs::read_to_string(&antigravity) {
            if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&txt) {
                if let Some(obj) = v.as_object_mut() {
                    if obj.remove("agentstatus").is_some() {
                        if let Ok(out) = serde_json::to_string_pretty(&v) {
                            let _ = std::fs::write(&antigravity, out + "\n");
                        }
                    }
                }
            }
        }
    }
}

/// Whether a registered hook entry is one of ours, in any version this app has shipped.
fn is_ours(entry: &serde_json::Value) -> bool {
    let s = entry.to_string();
    HOOK_MARKERS.iter().any(|m| s.contains(m))
}

fn merge_hooks(
    settings_path: PathBuf,
    backup_path: PathBuf,
    command_str: &str,
    simple_events: &[&str],
    tool_events: &[&str],
) -> std::io::Result<()> {
    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut settings = if settings_path.exists() {
        let txt = std::fs::read_to_string(&settings_path)?;
        if !backup_path.exists() {
            let _ = std::fs::write(&backup_path, &txt);
        }
        serde_json::from_str::<serde_json::Value>(&txt).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if !settings.is_object() {
        settings = serde_json::json!({});
    }
    let obj = settings.as_object_mut().unwrap();
    let hooks_val = obj.entry("hooks").or_insert_with(|| serde_json::json!({}));
    if !hooks_val.is_object() {
        *hooks_val = serde_json::json!({});
    }
    let hooks = hooks_val.as_object_mut().unwrap();

    let events = simple_events
        .iter()
        .map(|e| (*e, false))
        .chain(tool_events.iter().map(|e| (*e, true)));
    for (event, with_matcher) in events {
        let list = hooks
            .entry(event.to_string())
            .or_insert_with(|| serde_json::json!([]))
            .as_array_mut()
            .unwrap();
        // Drop any prior AgentStatus entries so re-running never duplicates — including a
        // `report.sh` entry from a build before decision 068, which would otherwise keep
        // firing alongside the binary.
        list.retain(|entry| !is_ours(entry));
        let hook = serde_json::json!({
            "type": "command",
            "command": format!("{command_str} {event}"),
        });
        let registered = if with_matcher {
            serde_json::json!({ "matcher": "*", "hooks": [hook] })
        } else {
            serde_json::json!({ "hooks": [hook] })
        };
        list.push(registered);
    }

    std::fs::write(&settings_path, serde_json::to_string_pretty(&settings)? + "\n")?;
    Ok(())
}
