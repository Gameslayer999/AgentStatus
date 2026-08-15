# NEXT_STEPS.md — Living Build Queue

> Read this at the start of every session to pick up where the last one left off.
> Update it at the end of every session where anything changed (Agent Guideline #10).

---

## Current state

- **⚠ macOS is on unverified code (decision 076).** Both platforms now install the native
  `agentstatus-hook` binary — `report.sh` is no longer installed anywhere, only kept as the
  reference `gen-golden.sh` generates goldens from. The supported floor is **macOS 13**
  (declared, matching Claude Code's own requirement) and the DMG is **universal**, hook
  included. This closes the real macOS-15 bug: `jq` ships at `/usr/bin/jq` only from 15, so
  every DMG user on 13/14 had a hook that no-opped silently forever. **All of it was written
  on the Windows box and none of it has run on a Mac** — see the blocking checklist at the top
  of *Now* before tagging anything.
- **Claude Code lights now have a state reconcile of their own (decision 067).** Until this,
  Cursor had one (#048/#052) and background jobs had one (#063), but a Claude Code light was
  pure hook output — and the hook can only reach `idle` through a `Stop` event, so any turn
  that ends without one (an interrupt, a dropped event) left a green light forever. The app now
  reads `status`/`statusUpdatedAt` from `~/.claude/sessions/<pid>.json` — the same per-pid
  record it already reads every poll for the tooltip name — and greys a green light when Claude
  Code reports the session `idle` in a strictly later second than the hook event behind that
  light. Positive evidence only, so Claude Desktop (which writes no `status`) and any failed
  read keep exactly the old behaviour. **Still unobserved: whether a VS Code session writes
  that record at all**, and what `status: "shell"` means.
- **Four hosts now, not two (decision 054).** Claude Code in **VS Code**, in a **terminal**
  (`ide:"cli"`), and in **Claude Desktop** (`ide:"claude-desktop"`), plus **Cursor**. The two
  new ones needed no signal-layer work at all — all three Claude Code surfaces read the same
  `~/.claude/settings.json`, so the hook was already firing for them; they were invisible
  because everything non-Cursor was tagged `vscode` and decision 027 lock-prunes any `vscode`
  session whose `cwd` matches no live `~/.claude/ide/*.lock`. `report.sh` now tags the host
  from `$CLAUDE_CODE_ENTRYPOINT` and records the owning `claude` pid; those hosts are pruned by
  pid liveness instead. **This also fixed two latent bugs**, both from Desktop sessions being
  tagged `vscode`: they were pruned whenever no VS Code window happened to share their folder,
  and clicking one focused a VS Code window *and* left a stray new Claude tab behind. The stray
  tab turned out to be its own defect, reported live: **the VS Code extension was entirely
  host-blind** (no `ide` field in its `Session`, read zero times), so it claimed any session
  whose `cwd` fell in its workspace — giving foreign sessions a status-bar item in the wrong
  window, opening a stray Claude tab when one was clicked, and letting the decision-019 relay
  reach the same command. **Cursor was exposed to all of it too.** The extension now renders
  only `ide == "vscode"` (**v0.1.3**, installed) and the bar writes the relay only for
  `vscode`. Installed live via `./install.sh`; this session retagged itself to
  `claude-desktop` with a pid `ps` confirms is Claude Desktop's `claude` binary.
  **Follow-up from live use:** a Ghostty CLI light activates Ghostty but cannot select the
  tab — verified against the live app that Ghostty publishes no per-tab identifier that joins
  to a process (working directory is identical across tabs in one project, the title is a
  stale previous prompt, the ids are opaque handles unrelated to the pty). **Superseded by
  decision 055** — on Ghostty **1.3.1** the title is not stale: Claude Code 2.1.231 keeps the
  terminal title bar set to its own session title, and Ghostty 1.3 ships a scripting
  dictionary whose `terminal` surfaces expose that title plus a `focus` command, so a Ghostty
  light now lands on the exact tab *and* split. Also fixed there: **background agents** (`claude --bg`, real
  sessions that Claude names) run detached under `ClaudeCode.app`, so the ancestor walk called
  that their "terminal" and a click would have activated an unrelated app. A detached agent is
  now **opened in a new terminal** via `claude attach` (Claude Code's own verb, safe on a live
  agent), Ghostty when it runs else Terminal.app — verified live: clicking spawned a window
  with `claude attach 2fa90c2f` running on `ttys015`. Two facts had to be found by testing:
  `attach` rejects the full uuid and takes only the short id (uuid up to the first dash), and
  the command must go through a login shell because a GUI-launched app has a minimal PATH.
  That attach instance then broke the *interactive* click ("brought me to an empty Ghostty
  tab"): with two Ghostty instances running, `open -a Ghostty` cannot say which one it means.
  A terminal light now activates the **owning instance** by pid (`System Events`, `unix id`),
  not the app by name — verified by fronting the wrong instance and confirming the click moved
  focus to the one hosting the session's tty. Ghostty's AppleScript write surface (`surface
  configuration`, `initial input`, `input text`) is non-functional in 1.2.x — all three report
  success and start nothing — so `-e` plus a second instance is the only working route.
  **CLI lights are now reconciled against `claude agents --json`** (throttled 10s, only when a
  CLI light exists, fail-open — the #048 pattern). **Note (decision 064):** this query ran
  through a login shell and therefore found nothing whenever the bar was launched from Login
  Items rather than a shell, so everything in this paragraph was inert for real users until
  056 resolved `claude` by absolute path. Two reasons: Claude Code 2.1.231 hosts
  sessions inside pre-warmed `claude bg-spare` processes, so the hook's `$PPID` can be a
  helper with no tty even for an *interactive* session (which made a running Ghostty session
  open a redundant tab instead of focusing its window); and `bg-spare` processes fire
  `SessionStart` and create lights for sessions that never exist, indistinguishable locally
  from real background agents (identical argv, no tty, and they never inherit
  `AGENTSTATUS_IGNORE` because a daemon spawns them). Clicks route on the reported `kind` and
  pid; unlisted CLI lights are dropped after 20s. Verified live: 5 CLI lights → the 3 real
  sessions.
  New re-runnable diagnostic for "clicking does nothing" reports:
  `AGENTSTATUS_TEST_PID=<pid> AGENTSTATUS_TEST_SESSION=<id> cargo test -- --ignored --nocapture
  focus_terminal_live`, which prints the tty and resolved terminal and then performs the click. Headless `claude -p` (`sdk-cli`)
  is deliberately still pruned. **Claude Desktop's ordinary chat threads are out of scope** —
  its bundle contains no hook events and it stores no conversation state locally.
  **Left to verify live:** rebuild via `./install.sh`, then confirm a terminal session lights
  up and that clicking it selects the right Terminal.app tab (needs a one-time Automation
  grant, separate from Accessibility).

- **Cursor lights are reconciled against Cursor's record, vetoed by its tray (decisions 048,
  052).** A silent Cursor light is forced to `idle` when Cursor's stored `status` is terminal —
  but only if Cursor's own tray row doesn't say that composer is `", Running"` right now.
  Without that veto, an agent 107 seconds between hook events (a long file write) was greyed
  mid-turn, and #050's derived done light turned that into a white "unread" on a working agent.
  The veto is positive-evidence-only: no Accessibility grant, or a composer off the tray's
  recents list, means no veto and the pre-052 behavior.

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
  versions. Blocked (orange) is unavailable on Cursor (no event); "done" (white) is now derived
  from the watched running→idle transition (decision 050), and the menu-bar pip (038) covers the
  agents that have no light at all. **Still open:** port `cursor-setup.mjs` into
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
- **A Windows terminal light is now aimed by session title, not by host process alone
  (decision 077).** All of one Windows Terminal instance's windows share a single process, so
  the ancestor walk #071 added identifies the *host*, not the *window*; the title Claude Code
  writes into the title bar is what tells three terminals apart. Untitled sessions and
  sessions in background tabs cannot be placed, and their click is declined rather than
  landing somewhere wrong. Tab selection remains out of reach (#069).
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

**⚠ BLOCKING — verify decision 076 on a Mac before any release.** The macOS-13 floor, the
binary hook on macOS, and the universal DMG were all written on the Windows box. `cargo check`
passes there, which proves only that Windows is unregressed; **every line of the new macOS
path is unrun.** Until this list is green, macOS users are on unverified code, and tagging a
release ships it to all of them. On a Mac, in order:

1. `cd app/src-tauri && cargo check` — the `#[cfg(not(windows))]` `install_binary` has never
   been compiled.
2. `cd hooks/agentstatus-hook && cargo test` — the golden-parity suite, on macOS this time.
3. `./hooks/gen-golden.sh` — confirm it reproduces `tests/fixtures/golden.jsonl` byte for
   byte. A diff means `report.sh` behaves differently on macOS than on the Windows box the
   goldens were captured on, and the port's equivalence claim needs re-reading before
   anything ships.
4. `./install.sh`, then a **live session diffed old-vs-new**: run a turn with the old
   `report.sh` registered, capture `~/.claude/status/sessions/<id>.json` after each event,
   re-register the binary, run the same turn, diff. `pid` and `updated_at` differ by design;
   nothing else may.
5. Confirm the installed `~/.claude/status/agentstatus-hook` is executable and carries **no**
   `com.apple.quarantine` (`xattr -l`) after a DMG install cleared through *"Open Anyway"*
   rather than `xattr -dr` — that path is the reason the install writes bytes instead of
   `fs::copy`ing, and it is the one hazard here with no Windows analogue to reason from.
6. Build the universal DMG once (`AGENTSTATUS_HOOK_UNIVERSAL=1 npm run tauri build --
   --target universal-apple-darwin`) and check three things: `lipo -archs` on
   `Contents/Resources/resources/agentstatus-hook` reports both slices, `LSMinimumSystemVersion`
   in `Info.plist` reads `13.0`, and the **actual DMG filename** matches what the README tells
   users to download (`AgentStatus_0.7.1_universal.dmg` is an expectation, not an observation).
7. `hooks/sign-app.sh` still verifies clean — `codesign --deep` now has a nested Mach-O in
   `Contents/Resources` to sign, which it did not before. A failure here silently costs the
   Accessibility grant (#039), so read its output rather than trusting its exit code.

**Current focus — Windows support, option B (decisions 068/069).** Native Windows only;
Cursor-AX and per-tab terminal focus are out of scope by decision. Sequenced so macOS is
never at risk. (Kept out of the numbered queue below, whose numbering runs on into **Next**.)
   - **a. ✅ Done 2026-08-14 (decision 068)** — the hook is ported to a native binary and
     proven equivalent to `report.sh` across 42 fixtures. Toolchain installed on the Windows
     box (Rust 1.97.1 MSVC; VS Build Tools 2022 were already present).
   - **b. ✅ Done 2026-08-14** — **the app builds and runs on Windows, and the bar renders.**
     Driven end to end: the hook binary wrote four sessions into a status dir, the app read
     them and drew green/orange/red/white in label order with the subagent badge on the
     running light, transparent pill over the desktop, `WS_EX_TOPMOST` confirmed set.
     WebView2 151 was already present. Also fixed here: `status_root`, `claude_session_facts`,
     and `claude_ai_title` all resolved `HOME` only, which is unset for a Windows GUI process —
     the app would have looked for `\.claude\status`. They now share a `home()` helper that
     falls back to `USERPROFILE` (matching `install.rs`).
     Still to check by eye on this box: **drag-to-move**, and the two items under
     Decisions needed below.
   - **c. ✅ Done 2026-08-14** — **the packaged app ships and installs the hook.**
     `npm run tauri build` produces both Windows installers (3.1 MB MSI, 2.1 MB NSIS) with
     `resources/agentstatus-hook.exe` (203 KB) alongside `app.exe`. Verified by extracting the
     MSI and running the packaged app against a **throwaway `HOME`**, three launches in a row:
     11 events registered with exactly one entry each, the binary copied to
     `~/.claude/status/` hash-identical to the bundled copy, a third-party `SessionStart` hook
     and the user's other settings untouched, the backup written, and an injected legacy
     `report.sh` entry **replaced rather than stacked**. The registered command string was
     then run through Git Bash directly and wrote a correct status file — the specific risk
     being a Windows path inside a shell command, which is why it is written with forward
     slashes and quoted.
     The hook moved to its own crate, `hooks/agentstatus-hook/` — `tauri-build` validates
     `bundle.resources` for every target in its package, so building the hook inside the Tauri
     package required the staged binary that the build produces. `hooks/setup.mjs` (the dev
     installer) now registers the binary on Windows too, since `report.sh` there needs a `jq`
     Windows does not ship.
     Note: `stage-hook.mjs` runs on macOS builds as well, so a macOS build now also compiles
     the hook crate. Nothing bundles it there yet (only `tauri.windows.conf.json` lists the
     resource) — it acts as a compile canary ahead of step **g**.
   - **d. ✅ Done 2026-08-14 (decision 070)** — **Windows click-to-focus is wired.** Per-tab
     focus needed nothing: the extension relay is platform-neutral TypeScript. The window
     raise is now a direct `EnumWindows` + `SetForegroundWindow` call — no permission grant,
     no subprocess — with `Code.exe` as a fallback only when no window matched, and Cursor
     raising but never falling back to its CLI (#047). `workspace_root` is un-gated from
     macOS, and path matching moved into `path_within`, which is case-insensitive and
     separator-agnostic on Windows while staying byte-identical on macOS.
     **Carries one unverified assumption — see Decisions needed.**
   - **e. ✅ Done 2026-08-14** — **release pipeline and README cover Windows.**
     `release.yml` is now three jobs: a `version` check that fails in seconds on a mismatched
     tag before anything builds, a `build` matrix (macos-15 → DMG, windows-latest → MSI +
     NSIS), and a single `publish` that assembles one release from the uploaded artifacts.
     Split that way because `gh release create` can only run once, and because a failure on
     one platform should leave *no* release rather than half of one. The rust-cache now
     covers both crates, or the hook crate would rebuild every run. Artifact globs were
     checked against the real local build output, and the workflow YAML was parsed rather
     than eyeballed.
     README: platform badge, split macOS/Windows install sections (SmartScreen instead of
     Gatekeeper, `shell:startup` instead of Login Items), a shared "what the first start
     does" that names the per-platform hook, Windows build-from-source prerequisites, and
     three new entries under Limits (Cursor focuses the window only, terminal sessions cannot
     be focused, WSL unsupported).
     Also corrected a **pre-existing** README inaccuracy while rewriting that paragraph: it
     claimed the app registers hooks in `~/.cursor/hooks.json`, which `install.rs` has never
     done — Cursor picks the hook up through its Claude-compatible bridge reading
     `~/.claude/settings.json` (#018). The native `~/.cursor/hooks.json` entries come from
     `hooks/cursor-setup.mjs`, a dev script.
     **Do not tag a release yet** — see the three unverified items under Decisions needed.
   - **f. ✅ Done 2026-08-14 (decision 072)** — **tray mode works on Windows.** It turned
     out to be mandatory, not optional: the Mode control was not merely inert, it was a
     **trap**. A light click in menu-bar mode calls `hidePopover()`, which on Windows hid the
     window with no tray icon and no taskbar button to bring it back, and the mode is
     persisted — so the app came back in that mode and vanished again on the next click.
     Only recovery was killing the process.
     Built rather than hidden: tray plumbing un-gated, popover opening away from whichever
     screen edge the tray sits on, `set_mode` reporting whether a tray actually exists (the
     frontend reverts to floating if not, so a failed tray can never strand the app), the
     single condensed dot forced on Windows because a notification-area icon is square,
     a `platform()` command for the frontend (which previously had **no** platform detection
     at all), and "Menu bar" relabelled "Tray".
     Verified by driving the real UI end to end: right-click → Tray → the icon appears in the
     notification area (found by its new tooltip) → clicking it reveals the popover →
     switching back restores the floating bar.
   - **g. Switch macOS to the binary hook — ⚠ WRITTEN 2026-08-15 (decision 076), NOT YET
     VERIFIED.** The code is in: `tauri.macos.conf.json` declares the resource,
     `install.rs`'s `#[cfg(not(windows))]` branch copies the binary, `setup.mjs` follows.
     What has not changed is why this was deferred — **none of it has run on a Mac**, and it
     is the one step that touches existing macOS users. See the verification block at the top
     of the queue below; do not tag a release until it is green.

0. **From the 2026-08-13 competitor survey (decisions 056–059).** In priority order:
   - **a. ✅ Superseded 2026-08-14 (decision 068)** — the `jq` guard is no longer needed,
     because `jq` is no longer a dependency. The Windows port forced the question and the
     answer was to remove the dependency rather than detect it: a guard turns a silent failure
     into a loud one but still leaves the user with a non-working app. `report.sh` is replaced
     by a native `agentstatus-hook` binary. Still open from this item: **macOS is still running
     `report.sh`** until the port is diffed against a live macOS session (Windows step **g**).
   - **b. ✅ Done 2026-08-13 (decision 067)** — an interactive session *does* report `status`,
     and the reconcile is built and installed. Still open from this item: **no VS Code session
     has ever been observed**, so whether one writes `~/.claude/sessions/<pid>.json` is
     unconfirmed. Nothing depends on the answer — a host that writes no record reconciles
     nothing and keeps its pre-067 behaviour — but confirm it before claiming VS Code coverage.
     Also unresolved: what produces `status: "shell"`, deliberately excluded as unverified.
   - **c. ✅ Done 2026-08-13 (decision 067)** — the reconcile shipped, from
     `~/.claude/sessions/<pid>.json` rather than `CliFact`: it is already read every poll for
     the tooltip name (#053), costs no subprocess, and is strictly more accurate (`agents --json`
     called a `shell` session `busy` for ten minutes). Guarded by `statusUpdatedAt` outranking
     the light's own timestamp rather than by a silence threshold.
   - **d. Codex re-add** (decision 056), starting with the event logger — verifiable today via
     the installed `openai.chatgpt` VS Code extension. Gemini is blocked (see Decisions needed).

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

### Windows polish found by audit, deliberately not fixed yet

Two agents audited the Windows work on 2026-08-14 — one over the frontend, one over the new
`#[cfg(windows)]` Rust. The defects they found are fixed (decision 072, and the
`focus_host_window` / title-match / `path_within` / `SetForegroundWindow` corrections under
#070–071). These are the remainder: real, none of them a trap, none blocking a release.

- **Edge snapping uses full monitor bounds, not the work area** (`main.js:1053-1067`). On
  Windows the bar can therefore snap flush to the bottom edge and sit *over* the taskbar —
  precisely where the tray and clock are. macOS never showed this because it has no
  persistent bottom chrome. Should snap to the work area instead.
- **The colour swatches open a native modal on Windows.** `<input type="color">` opens the
  Win32 colour dialog, which is OK/Cancel rather than macOS's live `NSColorPanel` — so the
  "previews instantly" behaviour does not happen, and the dialog may open behind an
  always-on-top bar. Untested; needs one look.
- **`oklch()` and `color-mix()` need WebView2 ≥ Chromium 111.** On an older pinned runtime
  the accent colour and the subagent badge background silently fall back to unset. Either
  state a minimum WebView2 version or add plain-sRGB fallbacks.
- **The Cursor attention pip polls a no-op on Windows forever** (`cursor_attention_count`
  returns 0 unconditionally off macOS, polled every 20 ticks). Harmless, but it is work the
  bar does for a feature that cannot exist there (#069).
- **`-apple-system` leads the font stack**, so Windows falls through to Segoe UI. It renders,
  but the panel was metric-tuned for SF at 11px and has never been sized against Segoe.
- **Stale/recycled parent pids remain a theoretical mis-target** for `focus_host_window`.
  The walk now verifies the recorded pid is still `claude.exe` and refuses to climb into
  `explorer.exe`, which removes the reachable cases; a full fix would compare process
  creation times.

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

- **What should the Windows hook record as `pid`? (decision 068 — unresolved, blocks nothing
  yet.)** The shell hook records `$PPID`; the binary records its own parent, which under Git
  Bash may be `bash` rather than `claude.exe`. Which one it actually resolves to has **not been
  observed on Windows**, so the port records `0` rather than guess (Guideline #4). Safe today —
  everything consuming `pid` (`pid_alive`, `owns_terminal`, the #067 reconcile) is macOS-only or
  fails open on a pid it cannot resolve — but it must be measured before any Windows feature
  depends on session liveness. Needs one live interactive Windows session, not a headless run.
- **`StopFailure` has never been observed on Windows.** It is not reachable from a headless
  `claude -p` run, so its fixtures are built from the contract verified on macOS (#006) and are
  marked synthetic. They carry no paths, so platform *should* not matter — but that is
  reasoning, not evidence. Confirm from a live interactive session.
  **`PermissionRequest` is now observed** (decision 078, 2026-08-15): it fired on Windows for
  an `AskUserQuestion` box and twice for a Bash tool approval, holding `blocked` for 43 s / 32 s
  / 6 s, and the bar rendered orange on screen. Still unobserved there: whether it fires **on
  time** for an auto-mode escalation, a file-edit diff approval, or a subagent's own prompt —
  none could be reproduced, and `ExitPlanMode` is the proven case where it does not (#078).
- **VS Code / Cursor window-title formats on Windows are unconfirmed (decision 070).**
  Click-to-focus matches a title that `contains(<project folder>)` and `ends_with("Visual
  Studio Code")` / `"Cursor"` — reasoned from the macOS titles, **not observed**, because
  neither editor is installed on the Windows test box. If the rule is wrong, every VS Code
  click silently falls through to the `Code.exe` fallback (slow but still correct) and every
  Cursor click does nothing (a light that leads nowhere — UI Principle #3). Install one, open
  a project, and check the real title before shipping. The raise mechanism itself is proven:
  `cargo test --lib -- --ignored raises_a_window_by_title` matches a real window in 0.8 s.
- **Does the Windows bar sit in the taskbar? (Needs one look, not code.)** `skipTaskbar: true`
  is set, but it could not be confirmed by inspection: tao implements it on Windows through
  `ITaskbarList::DeleteTab`, which removes the button *without* setting `WS_EX_TOOLWINDOW`, so
  the extended style (`0x40118` — `WS_EX_APPWINDOW` set, `WS_EX_TOOLWINDOW` clear) says nothing
  either way. A speculative `set_skip_taskbar(true)` call changed nothing measurable and was
  reverted rather than left in as cargo-cult. **Glance at the taskbar while the bar is running
  and say which it is.** A glanceable overlay must not occupy a taskbar slot — that is the
  Windows equivalent of the Accessory policy that keeps it out of the Dock (#008).
- **The Windows bar can take focus; the macOS panel cannot.** `WS_EX_NOACTIVATE` is clear, so
  clicking a light activates the bar, where the macOS non-activating NSPanel never does (#008).
  Whether that is actually disruptive in use is untested — a click is usually navigating away
  anyway. Worth one real interaction test before deciding to add `WS_EX_NOACTIVATE`.
- **First-run window position landed at the right screen edge** (x=3326 for a 114px-wide bar on
  a 3440px desktop), not centred as `"center": true` implies. Not chased yet; may be the
  content-resize anchoring (#022) rather than anything Windows-specific, and may reproduce on
  macOS. Check before shipping — a bar that opens half-offscreen reads as broken.
- **The macOS build is unverified since these changes.** `report.sh`'s path fix, the
  `tauri-nspanel` dependency move, and the `install.rs` changes were all written and tested on
  Windows; nothing has been compiled or run on a Mac. Needs a `cargo check` and a live session
  on macOS before the next release.

- **Which product is "Gemini"? (decision 056 — blocks all Gemini work.)** Three different
  products now exist with different config paths and event sets: the **Antigravity IDE**
  (`~/.gemini/config/hooks.json`, what #033 built, installed on this machine), the **Gemini
  CLI** (`~/.gemini/settings.json`, never built, **not installed**), and the **Antigravity CLI
  `agy`** (`.agents/hooks.json`, never built, not installed). Building for one does not cover
  the others. Also needs accepting up front: **neither Gemini nor Antigravity can show orange
  or red** — no permission-request and no failure event — so their lights would be permanently
  green/gray only. Codex is unblocked and can start now (~2–2.5 days); it can reach orange but
  also **not red**.
- **Token/cost tracking: go or no-go? (decision 057.)** Three options costed: defer;
  last-turn cost in the tooltip (~½–1 day); or cumulative cost with a settings-panel readout
  (~2–3 sessions). The deciding facts are that it is **Claude-Code-only by construction**
  (Cursor stores no cost data locally) and that it needs a **hardcoded price table that goes
  stale on every model release**. Options 2 and 3 also require a status-file schema change,
  which needs explicit approval before code.
- **Which high-session-count fallback? (decision 058.)** Five options drafted; recommendation
  is the **overflow chip** (top-K by urgency + one summary chip, bounding the bar at any
  session count), with **folder grouping** as an orthogonal toggle, and auto-shrink rejected.
  Should be a **threshold**, not a mode switch, so per-session lights stay the default.
- **Confirmed event→state mapping** — pending Milestone 1's real-session observations
  (may adjust the doc-sourced names in Current state).
- **Light bar visual design** — ~~orientation (horizontal/vertical)~~ **decided (decision
  015):** now user-toggleable in the settings panel. Remaining: light shape/size, spacing,
  label-on-hover vs always-on.

---

## Recently completed

- **2026-08-15** — **A plan-mode approval turns the light orange (decision 078).** Reported as
  "Windows isn't picking up on orange", with the light **green** while waiting. The first thing
  the measurement did was **exonerate the Windows port**: `AskUserQuestion` and Bash permission
  prompts already held `blocked` for 43 s and 32 s, and a timed screen capture caught the bar
  drawing orange. The defect is `ExitPlanMode` — its `PermissionRequest` fires when the user
  *answers*, so `PreToolUse` is the only event that lands while the prompt is up and the light
  stayed green for the whole 48 s wait, flicking orange for under 150 ms at the end.
  Worth keeping: **the approved fix was falsified by its own reproduction.** The plan was to
  extend #067's reconcile on Claude Code's `status: "waiting"` — already parsed every poll, and
  #067's measured vocabulary always said `waiting` → orange. But status read **`busy`** for the
  entire plan prompt (it reads `waiting` for the other two), so it would not have caught the
  reported case. Dropped rather than shipped alongside; recorded in #078 because the reasoning
  is attractive and will be proposed again.
  Both hook implementations now rewrite `PreToolUse` for `ExitPlanMode`/`EnterPlanMode` to
  `PermissionRequest` — rewriting the event, not adding a state branch, is what keeps `state`
  and `detail` in agreement. `report.sh` changed in lockstep so macOS keeps parity while it
  still ships the shell hook (#076); the regenerated goldens are **+4 lines, 0 changed**, which
  is the proof the `$event`→`$ev` refactor touched nothing else. Verified live on the rebuilt
  binary: **31.9 s** of `blocked` across a real approval, against 48 s of green before, plus a
  screen capture of the orange light — a before/after pair, not a single after-the-fact reading.
  Left unmeasured on purpose: auto-mode escalation, file-edit diff approvals and subagent
  prompts could not be reproduced (`defaultMode: "auto"` allowed every command tried, and a
  project `permissions.ask` rule added mid-session had no effect).

- **2026-08-15** — **A Windows click now lands on the right terminal window (decision 077).**
  Reported live: with three terminals open, clicking a light brought *a* terminal forward, not
  the session's. #071 was measured with one terminal open and assumed one window per host
  process — but Windows Terminal runs every window of an instance in **one**
  `WindowsTerminal.exe`, so all three sessions' ancestor walks converged on one pid owning
  three windows, and the code kept whichever `EnumWindows` handed it first, i.e. z-order.
  The choice is now made by the session title Claude Code writes into the title bar, reusing
  the Ghostty rule (#055/#066) unchanged: ends-with is that session's window, contains must be
  unique. When nothing identifies a window — an untitled session, or one in a background tab
  whose window shows another session's title — the click does nothing rather than raise the
  wrong terminal. `focus_host_window` is now `host_window(...).map(raise)` so the choice is
  testable without `SetForegroundWindow`, which is refused for any process that is not the
  foreground one and therefore can never succeed from a test binary — the first live run
  failed on exactly that with the correct window already chosen underneath. Verified live:
  session `7512a93d` (pid 34064) resolves to `✳ Fix Windows orange input detection`. Tab
  selection is still out of reach (#069). **`target/release/app.exe` is rebuilt — relaunch the
  bar to pick this up.**

- **2026-08-15** — **macOS 13 becomes the floor and the DMG becomes universal (decision 076).
  Written, not verified — see the blocking block at the top of Now.** The app never targeted
  macOS 15: `Info.plist` carried Tauri's default `LSMinimumSystemVersion 10.13`, no code is
  version-gated, and the CSS (`oklch`, `color-mix`) is fine on Ventura's Safari. The 15 floor
  was one accidental dependency — `report.sh` needs a `jq` that ships at `/usr/bin/jq` only
  from macOS 15, so every DMG user on 13/14 installed a hook, registered it, and got silence
  forever. macOS now installs the same native `agentstatus-hook` Windows has run since 0.7.0,
  which is #068 finally reaching its second platform. **13 is the floor because Claude Code
  itself requires macOS 13.0+**, and it is now declared rather than defaulted, so an older Mac
  is refused at install instead of half-working. The DMG went universal because Claude Code
  supports `darwin-x64` and arm64-only excluded every Intel Mac — which forces the *hook* to
  be universal too, the non-obvious part: a shell script ran on any architecture, a compiled
  hook does not, so `stage-hook.mjs` now `lipo`s both slices under
  `AGENTSTATUS_HOOK_UNIVERSAL=1` and asserts both are present before staging. Two macOS-only
  hazards the Windows code gave no reason to expect: `fs::copy` is
  `fcopyfile(COPYFILE_ALL)` and would carry `com.apple.quarantine` onto a binary Claude Code
  runs on every tool call, and writing over the destination fails `ETXTBSY` while a hook is
  executing it — so the install writes fresh bytes to a staged path and `rename`s over.
  `install.sh` lost its `jq` prerequisite; `jq` is now only a dev dependency of
  `gen-golden.sh`.

- **2026-08-15** — **Collapsing the settings panel no longer flickers (decision 075).**
  Reported live, and the asymmetry — only on collapse, never on open — was the clue.
  `panel-above` is `column-reverse`, so the lights sit at the *bottom* of the open panel;
  collapsing mutates the DOM first, snapping them to the top of a window that is still
  ~390px tall, and only three-plus frames later do the async resize and re-anchor put them
  back. Since the DOM change is synchronous and the window calls are async IPC, they cannot
  be made atomic — so the fix suppresses the paint (`visibility: hidden`) across the
  transition instead, restoring once the window is final.
  Two things that stop this trading one bug for a worse one: the rAF wait in
  `resizeToContent` is now bounded (250 ms) and the restore is in a `finally`, because
  animation frames stop firing on a hidden window and a bar left `visibility: hidden` comes
  back **invisible**; and #073's close-debounce now only stamps when the popover is actually
  visible, since a hidden window still reports focus changes (opening the tray's own
  hidden-icons flyout produces one) and that was swallowing the next tray click entirely.
  **Verified with a negative control** — fix disabled: the light jumped 360 px; fix enabled:
  22 px (dot glow / hover scale). Worth keeping that habit: without the control, "no jump"
  only proves the measurement ran, not that it could ever fail.
  Also fixed: `hooks/windows-diagnostics.ps1` now has a UTF-8 BOM. PowerShell 5.1 reads a
  BOM-less script as ANSI, which turns a UTF-8 em dash into mojibake containing a curly
  quote and can break parsing outright. Keep committed `.ps1` files BOM'd or pure ASCII.

- **2026-08-15** — **The Windows tray popover opens with its settings panel (decision 074).**
  Requested live. On Windows the tray item is a single summary dot, so the popover was
  showing a bigger copy of what the user had just clicked while the panel they wanted stayed
  one right-click away. macOS is unchanged (#024) — its menu-bar item already shows every dot.
  Second time in two changes that the obvious signal was the wrong one: `visibilitychange` is
  the natural "the popover appeared" event and the codebase already used it for exactly this
  moment, but **WebView2 keeps the document "visible" while the window is hidden**, so it
  never fires on Windows. Built it that way first, deployed, and got the bare bar. The
  backend now emits `popover-shown`.

- **2026-08-15** — **Two tray-mode defects fixed (decision 073).** Reported from live use of
  #072: the popover stayed on top after clicking elsewhere, and it opened overlapping the
  taskbar.
  The dismissal one had a trap in it. Hiding on `Focused(false)` is the obvious fix, but the
  window is configured `focus: false` and `show()` does not activate it — so the popover
  **never held focus to lose**, and the first attempt deployed and did nothing at all.
  `set_focus()` after `show()` is what makes the event fire; a 400 ms guard then stops the
  focus loss from racing the tray click, which would otherwise leave the icon unable to close
  the popover (a bug indistinguishable from the original).
  The placement one: the popover anchored to the *click point*, and the tray icon lives
  inside the taskbar, so it landed on top of it. `Monitor::size()` is the full screen and
  cannot express this; it now anchors to the work area via `GetMonitorInfoW`. A third defect
  fell out of testing — opening settings grows the window ~360px, which ran off the bottom —
  so `fit_popover` pulls it back inside the work area.
  Also worth recording for whoever tests tray UI next: UIA's `Invoke()` does **not** move the
  cursor, so a tray icon "clicked" that way reports a nonsense position and the popover
  anchors to the wrong place. That looked like a product bug for a while. Use a real mouse
  event when the thing under test depends on the cursor position.

- **2026-08-14** — **A `cli` light on Windows now goes somewhere (decision 071).** Reported
  on the live install: clicking the light did nothing. It was not a bug in the matching rule
  — the only light was `ide:"cli"`, and decision 069 had declared `cli` unreachable on
  Windows, so the click was a no-op *by design*. That reasoning turned out to rest on `pid`
  being recorded as `0`, which was a measurement deferred in #068, not a platform limit. The
  process tree answered it in one look: `claude.exe → powershell.exe → WindowsTerminal.exe`,
  and `claude.exe → claude.exe` for Claude Desktop — two hops in both cases.
  The hook now walks to the nearest `claude.exe` ancestor (its immediate parent is a Git Bash
  shell that exits with it, so recording that would hand the app a dead pid); the app walks
  up from the recorded pid to the first ancestor owning a visible titled window. Verified end
  to end: the live status file went from `pid=0` to `pid=24292` (confirmed `claude.exe`), and
  `focus_host_window_reaches_a_window` resolves a live session and raises its host window.
  Still the window, not the tab — Windows Terminal has no tab API, so #069's ceiling stands.
  **Side effect worth picking up:** `pid` is now a real live handle on Windows, so #064's
  liveness pruning and #067's reconcile finally have something to work with there.
  `pid_alive` still answers "alive" unconditionally off macOS; enabling it is a separate
  change needing its own verification.

- **2026-08-14** — **Installed on the Windows box, and the first live bug fixed.** The
  packaged app installed per-user to `%LOCALAPPDATA%\AgentStatus` with no elevation, wrote
  `~/.claude/status/agentstatus-hook.exe`, registered 11 events, backed up
  `settings.json`, and left every other setting intact. **A session already open picked the
  hook up with no restart** — its light appeared on the next tool call, which confirms the
  claim the README has always made.
  Then, reported within minutes: *"a terminal window keeps popping in and out of my
  desktop"* — and it took **two** fixes, because the first diagnosis was incomplete and the
  report came back unchanged:
  1. The hook binary defaulted to the **console** subsystem, so Windows allocated a console
     for every invocation, twice per tool call. Fixed with
     `#![windows_subsystem = "windows"]` (the hook writes to neither stdout nor stderr by
     contract, and piped stdin is unaffected — verified through Git Bash).
     `stage-hook.mjs` now reads the PE header and refuses to stage a binary that is not
     subsystem 2, since no test would catch this.
  2. **Still flashing.** Recording every process creation for 25 s — rather than reasoning
     about it again — showed the real steady source: the bar itself runs
     `claude agents --json` every 10 s (`CLI_FACTS_TTL`), and a GUI process owns no console,
     so Windows allocated a **new** one each time. Measured as `app` → `claude.exe` →
     `conhost.exe` on a 10-second cadence, independent of anything the user was doing. Fixed
     with `CREATE_NO_WINDOW` via a `no_window()` helper.
  Note for anyone measuring this again: **counting `conhost.exe` processes proves nothing**.
  `CREATE_NO_WINDOW` still creates a console device, so conhost still appears — only
  enumerating *visible* top-level windows distinguishes a console that was allocated from one
  that blinked on screen.
  Also found while measuring: `cursor_running()` shelled out to `pgrep` **once per second**
  on Windows purely to fail, and is now macOS-gated with the same fail-open answer.

- **2026-08-14** — **The hook is a native binary, and `jq` is gone (decision 068);
  Windows support started (decision 069).** Investigating "can we support Windows" turned up
  that Claude Code runs hooks through **Git Bash** on Windows (verified by probe, not assumed:
  `SHELL=…\Git\bin\bash.exe`, `$HOME` expanded, `%USERPROFILE%` did not, and a shebang `.sh`
  ran directly with its stdin payload). So `report.sh` *runs* there — it just costs **210.6 ms
  per event** against the new binary's **26.4 ms** (bash-spawn floor 18.6 ms), i.e. ~421 ms vs
  ~53 ms of added latency on every tool call, which Agent Guideline #3 forbids. That, plus
  `jq` being absent on Windows and pre-macOS-15, made removing the dependency the answer rather
  than guarding it (which retires the queue's old top item).
  - `src/hook.rs` + `src/bin/agentstatus-hook.rs` (358 KB release binary, links no Tauri).
  - Equivalence is **proven, not claimed**: `hooks/gen-golden.sh` replays 42 fixtures through
    the current `report.sh`; the port must match strictly, with no per-field exceptions. Run
    on disk as well as in memory, both produce identical status files, subagent markers, and
    `calibration.log`. 13 tests, all passing.
  - Fixtures: 14 events captured from a **real Windows session** (`SessionStart` … `SessionEnd`,
    including `SubagentStart`/`SubagentStop` — which #006 recorded as never firing on 2.1.201,
    so that contract has changed) plus 28 synthetic ones. Sanitised before commit per Guideline
    #12 — home path, session ids, agent ids and transcript paths replaced, payload shape kept.
  - **Two real bugs found and fixed in `report.sh` as well**, both from Windows sending
    backslash `cwd`s: every Windows light was labeled with its *entire path*, and file details
    read `Read C:\…\sample.txt` instead of `Read sample.txt`. The fix normalises `\` only for
    drive-letter/UNC paths, so macOS behaviour is provably untouched.
  - The crate also **compiles on Windows** now: `tauri-nspanel` is macOS-only, `pid_alive` has
    a fail-open fallback, `install.rs` resolves `USERPROFILE` and no longer `chmod`s.
  - Windows box set up: Rust 1.97.1 MSVC (VS Build Tools 2022 already present), `jq` installed
    **dev-only** for generating goldens from `report.sh`.

- **2026-08-13** — **An interrupted turn greys its own light (decision 067).** Reported live:
  "when I stop a session manually, the light still shows up as green." Structural: `report.sh`
  maps hook *events* and reaches `idle` only via a `Stop`, which an interrupted turn never
  fires — so the light kept the `running` its last tool event wrote, indefinitely. This is
  decision 059's proposed reconcile, built after its live check finally landed: sampling
  `~/.claude/sessions/<pid>.json` against the lights every 2 s caught a real Ctrl+C
  (`status` → `idle` **3.84 s after** the light's last hook event, light still green) and
  mapped the vocabulary — `busy`→green, `waiting`→orange, `idle`→gray, and **absent on every
  Claude Desktop session**. `claude_session_names` → `claude_session_facts` now carries
  `status`/`statusUpdatedAt` (TTL 5 s → 1 s, since a status changes every turn where a name
  never does), and a green light greys when Claude Code says `idle` in a **strictly later
  second** than the hook event behind the light — so the hook always wins a tie and a starting
  turn is never grey for a poll. It also **clears `detail`**, or #014/#050 would have rendered
  the result as the white "finished, unread" light on a session the user is already looking at.
  Background jobs excluded (their `idle` means something else — #063/#065); `shell` left alone
  as unconfirmed. No hook change, no schema change. New unit test pinned to the real measured
  timestamps, new `dump_turn_reconcile` diagnostic for the next stuck-green report, built and
  installed live via `./install.sh`. **Verified live** on the installed build and confirmed gray
  on screen by the user: session `e7041031` went `hook=running`/`claude=busy` → no `Stop` ever
  arrived → Claude Code said `idle` **1.5 s** later and the light greyed on the next poll. That
  is the tight case, barely over the strictly-later-second threshold, so the guard held without
  swallowing the fix.
- **2026-08-13** — **A background agent that waits for you keeps its light, and one that works
  is green (decision 063).** Traced from "which session is `agentstatus-6e`": a `--bg` job that
  stops to ask a question fires `Stop`, so the hook writes `idle`, so the 5-minute retirement
  guard ("never a `blocked` light") never matched — measured live, `74dbc1ee` sat waiting on an
  answer with **no light at all for seven minutes**, which also removed the click that opens it
  in a Ghostty tab. `CliFact` now also reads Claude Code's `state` (`working`/`done`/`blocked`),
  which the JSON already carried: `status: busy` → green whatever the last hook said,
  `state: blocked` → orange and never retired on a timer, `done` and stale `working` unchanged
  and still retired at five minutes. Only a light whose hook last wrote `idle` is reconciled, so
  orange still means exactly one thing — this session waits on your decision. Verified by a new
  unit test over all four (`status`, `state`) pairs plus the interactive cases, and live against
  the seven open sessions via `dump_cli_facts`, which now prints the resolved light.

- **2026-08-13** — **Why a Ghostty light sometimes did nothing, and the half of it that is
  fixable (decision 066).** Measured instead of guessed: every CLI light driven through
  `focus_session` with a decoy focus set on an unrelated surface first, so a dead click showed
  as one. The pattern was exact — a click lands when decision 055's title match finds its
  surface and does nothing when it declines, because the fallback is `activate Ghostty`, which
  changes nothing on screen when Ghostty is already frontmost. **Fixed:** 055 matched with
  `contains` *and* demanded uniqueness, so an unrelated surface whose title merely spanned the
  session title counted as a second hit and killed the click. Matching now has two grades —
  **strong** (title *ends with* the session title, which is how Claude Code writes it, `◑
  <title>`; several strong hits are several views of the same session, so the first is right by
  construction) and **weak** (merely contains, so still must be unambiguous). Reproduced with a
  bystander surface: `contains-hits=2` → declined; `ends-with-hits=1` → lands, confirmed twice
  through the real click path plus a no-regression sweep. **Not fixable:** a terminal used only
  to start a background agent has no `ai-title` of its own and is titled with *that agent's*
  title. Every other key was checked and rejected — `working directory` identical across all
  five surfaces, `parkedJobId` stale (named a finished job while the tab showed another),
  elimination actively wrong (such a session shares its tab with the agent it displays), and
  Ghostty 1.3.1 exposes no tty or pid per surface in the dictionary or the environment. The
  README now states this plainly instead of leaving it as a surprise. Two earlier readings of
  the results were thrown out and re-measured: one probe read `window 1` while Ghostty had two
  windows, and one used the session's own surface as the decoy so a correct focus registered as
  "no change".

- **2026-08-13** — **A light keeps the slot it arrived in (decision 062).** Reported live: the
  lights kept swapping places. `sortSessions()` re-ordered the whole strip every 1 s poll by
  `cwd` then session id — a random uuid, so a new session in a folder that already had lights
  landed in an arbitrary slot *between* them and pushed the rest along, and a session that
  `cd`'d changed groups mid-life. #064/#065 made it constant rather than occasional (a
  background agent per `/stop`, plus pre-warmed spares), so a light moved out from under the
  pointer between seeing it and clicking it. Ordering is now **arrival**: a session takes the
  next free slot the first time the bar sees it and holds it until it is gone, when the gap
  closes; the first poll after a launch uses the backend's deterministic (label, id) order.
  The settings segment is **Stable | Urgency** — folder grouping dropped (it never separated
  several sessions in one repo, and the folder is in the tooltip), and a stored `window`
  preference reads as stable. Arrival could not be taken from the status file: `report.sh`
  writes atomically (temp + `mv -f`), so the file's birth time resets on every event — checked,
  not assumed. Frontend-only: no hook, schema, or Rust change. New `node app/tests/light-order.mjs`
  covers 7 cases on the shipped functions. **Left to confirm live:** the reported behaviour, by
  watching the bar through a few sessions (no Screen Recording grant here to check it directly).

- **2026-08-13** — **A click on a background agent reaches it where it already is (decision
  061).** Reported live: "the ghostty light kept opening a new tab." A `kind: background`
  session has no terminal to focus, so decision 054 routes its click to `claude attach`, and
  nothing ever asked whether that agent was already open somewhere — decision 064 then made it
  far more visible by turning "a second Ghostty instance" into "a tab in the window you are
  looking at". Reproduced through `focus_session` itself: **2 → 3 → 4** surfaces, one new
  terminal per click. Decision 055's matcher could not be reused as-is, because an attached
  agent's tab carries the same session title as every other view of it — three surfaces matched
  and 055's exactly-one rule would have declined and attached again. So
  `focus_ghostty_surface` gains a `require_unique` flag: **true** for an interactive light
  (the alternative is fronting the app, so a wrong tab is worse than no tab) and **false** for a
  background one (the alternative is creating another terminal, so any surface already showing
  the session wins). `attach_background_agent` now tries focus-existing → front Ghostty if
  `pgrep` finds an attach already running for that id → open a tab, in that order. Verified with
  a decoy focus set before each click so a no-op would show: three clicks, surfaces held at 4,
  each landing on a surface showing the session. Also recorded in 061: a **false verification**
  that nearly shipped — the first check piped a non-compiling test binary to `/dev/null`, so the
  clicks never ran and the unchanged count read as a pass; the check now asserts the test
  actually ran before believing a count.

- **2026-08-13** — **The tooltip names the application a session runs in (decision 060).**
  Requested: a light should say which app its session is in ("ex: vscode, ghostty"). The head
  line was `folder · session name — state` (#053) — which project, which session, never where,
  though a light has meant four different applications since #054. `list_sessions` now returns
  an `app` field and the head reads `AgentStatus · agentstatus-5b (Ghostty) — running`. The IDE
  hosts are the `ide` field spelled as the app is named on screen; a `cli` session is resolved
  to its **emulator** by walking the owning `claude` pid up to the first app bundle
  (`terminal_app_of`, already written for click-to-focus), memoized per session because the
  tooltip is rebuilt every 1 s poll. A detached background agent reads `background agent`, never
  the `ClaudeCode` launcher the walk would find — interactive-vs-background is decided exactly
  as `focus_session` routes a click, so the tooltip and the click agree (UI Principle #4).
  App-side only: no hook change, no status-file schema change, no new permission. Verified with
  a new `cargo test -- --ignored --nocapture dump_host_apps` against the four live sessions
  (two Ghostty, one Claude Desktop, one background agent, all correct) plus two new cases in
  `node app/tests/tooltip-head.mjs`. **Left to verify live:** the `VS Code` and `Cursor` strings
  on a real session of each (neither was open), and the tooltip on the packaged app after a
  rebuild via `./install.sh`.

- **2026-08-13** — **A finished background agent's light now retires (decision 065).** Reported
  live: lights lingering for sessions with "no currently active tab or terminal anywhere". Put
  every light on the bar next to its evidence and the stale ones were exactly the **background**
  jobs: a `--bg` job has no terminal by construction, and Claude Code keeps its process alive
  and still listed in `claude agents --json` after the work is done — so decision 054's rule (e)
  (owning pid died) never fired and 056's rule (f) (Claude Code does not list it) never applied,
  leaving them until the two-hour `MAX_IDLE_SECS` backstop. The interactive half already worked
  and was confirmed rather than assumed: a session the user had typed `exit` in had its light
  gone within seconds, with the shell still sitting on `ttys005` and no `claude` process on it.
  New prune rule (g): retire a `cli` light once Claude Code reports it `kind: background` +
  `status: idle` (a new `status` field on `CliFact`) **and** it has been hook-silent for
  `CLI_BG_DONE_SECS` = 5 min. A clock rather than a category, which is what reconciles "just
  finished and idle can stay" with "abandoned should disappear" — they are the same light at
  different ages. Never applied to a `blocked` or `error` light: those are the two states the
  user must act on and both go quiet by nature, so a silence timer points at exactly the wrong
  ones (UI Principle #2). Verified live on the packaged app: both background ghosts gone within
  one poll, while an interactive session idle for 38 minutes in an open tab, the Claude Desktop
  light, and the busy background job doing this work all survived.

- **2026-08-13** — **Phantom-spare light fixed, and the CLI reconciliation it depends on made
  to actually run (decision 064).** Reported live: "a new light appeared on my bar. it opens a
  new ghostty window with copies of my claude sessions." It was a **pre-warmed spare** —
  `1550acaa`, whose recorded pid `4592` was a `claude bg-spare` process absent from
  `claude agents --json` — clicked inside decision 054's 20s grace. The click fell through to
  `attach_background_agent`, which ran `open -na Ghostty` (a *second instance* of the app) on
  `claude attach 1550acaa`; `attach` on an id with no live job does not fail, it lands in
  Claude Code's **agent view**, i.e. a window listing every session. The daemon log dates the
  whole sequence, including the agent view claiming a spare of its own 14s later. **Underneath
  it sat a latent defect that made half the fix pointless:** `zsh -lc` is non-interactive, so
  it never reads `.zshrc` — which is where `~/.local/bin` is set — meaning 054's
  `claude agents --json` returned nothing whenever the bar was launched from **Login Items**
  instead of a shell, silently disabling *all* CLI reconciliation (it is fail-open). It only
  ever worked in development because `./install.sh` relaunches the app from a shell that
  already has the PATH. Four changes, all app-side, no hook or schema change: `claude_bin()`
  resolves the binary by absolute path and it is run with no shell at all; a click on a CLI
  light Claude Code does not list does nothing; an unlisted light whose pid owns no terminal is
  dropped on sight (a pid *with* a tty, and a pre-054 file with no pid, keep the full 20s); and
  a background agent now opens in a **tab of the running Ghostty** via 1.3's scripting
  dictionary, which also removes decision 055's two-instances limitation at the source.
  Verified end-to-end on the packaged app relaunched with `env -i PATH=/usr/bin:/bin`
  (`ps -E` confirming the bare PATH): a synthetic unlisted CLI light with `pid: 1` was pruned
  **within 1s**, exercising binary → query → rule in one observation, while the negative
  control (an unlisted light whose pid owns a tty) survived 8s. Also: the extended
  `cli_liveness_pruning` test caught the first cut pruning pre-054 files with no `pid`, fixed
  by requiring `pid > 0`. Two things confirmed *not* to be bugs: the other new lights are real
  background sessions (Claude Code spawns one per `/stop`), and `claude attach` legitimately
  wakes a settled agent.

- **2026-08-13** — **Competitor survey, and four decisions from it (056–059).** Researched what
  else exists in this space and compared it against AgentStatus. Findings that changed the
  build queue:
  **(1) The one-light-per-session bar is genuinely unmatched** — `claude-status-bar` and
  `gmr/claude-status` both collapse every session into a single aggregate menu-bar icon with a
  dropdown, so aggregating by default would trade away the differentiator. But the bar grows at
  `8 + 23N` px (confirmed against #051's measured `37 × 123` for five lights), so 30 sessions is
  a 698 px bar. Five fallback designs drafted (decision 058); recommendation is a threshold-based
  overflow chip, not a mode switch.
  **(2) Cost/usage analytics is the most common missing feature** — `AgentsView` (60+ agents,
  SQLite, spend charts), `usage` (burn rate, quota), `claude-statistics`. Scoped it (decision
  057): the data exists in Claude Code transcripts (`message.usage`/`message.model`, verified on
  a real 3.7 MB transcript) but **not in hook payloads** (all 246 captured events checked) and
  **not in Cursor at all** — Cursor stores context-window fullness, `usageData` is empty `{}` on
  every row. So it is Claude-only by construction and needs a hardcoded price table.
  **(3) Competitors run redundant signal channels** — `gmr/claude-status` runs three
  (Darwin push + filesystem watch + 5s poll); `so-agentbar` and `marmonitor` skip hooks entirely.
  Assessed the hook-only risk (decision 059) and found the project has **already built a second
  channel for Cursor** (#048/#052) and existence checks for CLI/Desktop (#054) — but the **core
  Claude Code path has no state reconcile at all**, and `claude agents --json` /
  `~/.claude/sessions/<pid>.json` both carry a `status` field the app already reads and throws
  away (`CliFact`, `lib.rs:916-919`). Also surfaced a concrete shipping gap: **`install.rs` never
  checks for `jq`** though `install.sh:11` does, and `jq` only ships with macOS 15+, so a DMG user
  on an older macOS gets a silently inert app.
  **(4) Codex and Gemini were already built and removed** (#040, as unverified), so the re-add is
  scoped as observe-then-build (decision 056), and it must first gate the #040 cleanup
  (`install.rs:80-121`, `setup.mjs:73-82`) that would delete new entries on the next launch.
  Verified locally: Codex's VS Code extension is installed (so **Codex is verifiable today**),
  while the Gemini CLI and Antigravity CLI are **not installed** — and "Gemini" now means three
  different products. Neither Gemini nor Antigravity can show orange or red.
  Research only — **no code changed**; all four are logged as `Proposed`, awaiting the user.

- **2026-08-13** — **A Ghostty light focuses the exact tab and split (decision 055).** Reported
  from live use: clicking a Ghostty CLI light opened Ghostty but left the previously active tab
  in front. Decision 054's finding — Ghostty publishes no per-tab identifier — was re-checked
  and had gone out of date twice over. Ghostty **1.3.1** ships `Ghostty.sdef`, a full scripting
  dictionary (`window` → `tab` → `terminal` surface, each with a title, and a `focus` command
  that selects the surface and fronts its window), and Claude Code **2.1.231** writes a session
  title into the terminal title bar, persisted as an `ai-title` record in the transcript —
  which decision 053 had checked for on 2.1.223 and correctly found absent. The two join on
  that title: `claude_ai_title()` reads the last `ai-title` for the session (transcript found
  by globbing `~/.claude/projects/*/<id>.jsonl`, files past 16 MB skipped), and
  `focus_ghostty_surface()` runs one `osascript` that focuses the surface whose title
  **contains** it — `contains` because Ghostty shows the title with the activity spinner glyph
  prepended (`◑ Fix Ghostty tab focus…`). It acts only on an **exactly one** match; no
  `ai-title` yet, no hit, several hits, Ghostty older than 1.3, or a refused Automation grant
  all fall back to fronting the app, which is exactly what every Ghostty click did before, so
  the worst case is never a wrong tab (UI Principle #4). Rejected on the way: matching by
  working directory (identical for two agents in one repo — the reported case), recording a
  surface id in the hook (Ghostty exports none into the session environment, and querying it
  from a hook would put an `osascript` spawn on the user's turn), and asking Claude Code
  (`claude agents --json` and `~/.claude/sessions/<pid>.json` carry no tty or title). Reading
  the transcript is a **flagged exception to Guideline #5**: only the title is extracted, lines
  are substring-filtered before any JSON parsing, and nothing is stored. No hook change, no
  status-file schema change. Verified: the shipped AppleScript run standalone against two live
  splits with focus read back in both directions (fabricated title → `no`; each real title →
  `ok` and the right split focused); a new `focus_ghostty_live` test (untitled session →
  `title = None / focused = false`; titled → `focused = true`); `focus_terminal_live` extended
  with the session id, resolving `tty=/dev/ttys001`, `terminal app=("Ghostty", 32265)` and the
  right title; `cargo build --release` clean with no new warnings; `cargo test --release`
  2 passed, 9 ignored. **Verified live on the packaged app**, two sessions as two splits of
  one Ghostty tab, with a background recorder sampling the focused surface twice a second so
  the result could not be corrupted by whatever was clicked afterwards: with split
  `6F2668F6` set as a decoy and Ghostty in the background, the `agentstatus-e0` light fronted
  Ghostty **and** moved focus to `EB362C00`, and the `agentstatus-3c` light — clicked while
  Ghostty was *already* frontmost — moved focus back to `6F2668F6`. That second transition
  can only come from the AppleScript `focus` (fronting an app cannot change which split is
  focused inside it), which also settles the Automation grant for AgentStatus.app → Ghostty
  without reading the protected TCC database: a refused grant falls through to
  activate-by-pid, which would have left the focused split untouched.
- **2026-08-13** — **Terminal CLI and Claude Desktop became first-class hosts (decision 054).**
  Investigated first (Guideline #4): an isolated `--settings` probe showed the **terminal CLI
  fires the full lifecycle with payload keys byte-identical to the VS Code extension**, and
  Claude Code **inside Claude Desktop was already writing a status file** during the
  investigation. So the signal layer needed nothing — all three surfaces read the same
  settings file. The reason neither appeared was display-layer: `report.sh` tagged everything
  without `.cursor_version` as `vscode`, and decision 027 deletes a `vscode` session whose
  `cwd` matches no live IDE lock, which a terminal never writes. Changes: `report.sh` tags the
  host from `$CLAUDE_CODE_ENTRYPOINT` (`cli` / `claude-desktop`; `sdk-cli` and every unknown
  value fall through to the old default, so VS Code and Cursor cannot regress) and records the
  owning `claude` pid as a new optional `pid` field; `lib.rs` prunes those hosts by
  `pid_alive()` instead of IDE locks, and routes their clicks — a terminal session selects its
  exact Terminal.app tab via the tty, any other emulator gets app-level focus, and a Desktop
  session activates Claude. **Claude Desktop's ordinary chat threads were ruled out**: zero
  hook event names in its 38 MB `app.asar` and no local conversation state, so any light would
  have to be inferred and would lie (UI Principle #4). Verified: 8 hook unit cases incl. Cursor
  and VS Code regressions; a real pty-driven interactive `claude` writing `ide:"cli"` with a
  pid that `ps` confirms is the claude process; `cargo check` clean debug **and** release; a new
  `cli_liveness_pruning` test (live pid survives, dead pid pruned, pre-054 file without a `pid`
  not pruned); and the Terminal.app tab lookup checked live with correct positive, negative,
  and full select-and-raise. Also refreshed the gitignored, generated `extension/report.sh`,
  which was a stale pre-#040 artifact locally (`vscode:prepublish` regenerates it, so nothing
  stale could have shipped). **Left to verify live:** rebuild via `./install.sh`, a real
  terminal light on the running bar, and the packaged app's Automation grant for tab focus.

- **2026-08-12** — **Released v0.6.4.** Version bumped 0.6.3 → 0.6.4 across
  `tauri.conf.json`, `package.json`, `package-lock.json`, `Cargo.toml`/`Cargo.lock`, and the
  README's download link and release-command example. Contents: the Cursor tray-row veto
  (decision 052) and the session-name tooltip (decision 053). The README was also rewritten to
  describe what the tool does — the prose no longer explains the problems behind each
  behaviour, and `Notes & limits` became a four-bullet `Limits` section covering only the
  standing constraints (macOS/Apple Silicon, unsigned builds, no Cursor blocked light, the
  Cursor Accessibility requirement).

- **2026-08-12** — **The tooltip names the session, not just the folder (decision 053).** Its
  first line was the project folder alone, so two sessions in one folder had identical
  tooltips — and a session that had `cd`'d into a subfolder read `src-tauri`, not
  `AgentStatus`. It now reads `folder · session name` (`AgentStatus · agentstatus-5b`), taken
  from the host's own record: Claude Code names every session in `~/.claude/sessions/<pid>.json`
  (`sessionId` → `name`, read behind a 5s TTL cache), and Cursor's composer name was already
  being queried by #048. App-side only — no hook change, no status-file change, no transcript
  read. There is **no LLM-written session title to use**: zero `summary`/`title` entries exist
  in any transcript under `~/.claude/projects/` on Claude Code 2.1.200. The name is dropped
  when it repeats the folder and stands alone when there is no folder. Verified with
  `cargo test -- --ignored --nocapture dump_session_names` (all 5 live sessions resolved) and
  `node app/tests/tooltip-head.mjs`.

- **2026-08-12** — **A working Cursor agent can no longer be greyed — or flagged unread
  (decision 052).** Reported live: a Cursor agent that was *running right now* showed up as
  white/unread. Traced to three things in sequence, from that composer's own timestamps:
  Cursor flushed `status="aborted"` at 13:45:30 (the **previous** turn), the live turn's last
  hook event was 13:48:09 (`running`, `detail="Write app.js"`), and the next one didn't arrive
  until 13:49:56 — a **107-second** mid-turn gap. #048's reconcile only requires 60s of hook
  silence plus a terminal `status`, so at 13:49:09 it forced the light to `idle`; #050 derives
  Cursor's done light from a watched non-idle→idle transition, so that forced idle rendered as
  **white "finished, unread"** on an agent still editing a file. (It also swallowed the real
  unread light: the genuine `stop` 47s later was an idle→idle no-op, so the finished turn ended
  up dim gray.) Fix: a fourth condition on the reconcile — Cursor's tray row for that composer
  must not read `"<name>, Running"` (#049's suffix), the only live status signal Cursor exposes;
  everything in `state.vscdb` describes the last *flushed* turn. `cursor_tray_titles()` reuses
  #045/#047's AX walk with a record-and-decline predicate (nothing pressed, no menu opened),
  cached at the same 5s TTL and read lazily; the composer name is one extra column on #048's
  existing query, so no second `sqlite3` spawn. **Positive evidence only** — an empty tray read
  (no Accessibility grant, composer off the tray's recents) never causes a veto, so it can only
  keep a light green, never light one up. Rust-only; no hook, schema, or frontend change.
  New `tray_running_veto` unit test; the live check rides on the existing
  `cargo test --release -- --ignored --nocapture cursor_facts`, which now prints each Cursor
  session's composer name and `tray_says_running` verdict. Rebuilt + reinstalled via
  `./install.sh`. README updated. **Verified live** on the reported composer itself, sampled
  every 3s through a real turn: `terminal=true` and `tray_says_running=true` held together for
  71 consecutive samples (exactly the pair that used to grey the light), and the suffix cleared
  when the turn ended (105 samples `false`), so the veto doesn't latch and #048's stuck-green
  fix still fires. The full 242-sample run also pinned the release timing: the tray drops
  `", Running"` ~3s *before* the `stop` hook arrives, so the veto lifts just ahead of the turn
  being confirmed over and can never delay a genuine reconcile. Not reproduced in that turn: the
  60s clock itself (its longest hook gap was 17s) — unchanged #048 code, but worth catching on a
  longer turn.

- **2026-08-12** — **Released v0.6.3.** Version bumped 0.6.2 → 0.6.3 across
  `tauri.conf.json`, `package.json`, `package-lock.json`, `Cargo.toml`/`Cargo.lock`, and the
  README's download link and release-command example. Patch: one fix, the clipped-pill resize
  trap (#051), which shipped in v0.6.2 and every version before it. Cut by merging
  `development` into `main` and pushing the `v0.6.3` tag (decision 041's workflow).

- **2026-08-12** — **Fixed the bar rendering clipped after one light (decision 051).** Reported
  right after the v0.6.2 relaunch: the pill was drawn with a rounded top and a flat, cut-off
  bottom. The window was `37 × 31` points — one light's worth — while its DOM held five lights.
  Not a v0.6.2 regression: `resizeToContent()` is edge-triggered (a light added/removed, a
  geometry setting changed) and awaits two animation frames before measuring, and the webview
  delivers none while the window isn't painting, so a resize landing during launch or
  `install.sh`'s relaunch is never applied; the measurements that did stick were taken before
  the first poll, when the bar held only the "empty" placeholder. Fix: `ensureSized()` runs
  after each poll's render, measures the content synchronously (one layout read, no animation
  frames) and re-resizes when it disagrees with the last size actually applied — a level check
  behind the existing edges. Verified live: the bar now relaunches at `37 × 123` for the five
  current sessions, screenshot-confirmed whole. Diagnosis was empirical — dropping one synthetic
  session file (an add edge) instantly resized the stuck window, proving the resize path worked
  and only its trigger had been lost.

- **2026-08-12** — **Released v0.6.2.** Version bumped 0.6.1 → 0.6.2 across
  `tauri.conf.json`, `package.json`, `package-lock.json`, `Cargo.toml`/`Cargo.lock`, and the
  README's download link and release-command example. Patch: both changes since v0.6.1 are
  Cursor fixes — the tray-row prefix match that made click-to-conversation actually work
  (#049) and the derived Cursor done light (#050). Cut by merging `development` into `main`
  and pushing the `v0.6.2` tag, which triggers the release workflow of decision 041.

- **2026-08-12** — **Cursor lights get a "done" (unread) state (decision 050).** Decision 014's
  done light keys off a non-empty `detail` (the wrap-up message `Stop` writes), and Cursor's
  bridged finish carries none (verified in #038) — so every Cursor turn ended in dim gray,
  indistinguishable from an hour-old idle. Fixed frontend-only in `app/src/main.js`: a new
  `noteFinishes()` runs first thing each poll and records a finish when a **Cursor** session
  moves from a non-idle state to `idle` (keyed by that `updated_at`); `isFinishedTurn()` accepts
  it alongside the `detail` test, so the light turns white and everything downstream — the
  `reviewedAt` click-to-acknowledge, the re-light on the next turn, urgency sort, tray priority,
  the "done" chime — works unchanged. Also lights up the #048 reconciled finishes (Cursor's own
  record says terminal → forced `idle` → a transition). Scoped to Cursor deliberately: Claude
  Code's `detail` survives a bar reload, a remembered transition doesn't. No hook, schema, or
  Rust change. Re-runnable check: `node app/tests/unread-light.mjs` (evals the shipped functions
  out of `main.js`; 5 lifecycle checks pass). README updated. **Left to verify live:** rebuild via
  `./install.sh`, run a Cursor agent with the bar up, confirm white-on-finish and dim-on-click.

- **2026-08-11** — **Cursor click-to-conversation actually works now (decision 049).** Reported
  again after v0.6.1: clicking a Cursor light only fronted Cursor, never opened the
  conversation. Decision 047's tray-row match required the row title to *equal* the composer's
  name (bullet aside), but an AX dump of the live menu shows Cursor appends a status suffix —
  `"Folder upload functionality, Running"` — so only **idle** composers matched, and a light is
  clicked precisely when its session is running. `tray_row_is` now accepts the bare name or the
  name plus `", "`; verified live (`pressed=true` on the composer that failed before, Cursor
  opening that conversation). New `cursor_dump_tray` ignored test prints the real row titles for
  the next time Cursor changes them. The other two Cursor sessions in the repro had no tray row
  at all — they are subagents, which #048 already keeps off the bar.

- **2026-08-11** — **Released v0.6.1.** Version bumped 0.6.0 → 0.6.1 across
  `tauri.conf.json`, `package.json`, `package-lock.json`, `Cargo.toml`/`Cargo.lock`, and the
  README's download link and release-command example. Patch rather than minor: everything
  since v0.6.0 is a Cursor fix — click-to-focus opening the conversation instead of a new
  agent (#047), and lights/badges reconciling against Cursor's own record (#048). Cut by
  merging `development` into `main` and pushing the `v0.6.1` tag, which triggers the release
  workflow of decision 041.

- **2026-08-11** — **Cursor lights reconcile against Cursor's own record (decision 048).** Two
  reported bugs, one cause: a Cursor light sat green for 95 min on a finished agent, and
  archived agents kept their lights. Cursor's hook bridge is lossy at end-of-life — archiving
  fires no `sessionEnd`, and a **subagent** or **aborted** turn fires no `stop`, so the light
  freezes at the last `preToolUse`. `list_sessions` now reads all status files first, then runs
  one throttled (5s TTL) `sqlite3 -readonly` query over all Cursor ids against Cursor's
  `composerHeaders` table + `composerData.status`: archived (and deleted) composers are pruned,
  `isSubagent` composers get **no light**, and a
  terminal `status` forces `idle`. Guarded so it can never grey a working agent: the light must
  have been silent 60s+ *and* Cursor's own write must be newer than its last hook event (a live
  agent showed a stale `status="aborted"` on disk, which is exactly what this rejects), and a
  failed query reconciles nothing. The **subagent badge** had the same disease — `subagentStop`
  fired for neither subagent, so a leftover marker file left a permanent "1 subagent running"
  badge — so for Cursor the badge now comes from Cursor's `subagentComposerIds`, counting only
  the linked composers still firing events; those events also serve as the proof that an agent
  silent on "Running Task" is still busy (Cursor's own `lastUpdatedAt` was tried as that guard
  first and rejected: it read 13:27 while the agent's hooks fired through 13:45). Claude Code
  keeps the marker files, whose Stop hook is reliable. New check: `cargo test --release --
  --ignored --nocapture cursor_facts`. Verified live — 5 archived pruned, 3 subagent lights gone,
  the stuck agent's badge resolves to zero and its light to idle. README updated.

- **2026-08-11** — **A Cursor session light opens that conversation instead of a new agent
  (decision 047).** Clicking a Cursor light landed the user on a *new agent page in the same
  repo*: `focus_session` focuses a window by running the IDE CLI (decision 016), and Cursor
  3.15.6's main process intercepts `cursor <folder>` — with the Agent ("glass") window active,
  `resolveGlassCliFolderTarget` routes it to `vscode:createNewComposer {folderUri}`. Verified in
  Cursor's bundle plus the user's `glassMode = true`. Fix: for `ide == "cursor"` the CLI is never
  called. A Cursor session's `session_id` *is* its `composerId` (confirmed against Cursor's
  `composerData:`/`bubbleId:` keys), so the click resolves the composer's name via
  `sqlite3 -readonly` on Cursor's `state.vscdb` (the `.name` field only — no message content) and
  AXPresses the matching row in Cursor's tray menu, which sends `vscode:openComposer` to that
  conversation, then activates Cursor. Decision 045's press was generalized into
  `cursor_press_tray_row(predicate)` + a recursive `press_in_menu`, shared with the pip, and both
  now reach the "View More" submenu too. Misses (unnamed composer, or older than the tray's 10
  recents, or no Accessibility grant) fall back to raise + activate — never a new agent. New
  re-runnable check: `AGENTSTATUS_TEST_SESSION=<id> cargo test --release -- --ignored --nocapture
  cursor_press_composer` (printed `name=Some("Simplify presentation slides") pressed=true` live).
  README updated: Cursor click behavior and the Accessibility requirement.

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
