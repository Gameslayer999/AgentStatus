// AgentStatus — self-installer (packaged app).
//
// On launch the bundled app makes itself work with zero external steps: it puts the status
// hook in a stable location and registers it in the user's Claude hook config. Idempotent
// (safe every launch), reversible (a one-time backup), and non-clobbering (only touches its
// own hook entries).
//
// **Both platforms install the native `agentstatus-hook` binary** (decision 068, extended to
// macOS by #076). It ships as a Tauri bundle resource. `report.sh` cost ~210 ms per event on
// Windows against the binary's ~26 ms, and needed a `jq` Windows does not have — but `jq`
// ships at `/usr/bin/jq` only on macOS 15+, so the shell hook silently no-opped for every
// macOS 13/14 user of the DMG. The binary has no dependency to be missing.
//
// The installed hook lives *outside* the app bundle, so it keeps working if the app is moved.
// Gated to release builds — in dev we point at the repo's hooks/ via `node hooks/setup.mjs`.

use std::path::{Path, PathBuf};

/// Hook entries this app owns, in **any** version it has ever shipped. Matching both is what
/// makes an upgrade *replace* its own registration rather than sit alongside the old one —
/// without this, a user upgrading from a `report.sh` build would have two hooks firing per
/// event, each writing the same status file.
const HOOK_MARKERS: &[&str] = &["report.sh", "agentstatus-hook"];

/// The staged resource path (see `hooks/stage-hook.mjs` and the two platform configs) and
/// the name it is installed under.
#[cfg(windows)]
const HOOK_BIN: &str = "agentstatus-hook.exe";
#[cfg(windows)]
const HOOK_RESOURCE: &str = "resources/agentstatus-hook.exe";
#[cfg(not(windows))]
const HOOK_BIN: &str = "agentstatus-hook";
#[cfg(not(windows))]
const HOOK_RESOURCE: &str = "resources/agentstatus-hook";

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

/// Put the hook binary in `~/.claude/status/` and return the command Claude Code should run
/// for it.
///
/// The command string is handed to a **shell** — Git Bash on Windows — so the path is quoted
/// (a home directory can contain spaces), and on Windows it is written with forward slashes
/// (a backslash is an escape inside a bash word). Verified live: a forward-slash Windows path
/// executes correctly as a hook command.
fn install_hook(app: &tauri::AppHandle, status: &Path) -> std::io::Result<String> {
    use tauri::Manager;

    let src = app
        .path()
        .resolve(HOOK_RESOURCE, tauri::path::BaseDirectory::Resource)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e.to_string()))?;
    let dst = status.join(HOOK_BIN);
    install_binary(&src, &dst)?;
    #[cfg(windows)]
    sweep_replaced_binaries(status);
    let path = dst.to_string_lossy().to_string();
    #[cfg(windows)]
    let path = path.replace('\\', "/");
    Ok(format!("\"{path}\""))
}

/// The unix half of `install_binary`. Two macOS specifics drive the shape of this:
///
/// 1. **Write beside, then `rename` over.** Writing straight onto the destination fails with
///    `ETXTBSY` while a hook process is executing it — and hooks fire on every tool call. The
///    rename is atomic, and a hook mid-run keeps the inode it already opened. No `.old-*`
///    sweep is needed: unix drops the unlinked file once the last process closes it.
/// 2. **Copy the bytes, not the file.** `fs::copy` is `fcopyfile(COPYFILE_ALL)` on macOS,
///    which carries extended attributes across — including `com.apple.quarantine` if the
///    resource inside the downloaded app bundle still has it. A quarantined unsigned
///    executable is exactly what we must not hand Claude Code to run on every tool call
///    (Agent Guideline #3), so the bytes are written to a fresh file that has no xattrs.
#[cfg(not(windows))]
fn install_binary(src: &Path, dst: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let bytes = std::fs::read(src)?;
    if std::fs::read(dst).is_ok_and(|current| current == bytes) {
        return Ok(());
    }
    let staged = dst.with_extension(format!("new-{}", std::process::id()));
    let result = std::fs::write(&staged, &bytes)
        .and_then(|_| std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755)))
        .and_then(|_| std::fs::rename(&staged, dst));
    if result.is_err() {
        let _ = std::fs::remove_file(&staged);
    }
    result
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

/// Undo the self-install, from the settings window's Uninstall button.
///
/// Two steps, both idempotent — running this with nothing installed succeeds quietly:
/// 1. Drop this app's hook entries from `~/.claude/settings.json`, matched by the same
///    `HOOK_MARKERS` the install path uses, so every other setting and every third-party
///    hook survives byte-for-byte.
/// 2. Delete the status directory (the installed hook binary and the per-session files),
///    honouring the same `AGENTSTATUS_DIR` / `CLAUDESTATUS_DIR` overrides as the install.
///
/// The `settings.json.agentstatus-bak` backup is deliberately left in place: step 1 already
/// restores the pre-install hook state, and the backup is the user's own copy of their
/// config, not ours to delete.
///
/// Errors name the file and the OS error (Agent Guideline #11) — the caller shows them
/// verbatim — and are returned *before* the app quits, so a failed uninstall leaves a
/// running app rather than a half-removed one.
pub fn uninstall() -> Result<(), String> {
    let settings = claude_dir().join("settings.json");
    remove_hooks(&settings)
        .map_err(|e| format!("Could not update {}: {e}", settings.display()))?;

    let status = status_dir();
    if let Err(e) = std::fs::remove_dir_all(&status) {
        if e.kind() != std::io::ErrorKind::NotFound {
            return Err(format!("Could not delete {}: {e}", status.display()));
        }
    }
    Ok(())
}

