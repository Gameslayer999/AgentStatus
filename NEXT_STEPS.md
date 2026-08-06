# NEXT_STEPS.md — Living Build Queue

> Read this at the start of every session to pick up where the last one left off.
> Update it at the end of every session where anything changed (Agent Guideline #10).

---

## Current state

- **Unobservable sessions render hollow (decision 042).** A folder-less Cursor window fires one
  bridged `sessionStart` and then nothing (Cursor runs command hooks only with a folder open), so
  its light used to sit at a permanent, untrue `idle`. It now renders as a **hollow gray ring**
  (`unknown`) — frontend-derived from `ide == "cursor" && cwd == ""`, no hook or schema change.
  Scoped to that one verified case: `.dot.stale` is still unused, and no heartbeat-timeout
  `unknown` exists (rejected — it would eventually make healthy long-idle sessions hollow).
  Whether those rings appear at all is now a setting — **Unknown: Show | Hide** (decision 044),
  default Show.

- **Releases are automated (decision 041).** `.github/workflows/release.yml` builds the
  arm64 DMG on `macos-15` and publishes it whenever a `v*` tag is pushed; merging to `main`
  publishes nothing on its own. The job fails fast if the tag disagrees with
  `app/src-tauri/tauri.conf.json`. **Unproven on a real runner** — the YAML and the version
  guard were checked locally, but no tag has been pushed yet, so treat the first tagged
  release as the verification and watch that run.
- **Codex and Antigravity support removed (decision 040).** Supported hosts are now **Claude
  Code (VS Code)** and **Cursor** — the two ever verified against a live install. Neither
  removed host was: #033 shipped explicitly "Accepted, unverified", and the Codex path
  compensated for unconfirmed lifecycle events with a `state_5.sqlite` read, a `pgrep codex`
  probe, and a bespoke 10-min idle timeout. Removed from both installers, `report.sh` (the
  declared-host `$2` arg and every Codex/Antigravity payload shape — **the hook now reads no
  transcript at all**, so its `python3` spawn is gone too), and `lib.rs` (`read_codex_threads`,
  `codex_running`, `CODEX_*` timeouts, both click-to-focus targets). Both installers now
  **clean up** the entries earlier versions wrote to `~/.codex/hooks.json` and
  `~/.gemini/config/hooks.json`, leaving any other hooks in those files intact. Re-adding
  either host means observing real events first (Guideline #4), not restoring code.
- **Cursor stale-pruning fixed + menu-bar pip added (decision 038).** The Cursor integration
  had gone dark: decision-027 pruning treated Cursor as writing `~/.claude/ide/*.lock` files
  (it writes none — only Claude Code's VS Code extension does), so every Cursor light was
  deleted the moment any VS Code window was open. Fixed: Cursor is no longer lock-pruned; it
  is dropped when no `Cursor` process is alive, plus the 2h idle backstop.
  Also added a **Cursor menu-bar pip** — Cursor's live status is renderer-memory-only, but its
  macOS menu-bar item exposes an aggregate count of composers awaiting the user; a new Rust
  command (`cursor_attention_count`) reads it via the **Accessibility API directly** (not
  osascript — that needs Automation an unsigned rebuild lacks) and the frontend renders one
  hollow-ring pip with the count, covering the "done"/attention cue Cursor's hooks can't (its
  bridged `Stop` carries no wrap-up message). **Clicking the pip now opens the next composer
  that's waiting and clears its notification (decision 045)** — Cursor's tray menu is a native
  `NSMenu`, so `cursor_open_next_attention` presses the top `"• …"` row straight off the AX
  tree (no menu ever opens on screen); Cursor focuses that composer and marks it read, so the
  count ticks down and repeated clicks walk the queue. Falls back to just activating Cursor if
  nothing is pressable. **Verified live with the user** (`trusted=true count=1`, pip visible,
  click works; the press path confirmed ` 2` → ` 1` on Cursor 3.12.10). Getting the
  Accessibility grant to *stick* required **stable self-signing (decision 039)** — `install.sh`
  now re-signs each build with a per-machine self-signed cert, so trust persists across
  rebuilds instead of resetting every time.
- **Known Cursor 3.12 issue — hooks don't execute (`MainThreadShellExec not initialized`).**
  Observed live 2026-07-28 on Cursor 3.12.10: a **folder-open** local agent (NinthWave,
  `is_background_agent:false`) fired `sessionStart`/`beforeSubmitPrompt`/`stop` but every hook
  failed with `Error: MainThreadShellExec not initialized` (or didn't fire at all), so
  `report.sh` never ran → no `ide:"cursor"` session file → no green light. Proven NOT our bug:
  `report.sh` ran fine dozens of times for Claude Code in the same window and wrote correct
  `state=running ide=cursor` files in isolation; the app renders a green light the instant a
  cursor file exists. This is a Cursor-side shell-exec defect (a window `Reload` did not clear
  it; a full ⌘Q restart of Cursor is the usual fix). Nothing to change in AgentStatus — green
  appears automatically once Cursor executes the hook. (Also confirmed: **cloud/background**
  Cursor agents run remotely (`/home/ubuntu`, `background-composer+bc-…`) and fire no local
  hooks at all — the menu-bar pip is their only possible signal.)
- **Cursor support (decision 018) — the signal layer.** Cursor bridges Claude Code hooks (runs
  `~/.claude/status/report.sh` from `~/.claude/settings.json`), so running/idle/remove work for
  free; native `~/.cursor/hooks.json` entries ([hooks/cursor-setup.mjs](hooks/cursor-setup.mjs))
  add `subagentStart/Stop` + `postToolUseFailure` (the events the bridge drops). `report.sh`
  handles Cursor payloads (`workspace_roots` cwd, camelCase events, `empty-state-draft` skip)
  and writes an `ide` field driving per-IDE click-to-focus. Verification tooling
  ([hooks/cursor-log-events.sh](hooks/cursor-log-events.sh),
  [hooks/cursor-logger-setup.mjs](hooks/cursor-logger-setup.mjs)) kept for future Cursor
  versions. Blocked (orange) is unavailable on Cursor (no event); the menu-bar pip (038) now
  supplies the attention/"done" signal instead. **Still open:** port `cursor-setup.mjs` into
  `install.rs` so the packaged app self-installs the native Cursor hooks (today they're
  installed via `node hooks/cursor-setup.mjs install`).
- **Milestones 1–6 done — v1 complete.** Two shipping surfaces off one signal layer:
  (1) `/Applications/AgentStatus.app` — floating always-on-top bar of all sessions; self-
  installs its hooks on launch. (2) The **VS Code extension** — per-window status-bar items.
  Features: four-state lights, hover (task/activity), click-to-focus, subagent badges, floats
  over full-screen, drag + position-memory, dead-session pruning. **Remaining is polish /
  distribution only:** confirm the interim error (red) signal from live `StopFailure`;
  marketplace-publish the extension; optional launch-at-login toggle; app code signing.
  Details of each milestone below.
- **Milestone 1 complete — event model verified on Claude Code 2.1.201** (full evidence in
  [DECISIONS.md](DECISIONS.md) #006). The temporary broad logger has been **uninstalled**;
  the user's global `~/.claude/settings.json` is back to clean (permissions + theme, no
  hooks). Capture evidence retained in `logs/events.log` (gitignored). The logger tooling
  ([hooks/log-events.sh](hooks/log-events.sh), [hooks/logger-setup.mjs](hooks/logger-setup.mjs))
  stays in the repo for re-running verification on future Claude Code versions.
- **Verified signal contract (use this in Milestone 2):**
  - 🟢 running ← `UserPromptSubmit`, `PreToolUse`, `PostToolUse`
  - 🟠 blocked ← `PermissionRequest` (**not** `Notification` — that never fired; corrects the
    earlier assumption). Fires for tool approvals *and* `AskUserQuestion`; `tool_name`
    distinguishes them. No "resolved" event — infer unblocked from the next event.
  - ⚪ idle ← `Stop`, `SessionStart`
  - 🔴 error ← `PostToolUseFailure` **(interim, low confidence — noisy)**; wants a real
    turn-level `StopFailure`, not yet observed. See Milestone 2 calibration item.
  - remove ← `SessionEnd`
- **Key facts confirmed:** hooks in global settings apply **immediately to running
  sessions** (no restart); every event carries `session_id` + `cwd` (but `Stop` carries
  *only* those two — don't rely on `transcript_path`/`prompt_id` being present); a window can
  spawn **many short-lived sessions**; hooks are **session-level** (no subagent lights); the
  **workspace folder** (from `cwd`, cross-checked against `~/.claude/ide/*.lock`) is the
  window key; auto "this window" is the extension's job.
- **What we're building:** a small, always-on-top, drag-to-position bar of colored lights,
  one per open Claude Code session — 🟢 running, 🟠 blocked (waiting for input), ⚪ idle,
  🔴 error. See [CLAUDE.md](CLAUDE.md) Project Overview.
- **Architecture (two layers), all logged in [DECISIONS.md](DECISIONS.md):**
  - **Signal:** Claude Code **hooks** (decision 001) write session state to a shared JSON
    file `~/.claude/status/sessions.json` (decision 002), keyed by `session_id`.
  - **Display:** a **Tauri** borderless always-on-top window (decision 003) watches that
    file and renders the lights; sessions keyed by `session_id`, labeled by project folder,
    with heartbeat-based staleness (decision 004).
- **Event → state mapping (to be verified against the installed version — see Now):**
  `UserPromptSubmit`/`PreToolUse` → green; `Notification`/`PermissionRequest`/`Elicitation`
  → orange; `Stop`/`SessionStart` → gray; `StopFailure`/`PostToolUseFailure` → red;
  `SessionEnd` → remove.
- **Known open risk:** several of those event names come from docs and are version-dependent
  (Agent Guideline #4). The `Notification` / `Stop` / `UserPromptSubmit` / `SessionEnd` core
  is high-confidence; the error/permission events need confirming.

---

## Target architecture (working sketch)

```
AgentStatus/
├─ hooks/            # shell scripts + installer for the status hooks
│   ├─ log-events.sh   # TEMPORARY Milestone 1 event logger (remove after verification)
│   ├─ logger-setup.mjs# TEMPORARY install/uninstall for the logger
│   └─ report.sh       # (M2) the real hook: writes session state to the status file
├─ app/              # Tauri project — core floating bar + self-installer (decision 005)
│   ├─ src-tauri/    # Rust: borderless always-on-top window, status-file watcher, hook install
│   └─ src/          # web UI: renders lights from the status JSON
├─ extension/        # (later) optional VS Code extension — marketplace install + click-to-focus
├─ install.sh        # interim one-command installer until the app hosts the install logic
└─ README.md
```

**Data contract** — one file per session (decision 007):
`~/.claude/status/sessions/<session_id>.json` (dir overridable via `$AGENTSTATUS_DIR`):
```json
{
  "state": "running|blocked|idle|error",
  "cwd": "/path/to/project",
  "label": "project-name",
  "updated_at": 1751731200
}
```
The app watches the `sessions/` directory. `SessionEnd` deletes the file. Errors also append
to `~/.claude/status/calibration.log` (calibration only — no `tool_input`).

---

## Now (build queue, in order)

1. ✅ **Milestone 1 — Verify hooks.** *Done 2026-07-05.* Verified event→state mapping on
   Claude Code 2.1.201 (DECISIONS.md #006); logger uninstalled; settings clean.
2. ✅ **Milestone 2 — Signal layer.** *Done 2026-07-05.* Built [hooks/report.sh](hooks/report.sh)
   (fast, non-blocking, fail-silent; one file per session per decision 007) + the idempotent,
   reversible installer [hooks/setup.mjs](hooks/setup.mjs) (`install`/`uninstall`/`status`).
   Unit-tested every branch, replay-validated against the 160 real M1 events, and **installed
   live** — `~/.claude/status/sessions/` now updates in real time from running sessions.
   Error signal still interim (`PostToolUseFailure`, `is_interrupt==false`); `report.sh`
   mirrors failure events to `~/.claude/status/calibration.log` to confirm a real `StopFailure`
   trigger from live data over time.

## Next

3. ✅ **Milestone 3 — Tauri shell.** *Done 2026-07-05.* Rust toolchain installed; Tauri v2
   app in `app/` runs as a **non-activating NSPanel overlay** (decision 008): borderless,
   transparent, floats over everything incl. other apps' full-screen spaces, drag-to-move,
   position remembered, no Dock icon. Polls `list_sessions` (reads `~/.claude/status/sessions/`)
   and renders colored dots. Verified live floating over full-screen VS Code.
4. **Milestone 4 — Light UI + interaction.** *In progress.* Done: four-color dots;
   **click-to-focus** (click a light → focus the session's window via the IDE CLI
   `code`/`cursor <workspace-root>`, resolved from `~/.claude/ide/*.lock`; Space-aware, never
   spawns a new window — decision 016, replaced the `open -a <folder>` that duplicated windows;
   drag handle = pill padding; a fast `osascript` window-raise added for same-Space switches,
   ~0.2s vs the CLI's ~1.1s — decision 021, needs Accessibility, degrades to the CLI without it);
   dead-session pruning (instant on IDE-window close via the lock files — decision 027 — with a
   2h idle timer as backstop, replacing heartbeat-dimming);
   **hover tooltip** (task + current activity, native OS tooltip); **subagent count badge**
   (decision 009). Remaining: optional visual polish (pulse on blocked/error is in CSS;
   spacing/size tuning), and confirm the interim error (red) signal from live `StopFailure`
   data (M1/M2 calibration item).
5. ✅ **Milestone 5 — Self-installing app + installer (decisions 005, 011).** *Done
   2026-07-05.* App self-installs its hooks on launch (embedded `report.sh` →
   `~/.claude/status/report.sh` + settings.json merge, release-only); `tauri build` produces
   `.app` + `.dmg`; [install.sh](install.sh) builds + installs to `/Applications`;
   [README.md](README.md) written. Verified: launched the packaged app, it wired up all 11
   hooks (deduped, backed up, non-clobbering) with zero external steps. Installed at
   `/Applications/AgentStatus.app` and running. *Deferred:* launch-at-login is a manual
   Login-Items step (a `tauri-plugin-autostart` toggle is a future enhancement); code signing
   (unsigned → Gatekeeper right-click-Open only when redistributed).

## Later

6. ✅ **Milestone 6 — VS Code extension (decisions 005, 006, 012).** *Done 2026-07-05.*
   `extension/` shows per-window status-bar items (scoped to the window's workspace), hover
   detail (task/activity/subagents), and click-to-focus a specific session's tab via
   `claude-vscode.editor.open` (no URI prompt). Guarded hook-ensure. Packaged as `.vsix`,
   installed via the `code` CLI, verified live. **Remaining:** marketplace publish (needs a
   verified publisher account — distribution, not build).
7. **(Promoted into M4) — click-to-focus from the bar.** Clicking a light focuses that
   session's window. Approach: a Rust command opens a `vscode://` deep link
   (`tauri_plugin_opener` is already a dep), with a `code <cwd>` fallback to at least focus the
   workspace window. Exact deep-link scheme to verify against the installed version
   (UI Design Principle #3).
8. **Stretch — polish.** Position persistence across reboots, per-session titles in labels,
   configurable colors/size, optional pulse animation on blocked.
10. **If Codex or Antigravity is wanted back, verify it first (decision 040, Guideline #4).**
    Both were removed as unverified. Re-adding one starts with a real session and a logged
    event stream — confirm which events fire, their payload shapes, and where the host writes
    its hook config — *then* write code against what was observed. Also settle up front
    whether the host emits a permission-request or turn-failure event: without one its lights
    can never go orange/red, which quietly breaks UI Principle #2 for that host.
9. ✅ **Extension parity — "done" light (decision 014).** *Done 2026-07-06.* The VS Code
   extension now mirrors the bar: a finished-but-unreviewed turn (`idle && detail`) renders at
   full brightness, acknowledged idle is dimmed (`disabledForeground`); click-to-focus also
   acknowledges (app-local `reviewedAt`, keyed by finish time). Recompiled, repackaged the
   `.vsix`, reinstalled — takes effect on the next window reload.

---

## Decisions needed

- **Confirmed event→state mapping** — pending Milestone 1's real-session observations
  (may adjust the doc-sourced names in Current state).
- **Light bar visual design** — ~~orientation (horizontal/vertical)~~ **decided (decision
  015):** now user-toggleable in the settings panel. Remaining: light shape/size, spacing,
  label-on-hover vs always-on.

---

## Recently completed

- **2026-08-06** — **Released v0.6.0.** Version bumped 0.5.0 → 0.6.0 across
  `tauri.conf.json`, `package.json`, `package-lock.json`, `Cargo.toml`/`Cargo.lock`, and the
  README's download link and release-command example. Minor rather than patch: three
  user-visible additions since v0.5.0 — hollow unknown lights (#042), the Unknown Show/Hide
  setting (#044), and the click-through Cursor pip (#045, fixed by #046). Cut by pushing the
  `v0.6.0` tag, which triggers the release workflow of decision 041.

- **2026-07-30** — **Pip click now actually lands you in Cursor (decision 046).** #045's press
  cleared the notification but never brought Cursor forward: macOS focus is per-*application*,
  and an `AXPress` from a background process leaves the frontmost app unchanged, so Cursor
  raised the composer's window behind whatever the user was in. `cursor_open_next_attention`
  now activates Cursor (`open -a Cursor`, extracted as the shared `activate_cursor()` that
  `focus_session`'s empty-`cwd` branch already used) after a successful press — press first so
  Cursor selects the window, activate second so the app comes to the front. Rebuilt, signed and
  reinstalled via `./install.sh`. No README change (this restores the behavior the README
  already describes).

- **2026-07-30** — **Cursor pip clicks through the waiting composers (decision 045).** The pip
  used to only activate Cursor; it now resolves what it reports. Cursor's tray menu is a native
  Electron `Menu` (`TrayMainService.createContextMenu`), so each row is an `AXMenuItem` readable
  **and pressable without opening the menu**; rows for composers with an unread notification are
  titled `"• <name>"`. New `cursor_open_next_attention` command presses the first such row —
  Cursor sends `vscode:openComposer`, focuses that window, and marks it read, so its own count
  drops by one and the next click goes to the next waiting composer. Verified live on Cursor
  3.12.10 with a standalone Swift AX probe before wiring anything (` 2` → ` 1`, correct composer
  opened, bullet cleared), then rebuilt/reinstalled. Frontend decrements the badge immediately
  and re-reads the true count 1.5s later; falls back to activating Cursor when no row is
  pressable, so the click is never dead. README updated.
  **First build crashed the bar on every pip click** — it read a menu element out of a
  temporary `AXChildren` `CFArray`, which released it on drop, so the next AX call took a
  dangling ref (`EXC_BREAKPOINT` in `_AXUIElementValidate`). Every `CFArray` is now bound for
  as long as its elements are used. The check is a re-runnable ignored test rather than a
  hand-probe: `cargo test --release -- --ignored --nocapture cursor_press`.

- **2026-07-30** — **Settings: `Unknown` Show/Hide toggle (decision 044).** Follow-on to #042 —
  the hollow no-signal rings are now optional. New `.seg` row after `Sort`, backed by
  `localStorage` (`agentstatus.showunknown`), defaulting to **Show** so nothing changes for
  existing users (a cleared pref and **Reset to defaults** also mean Show). `latestSessions` stays
  the complete poll and a new `visibleSessions()` filters at draw time, so the bar and the
  menu-bar tray agree and toggling repaints instantly instead of waiting for the next 1s poll.
  Rejected filtering in Rust `list_sessions` — a display pref doesn't belong in the backend.
  Rebuilt/reinstalled and confirmed the default path live; README + settings-panel art updated.
  **The Show/Hide click itself is user-confirmed** (opening the panel needs a right-click on the
  bar).

- **2026-07-30** — **Hollow "unknown" light for unobservable sessions (decision 042).** A
  running Cursor session showed a blank colorless light; diagnosed from Cursor 3.12.10's own
  hook logs as a **folder-less window** — its only event ever was one bridged `sessionStart`
  with `workspace_roots: []`, and Cursor requests no further hook steps without a folder open
  (zero `beforeSubmitPrompt`/`preToolUse`/`stop` all day, vs. 1190 `preToolUse` in the
  folder-open log generation). Its recorded `idle` was therefore one stale event, rendered as a
  confident dim gray dot — a lying light (UI Principle #4). Fixed frontend-only: `displayState()`
  returns `unknown` when `ide == "cursor" && cwd == ""`, and `.dot.unknown` draws a **hollow gray
  ring** (no glow, no pulse, borrowed `--c-idle`) that says "a session is here, its state is
  unreadable". Tooltip names the cause; `URGENCY_RANK`/`TRAY_PRIORITY`/`drawTray()` handle the new
  state (the tray strokes a matching ring); no chime, no new color setting. **No hook or
  status-file schema change** — both fields were already there. Verified live: rebuilt via
  `./install.sh` and screenshotted the running bar with the real Cursor session showing a ring.
  README art regenerated to six states.

- **2026-07-29** — **Automated releases (decision 041) and cut v0.5.0.** Added
  `.github/workflows/release.yml` — the repo's first CI. Tag-triggered rather than
  main-triggered, so releasing stays a deliberate act instead of a side effect of merging;
  a version guard fails the run before building if the tag and `tauri.conf.json` disagree.
  Version bumped 0.4.2 → 0.5.0 across `tauri.conf.json`, `package.json`,
  `package-lock.json`, `Cargo.toml`/`Cargo.lock`, and the README download link (minor, not
  patch: two supported hosts were removed). Workflow YAML parsed and the guard's shell logic
  tested against both a matching and a mismatched tag; the build-and-publish path itself is
  unverified until the first tag is pushed.

- **2026-07-29** — **Removed Codex and Antigravity support (decision 040).** Both hosts were
  built on unverified event contracts — #033 shipped as "Accepted, unverified", and the Codex
  path substituted a `state_5.sqlite` read, a `pgrep codex` liveness probe, and a bespoke
  10-min idle timeout for lifecycle events that were never confirmed to fire. An unverified
  host can only produce lying lights (UI Principle #4). Stripped from `report.sh` (the
  declared-host `$2` arg, Codex id/`env.PWD` fallbacks, `PreInvocation`/`PostInvocation`,
  Antigravity's `workspacePaths[]`/`toolCall.*` shapes and aliased tool names, the
  `<USER_REQUEST>` unwrap **and the whole transcript read + `python3` spawn**), from
  `setup.mjs`/`install.rs` (Claude-only install now), and from `lib.rs`
  (`read_codex_threads`, `codex_running`, `label_from_cwd_or_title`, `CODEX_ACTIVE_SECS`/
  `CODEX_IDLE_SECS`, both hosts' pruning and click-to-focus branches). Both installers now
  clean up the entries earlier versions wrote to `~/.codex/hooks.json` and
  `~/.gemini/config/hooks.json`, preserving other hooks in those files and never creating a
  file that isn't there. Verified: `cargo check` clean in debug **and** release; `report.sh`
  smoke-tested running→blocked→idle→SessionEnd for Claude plus a Cursor
  `beforeSubmitPrompt` (`ide:"cursor"`, cwd from `workspace_roots[]`); installer exercised
  against a throwaway `$HOME` seeded with an old install (our entries removed, a foreign
  `~/.codex/hooks.json` hook and an unrelated `otherplugin` key both survived, nothing
  created when the files were absent). README and DECISIONS updated.

- **2026-07-28** — **Fixed the dead Cursor integration + added a menu-bar pip (decision 038).**
  Root cause the old integration produced no lights: the bar's decision-027 pruning treated
  Cursor as writing `~/.claude/ide/*.lock` files, but only Claude Code's VS Code extension does
  — so a Cursor session's `cwd` matched no live lock and was deleted every poll whenever any VS
  Code window was open (confirmed: all locks on the machine were `ideName: Visual Studio Code`).
  Fixed in `app/src-tauri/src/lib.rs`: removed `cursor` from `uses_ide_locks`, added
  `cursor_running()` (`pgrep -x Cursor`) so Cursor lights drop when Cursor quits, keeping the 2h
  idle backstop. Investigated the actual menu-bar item (Cursor 3.12.10 bundle): it's a
  notification/unread-count indicator, **not** a live spinner (two icon states + a count from a
  composer snapshot; live status is renderer-memory-only). So added `cursor_attention_count`
  (reads the item's AX title via `osascript`, fails closed to 0) and a frontend hollow-ring pip
  (`.cursor-pip`) showing the count of composers awaiting the user — the "done"/attention cue
  Cursor's hooks can't give (bridged `Stop` has no wrap-up, verified). Pip click activates
  `Cursor.app`. Verified: synthetic non-lock Cursor session now survives pruning in the running
  app; `report.sh` tags `ide:"cursor"` correctly; the exact `osascript` returns the count.
  Rebuilt + reinstalled via `install.sh`. `README.md`/`DECISIONS.md` updated. **Left to verify
  live:** a real Cursor agent run + the pip after re-granting Accessibility.

- **2026-07-28** — **Cleared build warnings (decision 037).** `cargo build` / `tauri dev`
  emitted a batch of warnings; all are gone from the `app` crate now. (1) Gated
  `mod install;` to `#[cfg(not(debug_assertions))]` — the self-installer's only caller is
  already release-gated, so in a dev build the whole module read as dead code; same for the
  release-only `let mut builder` (added `#[allow(unused_mut)]`). (2) Replaced
  `tauri-nspanel`'s deprecated `cocoa`-typed `set_collection_behaviour` in
  `make_overlay_panel` with `objc2-app-kit`'s typed `NSWindow::setCollectionBehavior` on the
  window pointer from `WebviewWindow::ns_window()` — same object, same two flags
  (`FullScreenAuxiliary | CanJoinAllSpaces`), no behavior change. Added `objc2-app-kit` as a
  macOS-only dep pinned to `0.3` (already in the tree via Tauri, no second copy). One
  transitive warning remains (`block v0.1.6`, inside `tauri-nspanel`), outside this repo.

- **2026-07-28** — **New app icon + README format overhaul + v0.4.1 (decision 036).**
  (1) Replaced the off-brand Tauri-template swirl icon with **three glowing status lights**
  (green/orange/red) on a dark Big-Sur squircle — a reproducible SVG master at
  `docs/icon-master.svg` (+ 1024px PNG); `cd app && npx tauri icon ../docs/icon-master.png`
  regenerates the whole macOS icon set, and a 256px export is `docs/logo.png`, reused as the
  README header logo. Chosen from five generated candidates + a user ChatGPT option, weighted
  on 32px legibility. (2) **README format overhaul** to match convention (surveyed Stats, Ice,
  Rectangle, starship, bat, lazygit, React): centered header block (logo → name → one-line
  tagline → release/downloads/platform badges → nav links), a **menu-bar mode** section (was
  undocumented — decision 026), and Gatekeeper/full-screen caveats moved into `> [!IMPORTANT]`
  / `> [!NOTE]` callouts. (3) Bumped `0.4.0 → 0.4.1` (`tauri.conf.json`, `package.json`,
  `package-lock.json`, `Cargo.toml`, `Cargo.lock`, README DMG link); shipped `v0.4.1` built
  from `main` with `AgentStatus_0.4.1_aarch64.dmg`.

- **2026-07-28** — **Audio alerts + README de-duplication (decision 035).** (1) Removed the
  status-light **table** under "The lights" in the README — the `lightbar-states.svg` graphic
  already labels all five states with their exact meanings, so the table was pure repetition.
  (2) Added **audio alerts** to the settings panel: an **Audio** On/Off `.seg` toggle reveals
  an inline sub-panel (per-state chime checkboxes for Blocked/Error/Done + a Volume slider),
  reusing the conditional-row disclosure pattern rather than a separate window (which would
  fight the single NSPanel). Chimes are short WebAudio tones — no bundled asset, no CSP concern.
  Edge-triggered off a `prevChimeState` map (fires once on the transition *into* an attention
  state), seeded silently on the first poll so pre-existing blocked sessions don't blast on
  launch. Off by default; frontend-only `localStorage` (`agentstatus.audio`/`.chimes`/`.volume`),
  no hook/schema/backend change. Touches `app/src/index.html`, `styles.css`, `main.js`, README.
  **Left to verify (live):** open the panel, flip Audio on, drive a session to blocked/error/done
  and confirm the chime fires once per transition. **Also:** the `lightbar-settings.svg` art still
  shows the pre-audio panel — regenerate `docs/gen-readme-art.mjs` to add the Audio row if the
  graphic should stay current.
- **2026-07-28** — **Added lightbar visuals to the README (decision 034).** New generator
  `docs/gen-readme-art.mjs` renders five self-contained SVGs from the exact `app/src/styles.css`
  values — `lightbar-hero.svg` (a realistic mixed-state bar), `lightbar-states.svg` (every
  state labeled), `lightbar-hover.svg` (a light with its subagent badge + hover tooltip),
  `lightbar-orientation.svg` (horizontal vs vertical), and `lightbar-settings.svg` (the
  right-click settings panel) — embedded in the README (hero under the tagline; states + hover
  in "The lights"; orientation + settings in a new "Customize it" section). Reproducible art per
  Guideline #8 (`node docs/gen-readme-art.mjs` re-runs it); not manual screenshots. The one
  thing it can't show is the bar over the real desktop — a real screenshot hero could be added
  later if wanted.
- **2026-07-15** — **Released v0.4.0.** Bumped `0.3.0 → 0.4.0` (`tauri.conf.json`,
  `Cargo.toml`, `package.json`, lockfiles, README DMG name), rebuilt
  `AgentStatus_0.4.0_aarch64.dmg`, installed/relaunched `/Applications/AgentStatus.app`, and
  tagged/published `v0.4.0` from `development`. Contents: the Codex lifecycle fix (decision
  032), Antigravity as a fourth host (decision 033 — **unverified against a live install**,
  see item 10), the pill backdrop-filter clipping fix, the Antigravity transcript-read gate
  (~98 ms/turn off every Claude prompt submit), and the latched hover-scale fix.
- **2026-07-15** — **Fixed the hover scale latching after a light click.** Clicking a light
  focuses another app's window, so the pointer leaves the bar without WebKit delivering a
  `mouseleave` — `:hover` stayed latched and the dot sat at `scale(1.18)` indefinitely. A
  `#bar.nohover` class added on click neutralizes the hover transform and is removed on the
  next `mousemove`, when the pointer's real position is known again.
- **2026-07-15** — **Documented Antigravity support (decision 033) and fixed the transcript
  read it added.** The Antigravity host shipped undocumented in `3195f11`; 033 records its
  hook schema (`agentstatus` key in `~/.gemini/config/hooks.json`), event→state mapping,
  payload differences, and pruning/focus behavior, and flags the whole thing as unverified
  against a live install. Fixed: the transcript read was gated on the event name alone, and
  `UserPromptSubmit` is Claude's event — so every Claude prompt submit walked the fallback
  chain into the real Claude transcript and ran `python3` over it (137 ms → 39 ms per turn
  once gated on `ide == antigravity`; ~98 ms saved). The parse was always discarded: it scans
  for `USER_INPUT` records that Claude transcripts don't contain, and jq prefers the payload
  `.prompt` regardless. Smoke-tested both hosts against a temp status dir.

- **2026-07-09** — **Fixed the faint rectangle around the pill on light backgrounds.** The pill's
  `backdrop-filter: blur()` sat on `#bar` alongside `border-radius: 999px`; WebKit does not clip a
  backdrop-filter to the element's (or an ancestor's) rounded corners, so the blurred backdrop
  leaked to the bounding box and showed as a lighter rectangle over non-uniform/light backgrounds.
  `overflow: hidden` on a parent did not clip it either. Moved the frosted pill (fill + border +
  blur) onto a `#pill` layer behind the lights (`z-index: -1`, `pointer-events: none`) and clipped
  the blur with `clip-path: inset(0 round 999px)` — the one thing WebKit does honor — with a
  `.settings-open` override to `15px` to match the panel corners; the drop shadow stays on `#bar`
  so `clip-path` doesn't clip it away. `--bar-opacity` behavior and the badges/hover-scale that
  spill outside the pill are unchanged. Verified in a dev build over a white background: the
  square-cornered halo is gone, leaving only the intended rounded shadow. Ships on next rebuild.
- **2026-07-09** — **Fixed Codex open/close lifecycle (decision 032).** Established (from the
  installed binary + `openai/codex` source) that Codex has no conversation open/close signal at
  all; replaced the dead payload-sniffing heuristics with an explicit `codex` arg from both
  installers, shortened Codex light expiry to 10 min idle (user-approved) with instant drop when
  no `codex` process runs, excluded archived threads from the #031 fallback, and pointed
  click-to-focus at the VS Code window hosting the thread. Rebuilt/reinstalled the app;
  smoke-tested `report.sh` tagging for codex/claude/cursor payloads.
- **2026-07-09** — **Renamed ClaudeStatus → AgentStatus (decision 030).** The product name now
  matches the broader agent scope: Claude Code, Codex, and Cursor. Updated app bundle/product
  names, Tauri identifier/window title, docs, installer paths, extension metadata/command ids,
  localStorage keys, hook backup suffixes, and release asset naming. Kept migration support for
  legacy `CLAUDESTATUS_DIR` / `CLAUDESTATUS_IGNORE`, and the installer removes a prior
  `/Applications/ClaudeStatus.app` while installing `/Applications/AgentStatus.app`.
- **2026-07-09** — **Released v0.3.0.** Promoted the Codex-compatible lightbar build to the
  next public release, then repointed it to the branded AgentStatus build with the live-Codex
  fallback fix: rebuilt `AgentStatus_0.3.0_aarch64.dmg`, installed/relaunched
  `/Applications/AgentStatus.app` locally, and moved the `v0.3.0` tag/release to the fixed commit.
  Headline: AgentStatus tracks Claude Code, Codex, and Cursor sessions from the shared lightbar,
  and active Codex work renders green even when hooks are not yet trusted/loaded.
- **2026-07-09** — **Codex compatibility (decision 029).** AgentStatus now installs the shared
  `report.sh` into Codex user hooks at `~/.codex/hooks.json` as well as Claude's
  `~/.claude/settings.json`. Codex registration uses only the currently documented Codex hook
  events, while Claude/Cursor keep their existing fuller event set. The reporter accepts Codex
  thread/conversation ids, falls back to the hook process cwd, writes `ide:"codex"`, and the app
  skips IDE-lock pruning for Codex sessions. Clicking a Codex light opens `Codex.app`. Verified
  against the official Codex manual (fetched 2026-07-09), `bash -n`, `node --check`,
  `cargo check`, and temp-dir hook smoke tests.
- **2026-07-07** — **Quit button in settings (decision 028).** The accessory app (no Dock icon,
  no app menu) now has an in-UI way to quit: a **Quit** button in the settings-panel footer wired
  to a new `quit_app` Tauri command (`app.exit(0)`), red-tinted on hover. New in
  `app/src-tauri/src/lib.rs` (`quit_app` command + handler), `app/src/index.html` (`#quit-btn`),
  `app/src/main.js` (click → `invoke("quit_app")`), `app/src/styles.css` (shared footer style +
  red hover). `cargo check` clean. **Left:** rebuild + reinstall to exercise it in the packaged app.
- **2026-07-07** — **Stale-light fix: prune on IDE-window close (decision 027).** Lights no
  longer linger up to 2h after a session's IDE window is gone. `list_sessions` now builds the set
  of **live workspace folders** from `~/.claude/ide/*.lock` (skipping locks whose owning `pid` is
  dead — force-quit/crash) and deletes any session whose `cwd` maps to no live folder (empty `cwd`
  = anonymous ghost → matches nothing → pruned), instantly. Purely additive: the 2h idle timer
  (#004) is unchanged and still covers a superseded session sharing a live window's lock. Gated so
  an empty live-lock set (no-IDE machine / bad read) skips lock-pruning entirely. New in
  `app/src-tauri/src/lib.rs` (`pid_alive`, `live_workspace_folders`, `cwd_is_live`) + `libc`
  macOS-target dep. Verified against live state (both empty-`cwd` Cursor ghosts flagged for prune,
  all real sessions kept). **Left:** rebuild + reinstall the app (running copy predates this), then
  confirm by closing a window and watching its light vanish within a poll.
- **2026-07-07** — **Released v0.2.0.** Cut the second GitHub Release (follows the decision-024
  unsigned Apple-Silicon DMG pattern): bumped `0.1.0 → 0.2.0` (`tauri.conf.json`, `Cargo.toml`,
  `package.json`, README DMG name), rebuilt `AgentStatus_0.2.0_aarch64.dmg` via `install.sh`,
  merged `development → main`, tagged `v0.2.0`, and published the release with the DMG. Headline
  features over v0.1.0: **menu-bar mode** (decision 026) and the **sort** toggle (decision 025).
- **2026-07-06** — **Menu-bar mode: floating ↔ macOS menu bar toggle (decision 026).** The bar
  can now run in the **macOS menu bar** as well as floating. A `tray-icon` `NSStatusItem` shows
  the lights as an **image the webview renders** each poll (offscreen `<canvas>` reusing
  `displayState()`/`currentColors()` → RGBA → Rust `set_tray_image`, pushed only when a
  states+colors+condense signature changes), with a **Condense** option that draws one summary dot
  (`error>blocked>done>running>idle`). Clicking the item drops the *same* NSPanel down as a
  **popover** (`toggle_popover`), so per-light tab-focus (#019), hover, and badges work unchanged.
  Toggle is a **Mode** segmented control in the settings panel (`localStorage`
  `agentstatus.mode`/`.menubarcondense`); menu-bar mode **forces horizontal** (a vertical popover
  off the bar looks wrong) and hides the Orientation control. Amends decision 003 (menu bar is now
  an optional mode). Two macOS gotchas found + fixed live: **tray ops must run on the main thread**
  (`run_on_main_thread`; off-main they silently no-op'd, so the panel hid with no tray — plus a
  fallback that never hides the panel when the tray is absent), and the **icon must be forced
  non-template** (`set_icon_as_template(false)` re-asserted per image, else macOS draws it as a
  black alpha-mask silhouette, swallowing the colors). Touches `app/src-tauri/src/lib.rs`,
  `app/src-tauri/Cargo.toml` (`tray-icon` feature), `app/src/main.js`, `index.html`, `styles.css`.
  Shipped via `install.sh` (release, single-instance; auto-restarted). **Known limits:** the menu
  bar auto-hides in full-screen apps (so floating still owns the over-full-screen case — it's a
  per-situation toggle, not a replacement); can't force the item rightmost (macOS reserves the
  right edge; ⌘-drag once to pin it out from under the notch). **Left to verify with the user:**
  colored dots visible/findable in the menu bar, popover opens horizontal, click-to-focus from it.
- **2026-07-06** — **Settings: light sort toggle (decision 025).** Added a **Sort** segmented
  control to the settings panel: **Window** (default — group sessions by their workspace folder,
  sorting by full `cwd` path so subfolders cluster with their root and same-basename windows stay
  distinct) vs **Urgency** (attention states first — `error → blocked → done → running → idle`).
  Answers "sort by what window a session is in"; since hooks expose no true per-window id
  (decision 006), a window is proxied by `cwd` and two windows on the *same* folder merge (user
  accepted this limit). Frontend-only: sorting moved into `tick()`, persisted in `localStorage`
  (`agentstatus.sort`), same pattern as orientation — no hook/schema change. Touches
  `app/src/index.html`, `app/src/main.js`. Rebuilt + reinstalled via `install.sh`. **Left to
  verify (live):** open the panel, toggle Window/Urgency, confirm the lights reorder (and, with
  ≥2 sessions in one folder + others elsewhere, that same-folder lights sit adjacent).
- **2026-07-06** — **First public release v0.1.0 (decision 024).** Cut the first GitHub
  Release. Committed the pending decision-023 work, rebuilt a fresh
  `AgentStatus_0.1.0_aarch64.dmg` (Apple-Silicon-only, unsigned) from the tagged commit,
  tagged `v0.1.0`, and published the Release with the DMG attached. Rewrote
  [README.md](README.md) to lead with the DMG download + accurate macOS-15+/26 Gatekeeper
  steps (`xattr -dr com.apple.quarantine` or "Open Anyway"), keeping `install.sh`
  build-from-source as the Intel/dev path. **Deferred:** code signing + notarization (removes
  the Gatekeeper step); a universal (Intel + ARM) binary; Homebrew cask; marketplace-publish
  the VS Code extension.
- **2026-07-06** — **Settings: bar opacity slider (decision 023).** Added an **Opacity** slider
  (0–100%) to the settings panel. Drives a new `--bar-opacity` CSS variable on `#bar` that fades
  the whole pill together — fill, border, drop-shadow, and backdrop-blur, all scaled via `calc()`
  with multipliers normalized so 82% reproduces the original look. (A first cut varied only the
  fill; barely visible when the bar is minimized to a few lights, since the border/blur dominate
  there — so the chrome now fades too.) Range widened to 0–100 for more travel toward transparent;
  at 0% the pill vanishes and only the lights float. The lights are separate, fully-opaque
  elements, so the signal never fades. Same frontend-only `localStorage`
  (`agentstatus.baropacity`, whole percent) + `applyStyle()` pattern as decision-017; `Reset to
  defaults` restores 82%. Touches `app/src/index.html`, `app/src/styles.css`, `app/src/main.js`.
  Rebuilt + reinstalled via `install.sh` (auto-restart), now live. **Left to verify (live):** drag
  the slider and confirm the whole pill fades smoothly to invisible while the lights stay sharp.
- **2026-07-06** — **Display polish + position persistence + installer auto-restart (decision
  022).** Rebuilt and installed via `install.sh` — now live. (1) **Even padding in vertical mode**:
  `#bar.vertical { padding: var(--bar-pad) }` drops the horizontal-only `+4px` side padding so all
  four sides match (`app/src/styles.css`). (2) **Drag clamp across all monitors + magnetism**:
  `clampToMonitor()` on the window `moved` event bounds the bar to the union bounding box of every
  `availableMonitors()` (slides across shared edges, can't leave the outer edges; center-in-a-gap
  guard pulls it onto the nearest display), with **soft edge magnetism** (`SNAP_LOGICAL = 16`
  logical px, per-monitor scaled) that pins a near edge flush (`app/src/main.js`). (3) **Position
  persistence**: saves the **lights' screen anchor** (`{x,y,scale}`, `agentstatus.pos`) — not the
  window top-left, which depends on settings-panel state — and re-anchors on launch via
  `anchorLightsTo()` over `center: true`, so restarts/rebuilds/reloads no longer move the bar
  (`app/src/main.js`). (First cut saved the window top-left and jumped on Reload because the Reload
  button is inside the panel; fixed.)
  (4) **Reload button** in the settings-panel footer → `window.location.reload()`
  (`app/src/index.html` + CSS). (5) **`install.sh` auto-restart**: if an instance was already
  running, it quits and relaunches the rebuilt app (past the single-instance guard) so rebuilds
  land in one command; first installs still fall through to the manual Gatekeeper-Open steps.
  **Left to verify (live):** position actually restores across the *next* rebuild (nothing was
  saved before this one, so it centered by design); and multi-monitor crossing/magnetism on a real
  multi-display setup.
- **2026-07-06** — **Single-instance guard (decision 020) + faster click-to-focus (decision
  021).** (1) **Fixed two bars running at once** — the installed `/Applications` copy and the
  in-repo dev build were both up, drawing overlapping duplicate bars off the same status dir.
  Root cause: no instance guard. Added `tauri-plugin-single-instance` (release-gated) as the
  first plugin in `run()`; keyed by the shared `com.agentstatus.app` identifier so it catches
  both bundles. **Verified:** from a clean state, launching a second copy (either path) exits
  immediately — 3 rapid launch attempts left exactly one instance. (Observed one transient
  double-instance while rapidly kill/relaunching during the rebuild; it's a narrow stale-socket
  race that self-heals — a dead socket → connection-refused → rebind — confirmed live.) (2)
  **Sped up same-Space window switching** from ~1.15s to ~0.2s: the decision-016 IDE CLI boots a
  Node runtime every click, so `focus_session` now *also* fires a fast `osascript` System Events
  raise (`set frontmost` + `AXRaise` by workspace-root basename) before it. Fast path covers the
  same-Space case; the CLI still fires and covers cross-Space / full-screen. Needs a one-time
  **Accessibility** grant for AgentStatus.app (documented as optional in `install.sh`); without
  it the `osascript` no-ops and the CLI alone runs — no regression. Touched
  `app/src-tauri/Cargo.toml`, `app/src-tauri/src/lib.rs` (`raise_window_fast` + guard),
  `install.sh`. Rebuilt and reinstalled to `/Applications`. **Left to verify:** the focus
  speedup live once the user grants Accessibility (re-copying the bundle likely reset its TCC
  grant).
- **2026-07-06** — **Bar light → focus the exact session tab, not just the window (decision
  019).** A bar-light click now lands on the specific Claude *session tab*, solving the
  multiple-sessions-in-one-folder case that window-raise (decision 016) can't. Hybrid: the bar
  still raises the right window via `code/cursor <root>`, and additionally writes
  `~/.claude/status/focus-request.json` `{session_id, requested_at(ms)}`; the per-window
  extension polls it and calls the popup-free in-editor `claude-vscode.editor.open` to reveal
  that session's panel (advances a per-window watermark so each click fires once; seeded at
  `activate` so a stale request isn't replayed on reload). Rejected the `vscode://…open?session=`
  deep link after re-verifying live that it shows a **consent popup on every click** (the old
  "spawns new agents" note was stale — no new agent spawned — but the popup is real). **Verified
  end-to-end:** clicked a session's light from another VS Code window → the AgentStatus window
  came forward *and* the exact conversation tab was revealed. Touched `lib.rs`
  (`write_focus_request`, `focus_session` gained a `session_id` arg), `main.js` (passes `s.id`),
  `extension/src/extension.ts` (relay). Extension repackaged/reinstalled (`0.1.2`); packaged app
  rebuilt via `install.sh`. No hook or per-session schema change.
- **2026-07-06** — **Cursor support (decision 018).** Verified (Cursor 3.10.11, via a temp
  Cursor logger + Cursor's own hook logs) that Cursor bridges Claude Code hooks and exposes
  clean payloads (`session_id`, `workspace_roots`, `cursor_version`, `subagent_id`, `Stop.status`).
  Taught `report.sh` to handle Cursor payloads and write an `ide` field; wrote
  [hooks/cursor-setup.mjs](hooks/cursor-setup.mjs) to register the bridge-dropped events natively;
  made the bar's click-to-focus IDE-aware (Rust + JS, compiles clean). Unit-tested against real
  captured payloads (VS Code regressions intact). Left the temporary Cursor logger uninstalled.
  **Not yet live-verified end-to-end** (needs a folder-open Cursor run — Cursor runs no hooks in
  a folder-less window) and the app still needs a rebuild + `install.rs` port to self-install the
  Cursor hooks.
- **2026-07-06** — **Settings: size + padding + per-state colors, and keep-on-screen (decision
  017).** Added to the panel: a **size** slider (dot size, 8–24px), a **padding** slider
  (wrapper padding around the lights, 2–20px), **per-state color** pickers (native
  `<input type="color">` for running/blocked/done/idle/error — confirmed working live on the
  NSPanel), and a "Reset to defaults." Refactored the dot geometry, wrapper padding, and state
  colors in `styles.css` to CSS variables on `#bar`, glow derived from the base color via
  `color-mix`; JS sets them from `localStorage` (`agentstatus.dotsize`/`.barpad`/`.colors`),
  same frontend-only pattern as orientation. Also reworked how the panel opens near a
  screen edge so the **lights stay put** and the panel grows toward the screen middle: on
  toggle we anchor the `#lights` screen position, pick the direction (panel above when the bar
  is in the bottom half via `column-reverse`, below in the top half; grows left/right toward
  center via `align-items`), then reposition the window so the lights land back on the anchor —
  they never move on open or close. (Replaced the earlier `keepOnScreen` inward-clamp, which
  moved the lights; its `currentMonitor` call also silently threw — v2 monitor APIs are
  module-level functions, not window methods.) Anchoring runs only on the toggle, so dragging a
  panel-open bar isn't snapped back. **Left to do:** `./install.sh` to update the packaged app
  once confirmed. Frontend only; dev instance compiles + runs clean.
- **2026-07-06** — **Fixed bar click-to-focus opening new windows (decision 016).** Root cause
  (verified live): `open -a "Visual Studio Code" <cwd>` spawns a *new* window when the target
  is a full-screen window on another macOS Space — the app's core use case. Replaced it with
  the IDE's own CLI (`code`/`cursor <root>`), which resolves folder→window internally (Space-
  aware, no duplicate window, no Accessibility permission). Workspace root resolved from
  `~/.claude/ide/*.lock` so a subfolder `cwd` still focuses the right window. Rejected an
  AppleScript window-raise (System Events can't see full-screen windows on inactive Spaces —
  observed) and the `vscode://` deep link (routes through create/resumeSession + consent
  prompt). Rebuilt + reinstalled the app. **User to verify** the cross-Space full-screen focus.
  Still window- not tab-granular (multiple sessions in one folder focus the same window).
- **2026-07-06** — **Settings panel + orientation toggle (decision 015).** Added the first
  settings surface: **right-click the bar** toggles an inline panel below the lights (window
  grows to fit, shrinks back on close; pill radius rounds to 15px while open). First setting is
  **orientation** — a horizontal/vertical segmented toggle that flips `#lights` between a row
  and a column via a `.vertical` class on `#bar`; the existing content-hugging auto-resize
  reshapes the window, so no other geometry changed. Choice persisted in webview `localStorage`
  (`agentstatus.orientation`), app-local like `reviewedAt` — no hook/schema change. Frontend
  only (`index.html`, `styles.css`, `main.js`); dev instance compiles + launches clean.
  **Unverified by hand:** right-click routing on the non-activating NSPanel and the vertical
  render — confirm on the running dev bar, then rebuild the packaged app (`./install.sh`).
- **2026-07-06** — **"Done" vs "idle" light split (decision 014).** Split the single gray idle
  light into **done** (a turn just finished, output not yet reviewed — steady bright-white,
  no pulse) and **idle** (acknowledged — dim gray). Reviewed-tracking is app-local: clicking
  a light acknowledges it (and focuses the session as before), keyed by the finish time so the
  next finished turn re-lights automatically. Discriminates a finished turn from a fresh idle
  via `idle && detail != ""` — no hook or schema change. Unit-tested the lifecycle; rebuilt +
  reinstalled the app. **Also mirrored in the VS Code extension** — status-bar item at full
  brightness for `done`, dimmed for acknowledged idle, click-to-focus acknowledges; recompiled,
  repackaged the `.vsix`, reinstalled (effective on next window reload).
- **2026-07-06** — **Error signal + noise fixes (decision 013).** Red is now `StopFailure`
  only (confirmed live that tool failures produce only `PostToolUseFailure`, never
  `StopFailure`); `PostToolUseFailure` is calibration-logged but no longer flips the light.
  Added the `AGENTSTATUS_IGNORE` env opt-out for programmatic Claude calls (ApplicationBot's
  question-classification sessions were showing as fleeting lights). Propagated the hook to
  all four copies (repo / live / app-embedded / extension-bundled).
- **2026-07-05** — **Milestone 6 complete.** Built the VS Code extension (`extension/`):
  per-window status-bar items reading the status files scoped by workspace, hover detail,
  subagent `×N`, and click-to-focus via `claude-vscode.editor.open` (found by reading Claude
  Code's own URI handler — avoids the consent prompt). Packaged `.vsix`, installed via the
  `code` CLI, verified live. Decision 012.
- **2026-07-05** — **Milestone 5 complete.** Ported the hook installer to Rust
  ([app/src-tauri/src/install.rs](app/src-tauri/src/install.rs), release-gated), embedded
  `report.sh` via `include_str!`, and bundled `AgentStatus.app` + `.dmg`. Installed to
  `/Applications` and verified the app self-installs all 11 hooks on launch (deduped, backed
  up, non-clobbering). Wrote [install.sh](install.sh) + [README.md](README.md). Retired the
  dev server; the packaged app is the running bar now. Decision 011.
- **2026-07-05** — **M4 features:** click-to-focus (window focus via `open -a`, after the
  `vscode://` deep link proved to spawn new agents + a popup); dead-session pruning (2h,
  self-healing) replacing heartbeat-dimming; hover tooltip with task + activity; subagent
  count badge (decision 009 — verified `SubagentStart/Stop` carry `agent_id`/`agent_type`
  under the parent session; subagent tool calls aren't attributable, so lifecycle-only).
- **2026-07-05** — **Milestone 3 complete.** Installed Rust; scaffolded + customized the
  Tauri v2 app into a non-activating NSPanel overlay that floats over full-screen apps
  (decision 008, after ruling out always-on-top / native NSWindow level / accessory-only).
  Fixed an invisible-window bug (auto-resize measured before paint → 0-size). Renders live
  status dots from the M2 status files. Verified floating over full-screen VS Code.
- **2026-07-05** — **Milestone 2 complete.** Built + validated the signal layer: `report.sh`
  (per-session status files, decision 007) and the `setup.mjs` installer. Unit-tested all
  branches, replayed the 160 real M1 events correctly, installed live and confirmed real-time
  status writes. Logged decision 007 (per-session store).
- **2026-07-05** — **Milestone 1 complete.** Verified the event→state mapping on real
  sessions (Claude Code 2.1.201): blocked = `PermissionRequest` (not `Notification`), error
  signal is the soft spot (interim `PostToolUseFailure`), hooks are session-level, windows
  key on workspace folder via `~/.claude/ide/*.lock`. Uninstalled the broad logger; settings
  clean. Full write-up: [DECISIONS.md](DECISIONS.md) #006. Also researched window scoping →
  auto "this window" via the extension (decision 006).
- **2026-07-05** — Milestone 1 kickoff: scaffolded the repo (`hooks/`, `logs/`, `app/`,
  `.gitignore`, git init), built the temporary event logger + idempotent installer.
- **2026-07-05** — Decided the install model (decision 005): global hooks = install once per
  machine, delivered via a self-installing app **and** an optional VS Code extension.
- **2026-07-05** — Chose the architecture: hooks → shared JSON status file → Tauri
  always-on-top window (decisions 001–004). Adapted the project docs (CLAUDE.md,
  DECISIONS.md, NEXT_STEPS.md) from the imported best-practices templates to AgentStatus.