/// Remove exactly our entries from a Claude hook config: our hooks out of each event's
/// array, an event whose array we emptied, and the `hooks` object once it is empty.
///
/// The file is rewritten only when something of ours was actually found, so an uninstall
/// with nothing installed does not touch the user's file at all. Unparseable JSON is left
/// alone for the same reason — nothing of ours can be identified in it, so there is nothing
/// to remove and overwriting it would destroy settings we cannot read.
fn remove_hooks(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let Ok(mut settings) = serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(path)?)
    else {
        return Ok(());
    };
    let Some(obj) = settings.as_object_mut() else {
        return Ok(());
    };

    let mut changed = false;
    let mut hooks_empty = false;
    if let Some(hooks) = obj.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        for list in hooks.values_mut() {
            if let Some(arr) = list.as_array_mut() {
                let before = arr.len();
                arr.retain(|entry| !is_ours(entry));
                changed |= arr.len() != before;
            }
        }
        if changed {
            hooks.retain(|_, list| list.as_array().is_none_or(|a| !a.is_empty()));
            hooks_empty = hooks.is_empty();
        }
    }
    if !changed {
        return Ok(());
    }
    if hooks_empty {
        obj.remove("hooks");
    }
    std::fs::write(path, serde_json::to_string_pretty(&settings)? + "\n")
}

/// `remove_hooks` rewrites the user's own `settings.json`, so its blast radius is tested
/// rather than trusted: ours goes, everything else — third-party hooks, unrelated settings,
/// an unparseable file — is left exactly as it was.
///
/// These run under `cargo test --release` only, because `mod install` is gated to release
/// builds (see the gate on the module).
#[cfg(test)]
mod tests {
    use super::*;

    /// A unique scratch file per test; no `Date::now`, so the pid plus the caller's name.
    fn scratch(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("agentstatus-{}-{name}.json", std::process::id()));
        let _ = std::fs::remove_file(&p);
        p
    }

    fn write(path: &PathBuf, text: &str) {
        std::fs::write(path, text).unwrap();
    }

    fn read(path: &PathBuf) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    /// Ours out, foreign in — including a foreign entry sharing an event with ours, and an
    /// event we emptied, which is dropped rather than left as `[]`.
    #[test]
    fn removes_only_our_entries() {
        let p = scratch("mixed");
        write(&p, r#"{
          "model": "opus",
          "hooks": {
            "SessionStart": [
              {"hooks":[{"type":"command","command":"/n/notify.sh SessionStart"}]},
              {"hooks":[{"type":"command","command":"~/.claude/status/agentstatus-hook SessionStart"}]}
            ],
            "Stop": [
              {"hooks":[{"type":"command","command":"~/.claude/status/agentstatus-hook Stop"}]}
            ],
            "PreToolUse": [
              {"matcher":"Bash","hooks":[{"type":"command","command":"/n/audit.py"}]},
              {"matcher":"*","hooks":[{"type":"command","command":"~/.claude/status/report.sh PreToolUse"}]}
            ]
          }
        }"#);
        remove_hooks(&p).unwrap();
        let v: serde_json::Value = serde_json::from_str(&read(&p)).unwrap();

        assert_eq!(v["model"], "opus", "unrelated settings must survive");
        let hooks = v["hooks"].as_object().unwrap();
        assert!(!hooks.contains_key("Stop"), "an emptied event is dropped, not left as []");
        assert_eq!(hooks["SessionStart"].as_array().unwrap().len(), 1);
        assert!(read(&p).contains("notify.sh"));
        assert_eq!(hooks["PreToolUse"].as_array().unwrap().len(), 1);
        assert!(read(&p).contains("audit.py"));
        // Both markers, in any shipped form, are gone.
        assert!(!read(&p).contains("agentstatus-hook"));
        assert!(!read(&p).contains("report.sh"));

        // Idempotent: a second uninstall is a no-op, byte for byte.
        let after = read(&p);
        remove_hooks(&p).unwrap();
        assert_eq!(read(&p), after);
        let _ = std::fs::remove_file(&p);
    }

    /// When ours were the only hooks, the `hooks` object goes too — no empty husk left.
    #[test]
    fn drops_an_emptied_hooks_object() {
        let p = scratch("only-ours");
        write(&p, r#"{"model":"opus","hooks":{"Stop":[{"hooks":[{"type":"command","command":"a/agentstatus-hook Stop"}]}]}}"#);
        remove_hooks(&p).unwrap();
        let v: serde_json::Value = serde_json::from_str(&read(&p)).unwrap();
        assert!(v.get("hooks").is_none());
        assert_eq!(v["model"], "opus");
        let _ = std::fs::remove_file(&p);
    }

    /// Nothing of ours to find: the file is not rewritten at all, so a user's formatting
    /// and key order survive an uninstall that had nothing to do.
    #[test]
    fn leaves_a_foreign_config_byte_identical() {
        let p = scratch("foreign");
        let original = "{\n  \"hooks\": { \"Stop\": [ {\"hooks\":[{\"command\":\"/n/other.sh\"}]} ] },\n  \"model\": \"opus\"\n}\n";
        write(&p, original);
        remove_hooks(&p).unwrap();
        assert_eq!(read(&p), original);
        let _ = std::fs::remove_file(&p);
    }

    /// Unreadable JSON is left alone rather than clobbered: nothing of ours can be
    /// identified in it, and overwriting would destroy settings we cannot parse.
    #[test]
    fn leaves_unparseable_json_alone() {
        let p = scratch("broken");
        let original = "{ this is not json";
        write(&p, original);
        remove_hooks(&p).unwrap();
        assert_eq!(read(&p), original);
        let _ = std::fs::remove_file(&p);
    }

    /// No settings file at all is a quiet success, not an error.
    #[test]
    fn missing_file_is_ok() {
        let p = scratch("absent");
        assert!(remove_hooks(&p).is_ok());
    }
}
