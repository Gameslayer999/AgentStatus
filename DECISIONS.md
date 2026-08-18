# DECISIONS.md — Architecture & Tooling Decisions

> Every significant choice — architecture, tooling, the status-file schema, the
> event→state mapping, the display stack, or a reversal of a prior decision — is logged
> here with its context, the options considered, the choice, and the reasoning
> (Agent Guideline #9). Code captures *what* the system does; this file captures *why*.

---

## Decision Index

| # | Date | Decision | Status |
|---|------|----------|--------|
| 001 | 2026-07-05 | Status signal source: Claude Code hooks (not transcript polling / no public API) | Accepted |
| 002 | 2026-07-05 | Signal transport: hooks write a shared JSON status file (not a local server) | Accepted |
| 003 | 2026-07-05 | Display layer: Tauri borderless always-on-top window (not SwiftUI / Electron / menu bar / VS Code status bar) | Accepted |
| 004 | 2026-07-05 | Session identity: key by `session_id`, label by project folder; stale detection via heartbeat timestamp | Accepted |
| 005 | 2026-07-05 | Install model: global hooks (install once per machine) delivered via a self-installing app **and** an optional VS Code extension | Accepted |
| 006 | 2026-07-05 | Verified event→state mapping on Claude Code 2.1.201 (blocked = `PermissionRequest`, not `Notification`); window scoping = workspace folder via `~/.claude/ide/*.lock`, auto "this window" is the extension's job | Accepted |
| 007 | 2026-07-05 | Status store: one file per session (`~/.claude/status/sessions/<id>.json`), not a single shared JSON — refines #002 | Accepted |
| 008 | 2026-07-05 | macOS overlay: non-activating NSPanel (via `tauri-nspanel`) + Accessory app — the only way to float over other apps' full-screen spaces | Accepted |
| 009 | 2026-07-05 | Hover tooltip (task + activity) and subagent tracking (count badge) — subagents tracked by SubagentStart/Stop lifecycle only (their tool calls aren't attributable) | Accepted |
| 010 | 2026-07-05 | Subagent storage: one marker file per subagent (`sessions/<id>.subagents/<agent_id>`), not a field in the session JSON — parallel subagents were racing the shared file | Accepted |
| 011 | 2026-07-05 | Packaging: self-installing app — bundles the hook (`include_str!`), writes it to `~/.claude/status/report.sh` and registers it on launch (release builds only); ships `.app` + `.dmg` + `install.sh` | Accepted |
| 012 | 2026-07-05 | VS Code extension: per-window status-bar items (scoped to the window's workspace); click-to-focus via Claude Code's own `claude-vscode.editor.open` command (no URI-consent prompt) | Accepted |
| 013 | 2026-07-06 | Red = `StopFailure` only (not `PostToolUseFailure`, which is recovered-tool-failure noise); `AGENTSTATUS_IGNORE` env var opts a session out of tracking (for programmatic/headless Claude calls) | Accepted |
| 014 | 2026-07-06 | Split idle into **done** (finished turn, unreviewed — steady white light) vs **idle** (acknowledged — dim gray); `reviewed` flag is app-local, cleared by clicking the light (which also focuses the session), reset by the next finished turn | Accepted |
| 015 | 2026-07-06 | Settings panel: right-click the bar toggles an inline panel (not a gear icon / separate window); first setting = orientation (horizontal/vertical) via a `.vertical` CSS class + existing auto-resize; persisted in webview `localStorage`, frontend-only | Accepted |
| 016 | 2026-07-06 | Bar click-to-focus via the IDE's own CLI (`code`/`cursor <root>`), not `open -a <folder>` — `open -a` spawns a new window when the target is a full-screen window on another Space; workspace root resolved from `~/.claude/ide/*.lock` so subfolder `cwd`s map to the right window. AppleScript window-raise rejected (can't see full-screen windows on inactive Spaces) | Accepted |
| 017 | 2026-07-06 | Settings: light **size** (slider) + **per-state colors** (native `<input type=color>`), driven by CSS variables on `#bar` from `localStorage`; glow derived via `color-mix`. Plus `keepOnScreen()` — after each resize, shift the window inward if it overflows a monitor edge, so the panel opens "toward the middle." Frontend-only | Accepted |
| 018 | 2026-07-06 | Cursor support: Cursor natively runs `report.sh` via its Claude-compat bridge (reads `~/.claude/settings.json`), so running/idle/error/remove work for free; native `~/.cursor/hooks.json` entries add the events the bridge drops (`subagentStart/Stop`, `postToolUseFailure`). New `ide` field (`cursor` when `.cursor_version` present, else `vscode`) drives per-IDE click-to-focus; Cursor cwd = `workspace_roots[0]`; blocked is unavailable on Cursor (no event) | Accepted |
| 019 | 2026-07-06 | Click a bar light → focus the exact **session tab** (not just its window) via a bar→extension relay: the bar writes `~/.claude/status/focus-request.json` `{session_id, requested_at}`; the per-window extension polls it and calls the popup-free in-editor `claude-vscode.editor.open`. Chosen over the `vscode://…open?session=` deep link, which shows a consent popup on every click (verified live). Complements decision 016's window-raise. Verified end-to-end (bar click from another window → correct tab) | Accepted |
| 020 | 2026-07-06 | Single-instance guard via `tauri-plugin-single-instance` (release only): a second launch — installed copy or dev build, both keyed by `com.agentstatus.app` — pings the running instance and exits instead of drawing a duplicate overlapping bar. Off in dev so `tauri dev` runs alongside the installed copy | Accepted |
| 021 | 2026-07-06 | Faster click-to-focus: fire a fast `osascript` System Events window-raise (`set frontmost` + AXRaise by workspace-root basename, ~0.2s) **in addition to** the decision-016 IDE CLI (~1.1s). Fast path handles the same-Space case; CLI still covers cross-Space / full-screen. Needs a one-time Accessibility grant; silently no-ops (falls back to CLI) without it | Accepted |
| 022 | 2026-07-06 | Persist bar position across restarts/rebuilds in `localStorage` (frontend-only, no window-state plugin), restored on launch over the config's `center: true`; drag bounded to the union of all monitors with soft edge magnetism; a user-facing **Reload** button in the settings panel; `install.sh` auto-quits+relaunches an already-running instance so rebuilds take effect past the single-instance guard | Accepted |
| 023 | 2026-07-06 | Settings: bar **opacity** slider (0–100%) drives a `--bar-opacity` CSS variable on `#bar` that fades the whole pill together — fill, border, shadow, and backdrop-blur (multipliers normalized so 82% reproduces the original look) — while the lights stay fully opaque so the signal never fades; at 0% the pill vanishes and only the lights float. Fading the chrome (not just the fill) makes the control perceptible when the bar is minimized to a few lights. Persisted in `localStorage` as a whole percent, frontend-only, same pattern as decision 017 size/padding | Accepted |
| 024 | 2026-07-06 | First public release **v0.1.0**: a git tag + GitHub Release with the prebuilt **Apple-Silicon-only, unsigned** `AgentStatus_0.1.0_aarch64.dmg` as the primary install path (build-from-source via `install.sh` kept for Intel/devs). Unsigned/unnotarized → users clear quarantine (`xattr -dr com.apple.quarantine`) or "Open Anyway"; README rewritten to lead with the DMG download and macOS-15+ Gatekeeper steps | Accepted |
| 025 | 2026-07-06 | Settings: light **sort** toggle — **Window** (group sessions by workspace folder, default) vs **Urgency** (attention states first). "Window" is proxied by the session `cwd` (hooks expose no true per-window id — #006), so two windows on the same folder merge (accepted limit). Frontend-only sort in `localStorage` (`agentstatus.sort`), same pattern as orientation | Accepted |
| 027 | 2026-07-07 | Stale-light fix: prune a session the instant its IDE window is gone, not just after the 2h idle timer. `list_sessions` builds the set of live workspace folders from `~/.claude/ide/*.lock` (skipping locks whose owning `pid` is dead — force-quit/crash), and deletes any session whose `cwd` maps to no live folder (empty cwd = anonymous ghost, matches nothing). Purely additive — the `MAX_IDLE_SECS=2h` backstop (#004) is unchanged and still covers a superseded session sharing a live window's lock. Caveat: a Claude session in a standalone terminal (no IDE lock) is pruned when any IDE is open; skipped entirely when no live lock exists so a no-IDE machine / bad read never nukes every light | Accepted |
| 028 | 2026-07-07 | Settings: a **Quit** button in the panel footer that calls a new `quit_app` Tauri command (`app.exit(0)`). As an Accessory app (no Dock icon, no app menu) the bar had no in-UI way to quit — only Activity Monitor / `kill`. Red-tinted hover marks it as the one destructive footer action; hooks keep writing status files, so relaunching repopulates the bar | Accepted |
| 026 | 2026-07-06 | Presentation-mode toggle: the bar runs **floating** (the NSPanel, default) **or in the macOS menu bar** — a `tray-icon` NSStatusItem drawn from a webview-rendered dot image (with a condense-to-single-summary-dot option), clicked to reveal the same panel as a popover below it. Amends #003 (menu bar is now an optional mode, not rejected). Settings-panel toggle, `localStorage`-persisted; menu-bar mode forces horizontal; tray ops marshaled to the main thread; icon forced non-template so the dots keep their colors | Accepted |
| 029 | 2026-07-09 | Codex compatibility: install the shared `report.sh` into Codex user hooks at `~/.codex/hooks.json` alongside Claude's `~/.claude/settings.json`, using only Codex-supported events from the official Codex Hooks manual. The reporter accepts Codex thread/conversation id fields, falls back to the hook process cwd, tags sessions as `ide:"codex"`, and skips IDE-lock pruning for them; click-to-focus opens `Codex.app` | Accepted |
| 030 | 2026-07-09 | Product rename: `ClaudeStatus` → `AgentStatus` now that the bar targets Claude Code, Codex, and Cursor rather than only Claude Code. App bundle, product name, docs, extension IDs, localStorage keys, hook backup suffixes, and release asset names move to AgentStatus; legacy `CLAUDESTATUS_DIR` / `CLAUDESTATUS_IGNORE` and old `/Applications/ClaudeStatus.app` cleanup remain for migration | Accepted |
| 031 | 2026-07-09 | Codex live-state fallback: if Codex lifecycle hooks are not yet trusted/loaded for an already-running thread, synthesize Codex lights from `~/.codex/state_5.sqlite` (`threads.updated_at`) so active Codex work shows green. Also prune status files whose `cwd` no longer exists, which removes rename ghosts like the old `ClaudeStatus` workspace | Accepted |
| 032 | 2026-07-09 | Codex open/close lifecycle: Codex emits **no** signal on conversation open or close (verified: no `SessionEnd` hook exists; `SessionStart` is deferred to the first turn; `updated_at`/`recency_at` advance only on turn starts). So: installers pass an explicit `codex` arg to `report.sh` (payloads are Claude-shaped and unsniffable — replaces the never-firing #029 heuristics); Codex lights expire after 10 min idle (`CODEX_IDLE_SECS`, user-approved) instead of 2h, drop instantly when no `codex` process is alive, and exclude archived threads; click-to-focus targets the VS Code window (Codex is the `openai.chatgpt` extension; `open -a Codex` was a no-op) | Accepted |
| 033 | 2026-07-15 | Antigravity IDE as a fourth host (retroactive — shipped in 3195f11 undocumented). Hooks install into `~/.gemini/config/hooks.json` under an `agentstatus` object key (not Claude's `hooks` map), registering `PreInvocation`/`PreToolUse`/`PostToolUse`/`Stop` as `report.sh <Event> antigravity` — declared host per #032. Payload differs from Claude: workspace in `workspacePaths[]`, tools in `toolCall.name`/`toolCall.args`, and **no prompt text**, so the task label is recovered from the thread transcript and unwrapped from `<USER_REQUEST>`. That transcript read is gated on `ide == antigravity`: ungated it walked its fallback chain into the real Claude transcript on every `UserPromptSubmit` (~98 ms + a 10 MB read per turn, discarded). Antigravity uses IDE-lock pruning and the 2h idle backstop like vscode/cursor; click-to-focus targets `Antigravity IDE.app`. **Hook events not yet verified against a live install (Guideline #4)** | Accepted, unverified |
| 034 | 2026-07-28 | README lightbar visuals: a generator (`docs/gen-readme-art.mjs`) renders five self-contained SVGs (hero / states / hover+badge / orientation / settings panel) from the exact `styles.css` values, committed under `docs/` and embedded in the README. Reproducible art (Guideline #8), not one-off screenshots; GitHub strips SVG animation so pulsing states render static and are labeled | Accepted |
| 035 | 2026-07-28 | Settings: **audio alerts** — an edge-triggered chime when a session transitions *into* an attention state (blocked/error/done). Master On/Off `.seg` toggle reveals an inline sub-panel (per-state checkboxes + volume), reusing the #015-conditional-row disclosure pattern rather than a separate OS window (which would fight the single hugging NSPanel). Chimes are short WebAudio tones (no bundled asset → no CSP/load concern); off by default (UI Principle #1). `prevChimeState` map fires only on a state *change* and is seeded silently on the first poll so pre-existing blocked sessions don't blast on launch. Frontend-only `localStorage` (`agentstatus.audio`/`.chimes`/`.volume`); never touches the status files | Accepted |
| 036 | 2026-07-28 | App icon redesign: three glowing status lights (green/orange/red) on a dark Big Sur squircle, replacing the off-brand cyan/yellow swirl. Reproducible SVG master (`docs/icon-master.svg`) → `tauri icon` regenerates the whole set; a 256px export doubles as the README header logo (`docs/logo.png`) | Accepted |
| 037 | 2026-07-28 | Overlay panel collection-behavior set via `objc2-app-kit` (maintained) instead of `tauri-nspanel`'s deprecated `cocoa`-typed `set_collection_behaviour`, clearing 5 `deprecated` build warnings. Also gate `mod install` to release (it's only called under `#[cfg(not(debug_assertions))]`), clearing the dev-build dead-code warnings. Same window object, same two flags — no behavior change | Accepted |
| 038 | 2026-07-28 | Cursor fix + menu-bar mirror: (1) stop lock-pruning Cursor sessions — Cursor writes no `~/.claude/ide/*.lock`, so decision-027 pruning deleted every Cursor light the moment any VS Code window was open; Cursor now prunes like Codex (drop when no `Cursor` process, plus the idle backstop). (2) Add a supplementary **Cursor menu-bar pip**: Cursor's live status is renderer-memory-only, but its macOS menu-bar item exposes an aggregate count of composers awaiting the user (its only externally observable attention signal), read via the **Accessibility API directly** (AXExtrasMenuBar → child title, needs only Accessibility not Automation) and rendered as one hollow-ring pip that clicks to activate Cursor — the "done"/attention bit Cursor's hooks don't provide | Accepted |
| 039 | 2026-07-28 | Sign the app with a stable **self-signed** code-signing identity (`hooks/sign-app.sh`, run by `install.sh`) so macOS Accessibility (TCC) trust survives rebuilds. Ad-hoc signing keys trust to the code hash, which changes every build — invalidating the grant the Cursor pip (038) and fast window-raise (021) need, so trust never stuck during iteration. A stable Designated Requirement (`identifier "com.agentstatus.app" and certificate leaf = H"…"`) makes the grant persist. Revises the "unsigned" decisions 011/024 for local builds; downloaded DMGs are unaffected (self-signed anchor is per-machine) | Accepted |
| 040 | 2026-07-29 | **Remove Codex and Antigravity support** (reverses #029/#031/#032 for Codex and #033 for Antigravity). Neither host was ever verified against a live install per Guideline #4 — #033 shipped explicitly "Accepted, unverified", and the Codex path leaned on unverifiable inference (a `state_5.sqlite` read, a `pgrep codex` liveness probe, a 10-min bespoke idle timeout) to paper over lifecycle events that may not fire. Unverified hosts can only produce lying lights (UI Principle #4). Removed: both installers' host registrations, `report.sh`'s declared-host `$2` arg and every Codex/Antigravity payload shape, the sqlite fallback and `CODEX_*` timeouts, and their click-to-focus targets. `install.rs` and `setup.mjs` now **clean up** the entries earlier versions wrote to `~/.codex/hooks.json` and `~/.gemini/config/hooks.json`, preserving any other hooks in those files. Supported hosts are Claude Code (VS Code) and Cursor | Accepted |
| 041 | 2026-07-29 | **Automate releases via GitHub Actions on a `v*` tag push** (`.github/workflows/release.yml`). Releases were manual through v0.4.2. Trigger is the tag, not a push to `main`: merging is routine and frequent, so main-triggered publishing either fires on every docs commit or needs a version-diff guard that silently does nothing most of the time — a tag makes releasing an explicit act with an obvious audit trail. The job builds the arm64 DMG on `macos-15`, then `gh release create --generate-notes`. It **fails fast if the tag and `app/src-tauri/tauri.conf.json` disagree**, the one way a tag-driven release can ship a wrong version number. Output stays unsigned/un-notarized (runners hold no Developer ID cert), unchanged from the manual releases; the self-signing of #039 is build-from-source-only and plays no part | Accepted |
| 042 | 2026-07-30 | **Hollow "unknown" light for sessions we get no signal from** — display-only, derived in the frontend from `ide == "cursor" && cwd == ""`. A folder-less Cursor window fires the bridged `sessionStart` (so a light exists) and then nothing at all — no prompt/tool/stop events, since Cursor runs command hooks only with a folder open (#018) — so its recorded `idle` is one stale event, not a state. `displayState()` returns `unknown` and the dot renders as a hollow gray ring (`.dot.unknown`), quiet and non-attention, saying "a session is here, its state is unreadable" instead of a solid dot asserting idle (UI Principle #4). No hook change and no status-file schema change (the app can derive it); scoped to this one verified case, not a generic heartbeat timeout | Accepted |
| 044 | 2026-07-30 | **Settings: `Unknown` Show/Hide toggle** — whether the hollow no-signal lights of #042 appear on the bar at all. Defaults to **Show** (preserves the behavior #042 shipped hours earlier); **Hide** filters them from both the bar and the menu-bar tray image. Frontend-only `localStorage` (`agentstatus.showunknown`), same pattern as orientation/sort/opacity. `latestSessions` stays complete and a new `visibleSessions()` applies the filter at draw time, so toggling repaints instantly from memory instead of waiting for the next poll, and re-enabling brings the lights straight back. Chosen over dropping the sessions in `list_sessions` (a display pref does not belong in the backend, and the data would be gone from the tray/chime paths too) | Accepted |
| 045 | 2026-07-30 | **Cursor pip clicks through the waiting composers** — a click now presses the top notification entry in Cursor's own tray menu (a native `NSMenu`, so every row is an `AXMenuItem` readable/pressable without opening it; notified rows are titled `"• <name>"`). Cursor opens that composer, focuses its window, and marks it read, so its count drops by one and the next click lands on the next one waiting — verified live on Cursor 3.12.10 (` 2` → ` 1`). New `cursor_open_next_attention` command; falls back to the old "just activate Cursor" behavior when nothing is pressable. Extends #038; same Accessibility grant (#039), no hook/schema/installer change | Accepted |
| 046 | 2026-07-30 | **Activate Cursor after the pip's press** — #045's press cleared the notification but left the user where they were: macOS focus is per-*application*, and an `AXPress` from a background process never changes the frontmost app, so Cursor raised the composer's window behind everything else. `cursor_open_next_attention` now calls a shared `activate_cursor()` (`open -a Cursor`, the same activation `focus_session` already used for an empty `cwd`) after a successful press — press first so Cursor picks the window, activate second so the app comes forward. Chosen over an `AXFrontmost` write (adds an AX write path to save a few ms) and over `open -a Cursor <folder>` (the pip is aggregate, has no folder, and a folder arg risks a new window) | Accepted |
| 047 | 2026-08-11 | **Cursor session lights open the conversation, not a new agent** — with Cursor's Agent ("glass") window active, `cursor <folder>` is intercepted by Cursor's main process (`resolveGlassCliFolderTarget` → `vscode:createNewComposer {folderUri}`) and starts a **new agent in that repo** instead of focusing anything, so the decision-016 CLI route silently became wrong for Cursor. A Cursor session's `session_id` *is* its `composerId`, so a click now resolves the composer's name (read-only `sqlite3` on Cursor's `state.vscdb`, `composerData:<id>` → `.name`) and AXPresses that row in Cursor's tray menu — the #045 press path, generalized — then activates Cursor. No tray row (unnamed, or older than the tray's 10 recents) falls back to raise + activate; the `cursor` CLI is never invoked | Accepted |
| 048 | 2026-08-11 | **Cursor lights reconcile against Cursor's own record** — Cursor's bridged hooks are lossy at end-of-life: archiving an agent fires no `sessionEnd`, and a subagent or aborted turn fires no `stop`, so lights sat green on finished agents (one for 95 min) and archived agents lingered until the 2h backstop. `list_sessions` now runs one throttled (5s TTL) `sqlite3 -readonly` query over all Cursor session ids against Cursor's `composerHeaders` table + `composerData` status, and drops archived/deleted composers, hides `isSubagent` composers (they belong to the parent's subagent badge), and forces `idle` when Cursor says `completed`/`aborted`. The subagent badge for a Cursor agent likewise comes from Cursor's own parent→subagent linkage (`subagentComposerIds`, keeping the ones still firing events) rather than the marker files, whose `subagentStop` leaks a permanent "1 subagent running" badge. Guarded: only lights silent 60s+ with no live subagent of their own are overruled, and a failed query reconciles nothing | Accepted |
| 049 | 2026-08-11 | **Match a Cursor tray row by name prefix** — #047's click-to-conversation matched the tray row whose title *equals* the composer name (allowing only the unread bullet), but Cursor also appends a live status suffix: an AX dump of the real menu shows `"Folder upload functionality, Running"`. Exact matching therefore missed every **running** composer — the state a light is in when the user clicks it — so every real click fell through to the "just activate Cursor" fallback. A row now matches on the bare name or the name followed by `", "` (`tray_row_is`), the separator keeping one composer's name from matching a longer one. New `cursor_dump_tray` test prints the live row titles | Accepted |
| 050 | 2026-08-12 | **Cursor gets a "done" (unread) light, derived from the observed transition** — decision 014's done light keys off a non-empty `detail` (the wrap-up message `Stop` writes), and Cursor's bridged finish carries none, so a finished Cursor turn dropped straight from green to dim idle with no unread cue. The bar now records a finish when a poll sees a **Cursor** session go from a non-idle state to `idle` (`noteFinishes`, keyed by that `updated_at`), and `isFinishedTurn` accepts it alongside the `detail` test — so the light goes white until clicked, and the existing `reviewedAt` ack + re-light cycle works unchanged. Frontend-only (`app/src/main.js`), no hook, schema, or backend change. Scoped to Cursor: Claude Code's `detail` is a real, restart-proof signal, whereas transitions are forgotten on reload. A session already idle at the first poll is never a finish, so launching the bar can't invent unread lights | Accepted |
| 051 | 2026-08-12 | **Re-check the window against its content every poll** — the bar came up sized for *one* light with five in it, drawing a pill with a cut-off bottom. `resizeToContent()` is edge-triggered (a light added/removed, a geometry setting changed) and awaits two animation frames before measuring; the webview delivers none while the window isn't painting, so a resize landing during launch/relaunch is simply never applied, and the only measurements that stuck were the pre-poll ones taken when the bar held just the "empty" placeholder. A new `ensureSized()` runs after each poll's render, measures the content synchronously, and re-resizes when it disagrees with the last size actually applied — a level check behind the edges, so a lost resize self-corrects on the next tick instead of never | Accepted |
| 052 | 2026-08-12 | **Cursor's tray row vetoes the finished-turn reconcile** — #048 greys a silent Cursor light when `composerData.status` is terminal, guarded only by 60s of hook silence. But 60s of silence isn't evidence an agent stopped: one writing a large file went **107s** between hook events while its `status` still held the value flushed at the end of its *previous* turn, so its light was forced to `idle` mid-turn — and #050, which derives Cursor's done light from a watched non-idle→idle transition, rendered that as a white **"finished, unread"** light on an agent that was still working (it also swallowed the real unread light 47s later, an idle→idle no-op). Fixed by adding a fourth condition: Cursor's tray row for that composer must not read `"<name>, Running"` (#049), the one live status signal Cursor exposes. Reuses #045/#047's AX walk with a record-and-decline predicate, cached at the same 5s TTL, read lazily; the name is one extra column on #048's existing query. Positive evidence only — an empty tray read (no Accessibility grant, composer off the recents list) never causes a veto, so it can only keep a light green, never light one up | Accepted |
| 054 | 2026-08-13 | **Claude Code in the terminal (`cli`) and in Claude Desktop (`claude-desktop`) become first-class hosts** — both already ran the hook (all three surfaces read the same `~/.claude/settings.json`), so the signal layer needed nothing; they were invisible because everything non-Cursor was tagged `vscode` and therefore lock-pruned. `report.sh` now tags the host from `$CLAUDE_CODE_ENTRYPOINT` and records the owning `claude` pid; the app prunes those hosts by pid liveness instead of IDE locks, and clicks focus the terminal tab (Terminal.app, by `tty`) or Claude. Claude Desktop's **normal chat threads are out of scope** — no hook mechanism exists and no local state is observable | Accepted |
| 053 | 2026-08-12 | **Tooltip identifies a light by the host's own session name**, not the project folder alone — the bar's first line was the folder basename, so two sessions in one folder had byte-identical tooltips. Claude Code names every session in `~/.claude/sessions/<pid>.json` (`sessionId`, `name`, `nameSource`); Cursor names every composer in `composerData.name`, already queried by #048. The app joins both by session id and the tooltip head becomes `folder · name` ("AgentStatus · agentstatus-5b"), dropping the name when it is absent or repeats the folder. App-side read only — no hook change, no status-file change, and no transcript is read (Guideline #5). Rejected: an LLM-style session title (no `summary`/`title` entry exists in any transcript on the installed 2.1.200 — nothing to read) and recording the first prompt as a title (a schema change for something the `task` line already covers) | Accepted |
| 055 | 2026-08-13 | **A Ghostty light focuses the exact tab (and split)** — #054 left Ghostty at app-level focus because it publishes no tty, so a click brought Ghostty forward on whatever tab was last active. Two things changed since: Ghostty 1.3 ships an AppleScript dictionary (`window` → `tab` → `terminal` surface, each with a title, plus a `focus` command that selects the surface and fronts its window), and Claude Code 2.1.231 writes a session title into the terminal title bar, stored as an `ai-title` record in the transcript — which #053 checked for on 2.1.223 and correctly found absent. The app reads the last `ai-title` for the session and runs one `osascript` that focuses the surface whose title **contains** it (the leading glyph is the activity spinner), acting only on an **exactly one** match. No `ai-title`, no hit, several hits, Ghostty < 1.3, or a refused Automation grant all fall back to fronting the app — the old behaviour, never a wrong tab (UI Principle #4). Reading the transcript is a flagged exception to Guideline #5: only the title is extracted, nothing is stored. No hook change, no schema change | Accepted |
| 064 | 2026-08-13 | **A light that names no session does nothing, and a background agent opens in a tab** — a pre-warmed spare's light was clicked inside #054's 20s grace, and the click ran `open -na Ghostty` (a *second instance*) on `claude attach <spare>`; `attach` on an id with no live job does not fail but lands in Claude Code's **agent view**, so the user got a new window listing every session. Underneath it, a latent defect: `zsh -lc` is non-interactive, never reads `.zshrc`, and that is where `~/.local/bin` is set — so #054's `claude agents --json` returned nothing whenever the bar was launched from Login Items rather than a shell, silently disabling **all** CLI reconciliation (fail-open). Fixes: resolve `claude` by absolute path and run it with no shell; a click on a CLI light Claude Code does not list does nothing; an unlisted light whose pid owns no terminal is dropped on sight (a pid *with* a tty, and a pre-054 file with no pid, keep the 20s grace); and a background agent opens in a **tab of the running Ghostty** via 1.3's scripting dictionary, which also removes #055's two-instances limitation at the source | Accepted |
| 065 | 2026-08-13 | **A finished background agent's light retires after 5 minutes** — lights lingered for sessions with "no currently active tab or terminal anywhere". A `--bg` job has no terminal by construction and Claude Code keeps its process alive and listed after the work is done, so #054's pid-liveness rule never fired and #064's unlisted rule never applied; the lights sat until the two-hour backstop. New rule (g): retire a `cli` light once Claude Code reports it `kind: background` + `status: idle` (a new `status` field on `CliFact`) **and** it has been hook-silent for `CLI_BG_DONE_SECS` = 5 min — a clock rather than a category, so "just finished" stays readable while an abandoned job stops taking a slot. Never applied to a `blocked` or `error` light: those are the states the user must act on and both go quiet by nature, so a silence timer would delete exactly the wrong ones. Interactive sessions are untouched — verified live that an idle-38-minute terminal session in an open tab keeps its light while two background ghosts went within one poll | Accepted |
| 061 | 2026-08-13 | **A click reaches a background agent where it already is** — "the ghostty light kept opening a new tab": a background session has no terminal to focus, so the click attaches, and nothing asked whether that agent was already open. Reproduced through `focus_session` itself: 2 -> 3 -> 4 surfaces, one per click. #055's matcher could not be reused — an attached agent's tab carries the same session title as every other view of it, so three surfaces matched and #055's exactly-one rule would have declined and attached again. `focus_ghostty_surface` gains `require_unique`, set **true** for an interactive light (the alternative is fronting the app, so a wrong tab is worse than no tab) and **false** for a background one (the alternative is creating another terminal, so any existing surface wins). `attach_background_agent` now focuses an existing surface, else fronts Ghostty when `pgrep` finds an attach already running for that id, else opens the tab. Also records a **false verification** that nearly shipped: the first check piped a non-compiling test to /dev/null, so the clicks never ran and the unchanged surface count read as a pass | Accepted |
| 066 | 2026-08-13 | **A surface whose title *ends with* the session title beats one that merely contains it** — "sometimes clicking the light for a ghostty session does nothing". Measured every light through `focus_session` with a decoy focus set first: a click lands exactly when #055's title match finds its surface, and does nothing when it declines — the fallback is `activate Ghostty`, which changes nothing on screen when Ghostty is already frontmost, so a declined match and a dead click look identical. #055 matched with `contains` **and** required uniqueness, so an unrelated surface whose title merely spanned the session title counted as a second hit and killed the click. Claude Code writes the title as a trailing run (`◑ <title>`), so matching now has two grades: **strong** (ends with — several such surfaces are several views of the same session, so the first is right by construction) and **weak** (contains — may be something else, so still must be unambiguous). Not fixable: a terminal used only to start a background agent has no `ai-title` of its own and is titled with that agent's title; `working directory` is identical across all surfaces, `parkedJobId` is stale, and Ghostty exposes no tty/pid per surface — the README now states this plainly | Accepted |
| 056 | 2026-08-13 | **Re-adding Codex and Gemini starts with observation, not with restoring the deleted code** — both were removed in #040 as unverified, so a re-add repeats the observe-then-build sequence using the existing logger tooling, and must first gate the #040 cleanup (`install.rs:80-121`, `setup.mjs:73-82`) that would otherwise delete the new entries on the next launch. Codex is verifiable today (the `openai.chatgpt` VS Code extension is installed) at ~2–2.5 days; Gemini is **blocked on which product is meant** — Antigravity IDE, Gemini CLI, or Antigravity CLI are three different config paths and event sets, and only the IDE is installed here. Only Codex can reach orange; **none of the three can reach red** | Proposed — blocked on user |
| 057 | 2026-08-13 | **Token/cost tracking: data exists for Claude Code only, and the maintenance is the real cost** — transcripts carry per-call `message.usage`/`model`, but hook payloads carry nothing, no aggregated local source exists, and Cursor stores context-window fullness with no cost at all, so any version is Claude-only. Options: (1) defer, (2) last-turn cost in the tooltip ~½–1 day, (3) cumulative cost + settings-panel readout ~2–3 sessions. Constraints: the read goes app-side, never in the hook (reversing #040 / Guideline #3); it is a schema change needing approval; nothing numeric goes on the bar (UI Principle #1); the price table must be hardcoded and rots on every model release | Proposed — awaiting go/no-go |
| 058 | 2026-08-13 | **A fallback view for high session counts** — the one-light-per-session bar is the project's one unmatched design property (every competitor aggregates to a single icon), but it grows at `8 + 23N` px, so 30 sessions is a 698 px bar. Five options drafted (folder grouping, overflow chip, grid wrap, auto-shrink, attention-only); recommendation is the **overflow chip** bounded by urgency sort with folder grouping as an orthogonal toggle, auto-shrink rejected outright for fighting UI Principle #1. Must be a **threshold, not a mode switch**, so per-session lights stay the default at normal counts | Proposed — awaiting choice |
| 059 | 2026-08-13 | **Hook-only is a real risk for the core path, but the fix is one guarded reconcile, not a second architecture** — six *observed* failures (hooks not executing at all, events that never fire, dropped end-of-life events, a 95-minute stuck green light), plus two silent undetectable ones (hook registration disappearing; `jq` missing on the DMG path, which `install.sh:11` guards and `install.rs` does not). Cursor already has a state reconcile (#048/#052) and CLI/Desktop have pid liveness, but **Claude Code has no state reconcile at all** — while `claude agents --json` and `~/.claude/sessions/<pid>.json` both carry `status`/`statusUpdatedAt` that the app already reads and discards. Fix mirrors #048's guarded shape. **Unverified: whether a VS Code session reports `status`** — confirm before writing code | Implemented by #067 for the turn-state half; the install-integrity half (hook registration vanishing, missing `jq` on the DMG path) is still open |
| 060 | 2026-08-13 | **The tooltip names the application a session runs in** — a light said which project and which session but never which app, so a user reading `AgentStatus · agentstatus-5b` could not tell a VS Code tab from a Ghostty split from Claude Desktop before clicking it. `list_sessions` now returns an `app` string and the tooltip head becomes `folder · name (app)`. For the IDE hosts it is the `ide` field spelled the way the app is named on screen; for a terminal session it is the **emulator**, walked from the owning `claude` pid up to the first app bundle (`terminal_app_of`, already written for click-to-focus) and memoized per session because the tooltip is rebuilt every 1 s poll. A detached background agent is named `background agent`, never by the `ClaudeCode` launcher the walk would find — decided the same way `focus_session` routes a click, so the tooltip and the click agree (UI Principle #4). App-side only: no hook change, no status-file change | Accepted |
| 062 | 2026-08-13 | **A light keeps the slot it arrived in** — reported live: the lights kept swapping places. The strip was re-sorted on every 1 s poll by `cwd`, then by session id, so a new session in a folder that already had lights landed at a position decided by its random uuid and pushed every later light along, and a session that `cd`'d changed groups mid-life. With a background agent per `/stop` and pre-warmed spares appearing and vanishing (#064, #065), the bar reshuffled constantly and a light moved out from under the pointer between seeing it and clicking it. Ordering is now **arrival**: a session takes the next free slot the first time the bar sees it and holds it until it is gone, when the gap closes; the first poll after a launch is laid out in the backend's deterministic (label, id) order. The settings segment becomes **Stable | Urgency** (a stored `window` reads as stable), so folder grouping is dropped — it never separated the common case of several sessions in one repo, and the folder is in the tooltip. Frontend-only: no hook change, no status-file change, no Rust change | Accepted |
| 063 | 2026-08-13 | **A background agent's light says what Claude Code says: green while it works, orange while it waits** — a `--bg` job that stops to ask a question fires `Stop`, not `PermissionRequest`, so the hook writes `idle` and #065's "never retire a `blocked` or `error` light" guard never matched the case it was written for: a waiting agent lost its light after five minutes and got it back only when the answer arrived. Claude Code's own job record carries `state` (`working`/`done`/`blocked`) next to the `status` (`busy`/`idle`) #065 already reads, so a bg light whose hook last wrote `idle` is reconciled from it — `status: busy` → green, `state: blocked` → orange — and a `blocked` job is never retired. Only an `idle` light is touched, so anything the hook actually observed still wins; `done` and stale `working` jobs still retire at five minutes | Accepted |
| 067 | 2026-08-13 | **An interrupted turn greys its own light** — "when I stop a session manually the light still shows up as green". Structural, not a one-path bug: `report.sh` maps hook *events* and has one route to `idle` (a `Stop`), and an interrupted turn fires none — so the light keeps the `running` its last tool event wrote, forever. Implements #059, closing its live check: sampling `~/.claude/sessions/<pid>.json` every 2 s caught a real Ctrl+C (`status` → `idle` **3.84 s after** the light's last hook event, light still green) and mapped the vocabulary, which matches the bar one for one — `busy`→green, `waiting`→orange, `idle`→gray, absent on **every Claude Desktop session**. Preferred over `claude agents --json`, which costs a subprocess (hence 10 s, not "immediately") and *loses* information — it called a `shell` session `busy` for ten minutes — while the per-pid file is already read every poll for #053's tooltip name. Greys only on positive evidence (`status == "idle"`) whose `statusUpdatedAt` falls in a **strictly later second** than the light, so the hook always wins a tie; also **clears `detail`**, or #014/#050 would render the result as the white "finished, unread" light on a session the user is already looking at. Background jobs excluded (their `idle` means something else — #063/#065); `shell` left alone as unconfirmed. No hook change, no schema change | Accepted |
| 068 | 2026-08-14 | **The hook becomes a native binary** (`agentstatus-hook`), replacing `report.sh` and removing `jq`. Forced by the Windows port: Claude Code runs hooks through Git Bash, where MSYS emulates `fork()`, so `report.sh`'s eight external spawns per event measured **210.6 ms** against the binary's **26.4 ms** (bash-spawn floor 18.6 ms) — ~421 ms vs ~53 ms of added latency per tool call, and Agent Guideline #3 forbids the former. Also closes #059's open `jq` defect at the root rather than with a guard: `jq` ships only on macOS 15+ and not at all on Windows, and its absence made the app silently inert. Ported against `report.sh` as the specification, with 42 recorded fixtures replayed through both implementations; output files, subagent markers, and `calibration.log` are byte-identical. Shipped Windows-first — macOS keeps `report.sh` until the port has proven itself on a platform with no users to regress | Accepted |
| 069 | 2026-08-14 | **Windows support, native only** (option B: floating bar, tray, lights, click-to-focus via the VS Code extension relay). `tauri-nspanel` moves to a macOS-only dependency and its plugin registration is `cfg`-gated — an NSPanel is macOS by construction, and Windows needs none of it (plain always-on-top + `skipTaskbar` + transparent). Cursor's menu-bar AX integration (#038/#045/#047) and per-tab terminal focus are **explicitly unsupported** on Windows: neither has a Windows equivalent, and a light that leads nowhere is worse than no feature (UI Principle #3). WSL is out of scope — its hooks write to the WSL filesystem, which a Windows GUI app cannot reach without `\\wsl$\` plumbing | Accepted |
| 070 | 2026-08-14 | **Windows click-to-focus is a direct Win32 raise**, not a port of the macOS two-path scheme. Per-tab focus already worked — the extension relay (#015/#019) is plain TypeScript — so only the *window* raise was missing. `EnumWindows` + `SetForegroundWindow`, matching the workspace-root basename plus the app name the editor puts at the end of its title; no permission grant (macOS's Accessibility prompt has no analogue) and no subprocess. macOS fires *both* its osascript raise and the IDE CLI because the CLI is the only thing that crosses Spaces (#021); Windows has no Spaces, so the CLI is a **fallback**, used only when no window matched, and via `Code.exe` directly rather than the `code.cmd` shim (`CreateProcess` cannot run a `.cmd`, and `cmd /c` would need nested quoting for paths with spaces). Cursor raises but never falls back to its CLI (#047 opened a new agent on macOS and has not been retested). `cli` is deliberately unreachable (#069, and the recorded pid is still 0). `workspace_root` is un-gated from macOS since the VS Code extension writes the same lock files on Windows, and path matching there is case-insensitive and accepts either separator — macOS keeps its exact, `/`-only rule. **Verified:** the raise finds a real window by title in 0.8 s (ignored Notepad test, runnable on a bare machine). **Unverified:** VS Code and Cursor title formats — neither is installed on the Windows test box | Accepted |
| 071 | 2026-08-14 | **A Windows terminal session is focused through its host process** (amends #069). Reported live: clicking the only light did nothing, because it was `ide:"cli"` and #069 had declared `cli` unreachable on Windows. That reasoning rested on `pid` being `0` — a measurement deferred in #068, not a platform limit. The process tree settled it: `claude.exe → powershell.exe → WindowsTerminal.exe`, and `claude.exe → claude.exe` for Claude Desktop, two hops in both cases. The hook now walks to the nearest `claude.exe` ancestor (its immediate parent is a Git Bash shell that exits with it, so recording that would give the app a dead pid), and the app walks up from the recorded pid to the first ancestor owning a visible titled window. Still the window, not the tab — Windows Terminal exposes no tab API, so #069's ceiling stands. Verified by an ignored test that resolves a live session's pid and raises its host window | Accepted |
| 072 | 2026-08-14 | **Tray mode on Windows** (extends #024, completes #069's option-B scope). #069 scoped tray mode in but the first pass gated all of it to macOS, leaving a **Mode** control that did nothing — and a frontend audit showed it was worse than inert: a light click in menu-bar mode calls `hidePopover()`, which on Windows hid the window with no tray icon, no taskbar button (`skipTaskbar`, verified against the shell's own UI tree) and no Dock icon, and the mode is persisted — so the app became unreachable except by killing it. Tray plumbing is now un-gated (`icon_as_template` stays macOS-only); the popover opens away from whichever screen edge the tray sits on; `set_mode` returns whether a tray actually exists and the frontend reverts to floating when it does not, so a failed tray can never strand the app; Windows always uses the single condensed dot because a notification-area icon is square 16x16 and the 170x44 strip would smear, with `square_icon` letterboxing what arrives; a new `platform()` command gives the frontend the platform detection it had none of; "Menu bar" is relabelled "Tray". Verified by driving the real UI: the icon appears in the notification area (named by a new tooltip), clicking it reveals the popover, and switching back restores the floating bar | Accepted |
| 073 | 2026-08-15 | **The tray popover dismisses itself, and anchors to the work area** — two defects in #072, both reported from live use. (a) The popover stayed on top after clicking away. The fix is to hide on `Focused(false)`, but the window is configured `focus: false` and `show()` does not activate it, so it **never held focus to lose** — the first attempt deployed and did nothing. `set_focus()` after `show()` is what makes dismissal possible; a 400 ms guard stops the focus loss from racing the tray click and making the icon unable to close it. (b) The popover opened overlapping the taskbar, because it anchored to the *click point* and the tray icon sits inside the taskbar. Now anchored to the monitor's **work area** (`GetMonitorInfoW`'s `rcWork`), which `Monitor::size()` cannot express. A third defect found while fixing them: opening settings grows the window ~360px and ran it off the bottom, so `fit_popover` pulls it back inside the work area. Windows only — macOS's panel is non-activating by design (#008). Verified by driving the real UI | Accepted |
| 074 | 2026-08-15 | **Opening the tray popover on Windows opens the settings panel with it.** Requested live. On Windows the tray item is a single summary dot (#072), so the popover was showing a larger copy of what the user had just clicked and hiding the panel they wanted behind another right-click; on macOS the menu-bar item already shows every dot, so it keeps #024's behaviour. The mechanism is the interesting part: `visibilitychange` is the natural signal and the codebase already used it for this moment, but **WebView2 keeps the document "visible" while the window is hidden**, so it never fires on Windows — the first implementation relied on it and did nothing. The backend now emits `popover-shown`. Opening via the normal `toggleSettings` inherits the upward growth, the lights anchor, and `fit_popover` (#073), so it lands 189x361 clear of the taskbar | Accepted |
| 075 | 2026-08-15 | **The settings panel collapses without the lights jumping.** Reported live, and the asymmetry (only on collapse) was the clue: `panel-above` is `column-reverse`, so the lights sit at the *bottom* of the open panel; collapsing mutates the DOM first, snapping them to the top of a still-tall window, and only three-plus frames later do the async resize and re-anchor put them back. DOM and window geometry cannot be made atomic, so the fix suppresses the paint instead — `visibility: hidden` across the transition, restored once the window is final. `visibility` rather than `display`/`opacity` because `resizeToContent` still has to measure the layout. The rAF wait is now bounded (250 ms) and the restore is in a `finally`, or a popover dismissed mid-collapse would leave the bar invisible for good. Also fixed here: #073's debounce stamped even when the popover was already hidden, so the next tray click was swallowed. Verified with a **negative control** — fix off: 360 px jump; fix on: 22 px | Accepted |
| 076 | 2026-08-15 | **macOS support drops to 13 (Ventura) and the DMG becomes universal.** The app never targeted macOS 15 — `Info.plist` carried Tauri's default `LSMinimumSystemVersion 10.13`. The 15 floor was one accidental dependency: `report.sh` needs a `jq` that ships at `/usr/bin/jq` only from macOS 15, so every DMG user on 13/14 installed a hook that silently no-opped forever (#059's finding, and the reason #068's port stopped at Windows). macOS now installs the same native `agentstatus-hook` binary Windows has shipped since 0.7.0, so the dependency is gone rather than guarded. 13 is the floor because **Claude Code itself requires macOS 13.0+**, and it is now declared, so an older Mac is refused at install instead of half-working. The DMG is universal because Claude Code supports `darwin-x64` and an arm64-only build excluded every Intel Mac — which also forces the *hook* to be universal, since a compiled hook is arch-specific where `report.sh` was not. Two macOS-only hazards a straight port would have shipped: `fs::copy` is `fcopyfile(COPYFILE_ALL)` and would carry `com.apple.quarantine` onto a binary Claude Code runs on every tool call, and writing over the destination fails `ETXTBSY` while a hook is executing it — so the install writes fresh bytes to a staged path and `rename`s over. **Unverified: everything macOS.** Written on Windows; needs a `cargo check`, `gen-golden.sh`, and a live old-vs-new session diff on a Mac before release | Proposed |
| 077 | 2026-08-15 | **Several Windows terminal windows are told apart by the session title.** Reported live: with three terminals open, a click brought *a* terminal forward, not the right one. #071 measured one terminal and assumed one window per host process — but Windows Terminal runs every window of an instance in **one** `WindowsTerminal.exe`, so all three chains converged on one pid owning three windows, and the walk kept whichever `EnumWindows` handed it first (z-order). The choice is now made by the title Claude Code writes into the title bar, reusing the Ghostty rule (#055/#066): ends-with is the session's window, contains must be unique. When it cannot tell — an untitled session, or one in a background tab whose window shows another title — the click does nothing rather than raise the wrong terminal (UI Principle #4), a deliberate change from #071. Tab selection remains out of reach (#069). Verified live on the three-window machine, choice asserted separately from the raise because `SetForegroundWindow` cannot succeed from a test binary | Accepted |
| 078 | 2026-08-15 | **A plan-mode approval turns the light orange.** Reported live as "Windows isn't picking up on orange", with the light showing **green** while waiting. Instrumenting the chain exonerated the Windows port — `AskUserQuestion` and Bash permission prompts already held `blocked` for 43 s / 32 s and rendered orange on screen. The defect is `ExitPlanMode`: its `PermissionRequest` fires when the user *answers*, so `PreToolUse` is the only event that lands while the prompt is up and the light stayed green for the whole 48 s wait, turning orange for under 150 ms at the end. The approved first fix — extend #067's reconcile on Claude Code's own `status: "waiting"` — was **falsified by the same measurement** (status read `busy` throughout the plan prompt) and dropped rather than shipped as dead weight. Instead `ExitPlanMode`/`EnterPlanMode` rewrite their `PreToolUse` to `PermissionRequest` in both hook implementations, so state and detail stay in agreement and `PostToolUse` greens the light on approval. `AskUserQuestion` left out as redundant; `EnterPlanMode` included at the user's choice, accepting a sub-second orange flicker when auto-approved as cheaper than 48 s of wrong-green (UI Principle #2). Golden regeneration is +4 lines / 0 changed, proving the refactor touched nothing else. Verified live: 31.9 s of `blocked` across a real approval, against 48 s of green before | Accepted |
| 079 | 2026-08-15 | **The release publishes with `find`, not a `dist/*` glob.** The first `v0.8.0` tag built both platforms and then published nothing: `read dist/msi: is a directory`. `actions/upload-artifact` preserves the structure below the **common ancestor of the paths it is given**, and the two jobs give it different numbers of paths — macOS one (its DMG lands flat), Windows two (`msi/` and `nsis/` arrive as directories) — so `dist/*` handed `gh` two files and two directories. Invisible until now because #069 checked the globs against the *local* build output; the asymmetry is created by `upload-artifact`, not by the build. Nothing broken reached users only because `gh release create` deletes the release it just made when an asset upload fails — `v0.7.1` stayed Latest. Now `find dist -type f`, plus a count guard that fails before publishing rather than shipping a release quietly missing an installer (the same class as #041's version guard). Flattening the Windows upload was rejected: it fixes this shape and leaves the next one to another failed tag | Accepted |
| 080 | 2026-08-18 | **A light can be closed by hand.** Every prune the bar had was evidence-based — a closed window (#027), a dead pid (#054), an archived composer (#048), a retired background agent (#065), the 2 h backstop (#004) — so a session the *user* knows is finished can keep its light until the evidence arrives. Right-click now reveals, alongside the lights, one small red × per light; clicking it invokes `dismiss_session`, which deletes that session's status file and subagent markers exactly as a prune does. A deletion, not a hide: a session that is genuinely alive re-registers on its next hook event and its light returns (UI Principle #4 — the bar must not withhold a light for a live session any more than it may show one for a dead session). A local tombstone hides the light on the click so a poll already in flight cannot paint it back, and is lifted the moment the poll agrees the session is gone — or after 5 s if the delete never took. The buttons ride with the settings panel rather than sitting on the bar: a close button always visible next to a click-to-focus light is a misclick that deletes what you meant to open (UI Principle #1/#3). Their red is a fixed constant, not `--c-error`, so recoloring the error state cannot turn them blue nor make a row of controls read as five sessions in error | Accepted |
| 081 | 2026-08-18 | **The bar stops reading Cursor's menu while it has nothing to decide** — reported live: "I can't click into the Cursor menu bar item, it keeps clicking off of it". #052's `", Running"` veto reads Cursor's status-item menu through the AX tree, and that walk **cancels the menu when the user has it open** — the same interference #038 found on the shallower pip read and answered with a deliberately gentle 20 s cadence. The veto sat at the end of a guard whose other conditions (`terminal && stale && !subs_live`) stay true forever on a **settled** Cursor light, so with idle Cursor lights on the bar the walk ran every `CURSOR_FACTS_TTL` (5 s) for as long as those lights existed — while its answer could only ever assign `idle` to a light already idle. The guard now tests `state != "idle"` **before** the walk, so the AX read happens only on a poll that could actually change a light. Measured: 45 s with two idle Cursor lights → 0 walks (was one every 5 s); one silent running light → exactly one walk, then none. Behaviour-identical by construction (Guideline #7): the arm it gates assigns the value the light already holds | Accepted |
| 082 | 2026-08-18 | **Settings become a window; the bar keeps only lights and a right-click controls row** — "the settings are starting to be a bit much" on a strip whose whole job is to be glanceable (UI Principle #1). The 16-control panel that grew out of the bar is now a normal, decorated window with a sidebar (General / Lights / Colors / Audio / About) that follows the system appearance, built lazily by a new `open_settings` command under its own capability. A right-click on the bar no longer opens anything: it reveals the bar's controls — decision 080's one × per light, plus a **settings gear** one slot past them, drawn as an inline SVG because U+2699 renders as a color emoji at dot size. Preferences move to a shared `prefs.js`; the **bar stays the source of truth** (it answers `prefs-request` with a snapshot and owns `applyPref`), so nothing depends on two webviews sharing localStorage. New pref: **Cursor pip Show/Hide** (default show) — hiding it also stops the Accessibility read behind it, removing the last reader that can cancel Cursor's menu (#081). Three follow-ons fall out: the pill's radius becomes `--pill-radius` (half the light strip's thickness) so the two-row bar is a stadium instead of an oval; #075's paint suppression now covers **both** toggle directions, because the controls row lands on the lights' own side and shifts them either way; and #074's Windows "popover opens with the settings panel" is dropped with the panel it opened | Accepted |
| 083 | 2026-08-18 | **The pip's click clears Cursor's notifications itself.** Reported live: clicking the pip never retired the Cursor menu-bar notification, so the pip kept coming back. #045's premise — press the `"• <name>"` row and Cursor both opens the composer and marks it read, verified on 3.12.10 — is false on **3.15.6**: that row's click sends only `vscode:openComposer`, whose handler opens the agent and touches neither half of a bullet (`hasUnreadMessages || badgeCount > 0`). Only `"Clear All Notifications"` sends `vscode:clearAllNotifications` → `markAgentRead` + `clearAllBadges`. In **Glass** mode (this user's Cursor) the per-composer badge-clear-on-focus listener is never registered at all, so a bullet survives opening the composer for up to its 1 h auto-clear. Measured: pressing the composer row leaves the count at 2 (with or without activating Cursor), pressing the clear-all row takes it 2 → 0 — so `AXPress` works and it is the row that changed. The click now presses both rows, navigating first and clearing second. Cursor exposes no per-composer clear, so this necessarily clears the other waiting composers' bullets — accepted, since on Glass they would sit for an hour anyway and their sessions keep their own lights. End-to-end press against a live notification still unverified (Cursor quit before one appeared) | Accepted |
| 084 | 2026-08-18 | **An orange background-agent light has to be a question.** Reported live: "why is there an orange light in my lightbar from a session that already finished?" #063 maps Claude Code's `state: "blocked"` for a `--bg` job straight to orange, on the premise that `blocked` means the job stopped to ask something. It has a second meaning: a job that finished its turn and sits at an empty prompt reports the same `blocked`. Both were measured on 2.1.234 — the difference is the `needs` in the job's own `~/.claude/jobs/<id>/state.json`, which is the question verbatim when one was asked (`"Should the fallback be red or blue?"`) and the literal `"send a prompt to start"` when nothing is being asked at all. `claude agents --json` carries the job's `tempo` as `state` but not the `needs` behind it, so the bar now reads it from the job record. `needs == "send a prompt to start"` no longer paints orange (the light stays as the hook wrote it) and no longer blocks #065's five-minute retirement — which had left the light orange for the full two-hour backstop, since #065's "never retire a blocked light" guard applied to it too. An exact-string test, deliberately: an unrecognised or absent `needs` keeps #063's orange, because a missed attention light costs more than one that lingers (UI Principle #2 over #4) | Accepted |

---

## Decisions

## 001 — Status signal source: Claude Code hooks

**Date:** 2026-07-05
**Status:** Accepted

**Context:** The tool needs to know, in near-real-time, the live state of each Claude Code
session (running / blocked / idle / error). We evaluated how to obtain that state from the
outside.

**Options considered:**
| Option | Pros | Cons |
|---|---|---|
| Claude Code **hooks** | Official, event-driven, push-based; payload includes `session_id`, `cwd`, `transcript_path`; fires exactly on the lifecycle transitions we care about | Event names/payloads are version-dependent and must be verified |
| Poll session **transcript `.jsonl`** files under `~/.claude/projects/` | Always present; no config needed | Internal, unstable format; polling lag; inferring "blocked vs running" from transcript tails is fragile |
| A **public API / IPC / MCP** into a running session | Would be cleanest if it existed | No such external status interface is exposed |

**Decision:** Use Claude Code hooks as the primary signal source. Transcripts may be used
only for a last-activity timestamp if needed, never as the state source.

**Reasoning:** Hooks are the only push-based, officially-supported signal that maps
directly onto the four states, and their payload already carries the fields we need to
identify a session. The transcript format is explicitly internal and unstable; building
state inference on it would be brittle. See Agent Guideline #4 — the exact event set is
version-dependent and must be confirmed against the installed version (build Milestone 1).

## 002 — Signal transport: shared JSON status file

**Date:** 2026-07-05
**Status:** Accepted

**Context:** Hooks (short-lived shell commands) need to communicate each session's current
state to the long-lived display app.

**Options considered:**
| Option | Pros | Cons |
|---|---|---|
| Hooks write a **shared JSON file** (`~/.claude/status/sessions.json`), app watches it | Dead simple; no server; survives app restarts; state persists between runs; hooks stay trivial and fast | Needs atomic writes to avoid torn reads; slightly less instant than a socket |
| Display app runs a **local HTTP/socket server**; hooks POST events | Real-time; clean push | App must always be running or events are lost; hooks now depend on the app being up (violates "hooks must never block/fail" if the app is down) |

**Decision:** Hooks write session state to a shared JSON file keyed by `session_id`; the
Tauri app watches the file for changes.

**Reasoning:** A file decouples the hook from the app's liveness — a core requirement, since
hooks run inside the user's real sessions and must never block or fail if the display app
isn't running (Agent Guideline #3). It also gives us free persistence across app restarts.
Torn reads are handled with atomic write-and-rename in the hook. A server buys real-time
delivery we don't need for a human-glanceable light and adds a failure mode we explicitly
want to avoid.

## 003 — Display layer: Tauri borderless always-on-top window

**Date:** 2026-07-05
**Status:** Accepted

**Context:** The user wants a small, freely-positionable, always-on-top bar of lights. This
rules out rendering inside VS Code (its extension surfaces — status bar, docked panels —
can't float over the screen). The display is therefore a standalone app.

**Options considered:**
| Option | Pros | Cons |
|---|---|---|
| **Tauri** (Rust shell + web UI) | Tiny (~3–5MB); true borderless always-on-top + drag; web UI makes lights trivial to style/animate; cross-platform later | Rust toolchain setup |
| **Native SwiftUI** (`NSPanel`) | Most Mac-native and lightest; best always-on-top/menu-bar behavior | Swift-only; macOS-only; more UI code |
| **Electron** | Easiest if web-first; huge ecosystem | ~150MB + heavy RAM — overkill for a light bar |
| **VS Code status bar** | No separate app | Not floating/positionable, in-editor only — fails the core ask |
| **Menu bar app** (SwiftBar/xbar) | Zero UI code | Not a repositionable bar placed anywhere on screen |

**Decision:** Build the display as a Tauri app: a borderless, always-on-top, drag-to-position
window that watches the status file and renders one colored light per session.

**Reasoning:** Tauri is the only option that hits the exact form factor (floating,
positionable, tiny) while keeping the UI easy to build and style in web tech, and leaves the
door open to non-macOS later. SwiftUI is a strong lighter-weight alternative but macOS-only
and more UI code; Electron is too heavy; the in-editor options don't meet the "position it
anywhere" requirement.

## 004 — Session identity: `session_id` key, folder label, heartbeat staleness

**Date:** 2026-07-05
**Status:** Accepted

**Context:** Each light must correspond to one session, be labeled so the user can tell
sessions apart, and disappear when a session ends — including unclean deaths (VS Code
force-quit) where no shutdown event fires.

**Options considered / findings:**
- No Claude Code or VS Code API maps a session to a specific **editor tab**, so a light
  cannot be tied to a literal tab. `session_id` is 1:1 with a session and is stable across
  all of that session's hook events, so it is the natural key.
- Multiple sessions can share a `cwd`; the folder name alone doesn't disambiguate. Label
  by project folder (plus a short session title if available) and rely on `session_id` as
  the unique key.
- A clean `SessionEnd` event can't be relied on for shutdown; force-quit skips it.

**Decision:** Key each light by `session_id`. Label it by the `cwd`'s project folder name
(plus session title when available). Write a heartbeat timestamp on every hook, and have
the display treat a session with no update for N minutes as stale — dimming or removing its
light rather than leaving a live-colored light on a dead session.

**Reasoning:** `session_id` is the only stable, unique per-session identifier available
from outside. A heartbeat is required because we cannot depend on a shutdown event, and a
stale/lying light is worse than no light (UI Design Principle #4).

## 005 — Install model: global hooks via self-installing app + VS Code extension

**Date:** 2026-07-05
**Status:** Accepted

**Context:** The user wants installation to be as close to one click as possible, framed as
"set it up in another project or VS Code window with one click." Key finding while building
the Milestone 1 logger: hooks registered in the user's **global** `~/.claude/settings.json`
take effect **immediately, in already-running sessions**, across every project and VS Code
window. So status tracking is inherently machine-wide — there is nothing to install
per-project or per-window. The real question is how to package the single, once-per-machine
install.

**Options considered:**
| Option | The "one click" | Pros | Cons |
|---|---|---|---|
| Self-installing app | Open the app once (or auto-launch at login) | App writes the global hooks + creates the status file on first run; genuinely one action; works everywhere by construction | Install logic lives in the app |
| VS Code extension | Install from the marketplace | Familiar channel; auto-starts with VS Code; enables precise click-to-focus of a specific tab | Can't *be* the floating bar; second codebase; redundant per-window since hooks are global |
| Both | Either | Marketplace reach + tight integration **and** the floating bar | Most work; two artifacts to maintain |
| Script only | `curl \| bash` / one command | Trivial now | Not literally one-click; defers packaging |

**Decision:** Build toward **both**: a self-installing Tauri app as the core floating bar
(installs global hooks on first launch, optional launch-at-login), plus an optional VS Code
extension that offers marketplace install and click-to-focus a specific session's tab. A
one-command script is the interim installer until the app hosts the install logic.

**Reasoning:** Because hooks are global, a single install already covers every project and
window — so the app model delivers the "works everywhere" goal by construction, and opening
the app is the one click. The extension adds a familiar distribution channel and the one
integration the app can't do from outside (focusing a specific VS Code tab). The interim
script keeps us unblocked before packaging exists (Milestone 5). Both share the same hook
installer and status-file contract, so the second artifact is mostly a distribution shell,
not a parallel implementation.

## 006 — Verified event→state mapping (Milestone 1)

**Date:** 2026-07-05
**Status:** Accepted
**Evidence:** Claude Code **2.1.201** (VS Code extension), observed live via the temporary
broad logger across the AgentStatus + ApplicationBot windows (`logs/events.log`).

**Context:** Decision 001 planned the event→state mapping from docs, flagged as
version-dependent and requiring confirmation (Agent Guideline #4). Milestone 1 ran a logger
on real sessions to confirm which events actually fire and with what payload.

**Verified event → state mapping:**
| State | Light | Fires on (confirmed) | Confidence |
|---|---|---|---|
| running | 🟢 | `UserPromptSubmit`, `PreToolUse`, `PostToolUse` | High — observed |
| blocked | 🟠 | `PermissionRequest` | High — observed |
| idle | ⚪ | `Stop`, `SessionStart` | High — observed |
| error | 🔴 | `PostToolUseFailure` (interim) | **Low** — noisy; see below |
| (remove) | — | `SessionEnd` | High — observed |

**Payload facts (this version):**
- Common fields on every event: `session_id`, `cwd`, `hook_event_name`. `transcript_path`,
  `prompt_id`, `permission_mode` appear on most but **not all** (`Stop` carries only
  `session_id` + `cwd`). So key on `session_id`; derive the window from `cwd`.
- `SessionStart` has `source` (startup/resume/clear/compact); `SessionEnd` has `reason`.
- `PermissionRequest` carries `tool_name` — it fires for **both** tool-permission prompts
  **and** `AskUserQuestion` (tool_name = "AskUserQuestion"), even in `bypassPermissions`
  mode. So "blocked" = "Claude needs the user," and `tool_name` distinguishes a question
  from a tool approval. There is **no** "permission resolved" event — the bar infers
  unblocked from the next event (`PreToolUse`/`Stop`).
- `PostToolUseFailure` carries `error`, `is_interrupt`, `duration_ms`, `tool_response`.

**Corrections to decision 001's assumptions:**
1. **Blocked is `PermissionRequest`, not `Notification`.** `Notification` never fired;
   `Elicitation`, `StopFailure`, `PermissionDenied`, `SubagentStart/Stop`, `Pre/PostCompact`
   also did not fire in this run.
2. **Error is the soft spot.** The only failure event seen was `PostToolUseFailure`, and the
   one captured was an incidental shell non-zero exit that the turn recovered from — i.e.
   tool failures are noisy and do **not** imply a session error. A clean red wants a
   turn-level `StopFailure` (documented but not yet observed). *Interim:* treat
   `PostToolUseFailure` with `is_interrupt == false` as red, and refine once a real
   `StopFailure` is observed. → tracked as a Milestone 2 calibration item.

**Session-granularity findings:**
- No `parent_session_id` / `agent_id` / `agent_type` in any payload, and `SubagentStart/Stop`
  did not fire. Hooks give **session-level** granularity (one light per `session_id`) — which
  is what the bar wants; subagents don't spawn their own lights.
- A single window can spawn **many short-lived sessions**, each with a full
  `SessionStart → UserPromptSubmit → Stop → SessionEnd` lifecycle (observed: ApplicationBot
  ran ~12). The bar must add/remove lights on `SessionStart`/`SessionEnd` and still rely on
  the heartbeat (decision 004) for unclean deaths.

**Window scoping (feature research):**
- `~/.claude/ide/<port>.lock` = one file per open VS Code window, each listing
  `workspaceFolders`. A session's `cwd` matches its window's workspace folder → **the
  workspace folder is the window key**, derivable from the `cwd` we already store.
- `VSCODE_PID` / `VSCODE_IPC_HOOK` are the **shared** main VS Code process (identical across
  windows) — not usable as a per-window key. No per-window port is exposed in the session
  env. Limitation: two windows on the *same* folder collapse into one group.
- The floating bar has no innate "current window," so automatic "this window only" is the
  **extension's** job (it knows its own workspace); the bar does "all windows" for free.

**Decision:** Adopt the verified mapping above as the signal contract for Milestone 2, with
the error signal marked interim pending a real `StopFailure` observation. Scope windows by
workspace folder (from `cwd`, cross-checked against the IDE lock files); deliver automatic
"this window" via the extension (decision 005), all-windows in the bar by default.

## 007 — Status store: one file per session (not a single shared JSON)

**Date:** 2026-07-05
**Status:** Accepted (refines #002)

**Context:** Decision 002 chose "a shared JSON status file" as the transport. Implementing
the Milestone 2 hook surfaced a concurrency problem #002 glossed over: Milestone 1 showed
**many sessions fire hooks concurrently** (ApplicationBot alone ran ~12). A single shared
`sessions.json` forces every hook into a read-modify-write on the same file → lost updates
and torn reads. Safe concurrent writes would need a mutex, and macOS ships no `flock`.

**Options considered:**
| Option | Pros | Cons |
|---|---|---|
| Single shared `sessions.json` | One file to read; one watch target | Concurrent read-modify-write races; needs a lock; no `flock` on macOS; awkward from a fail-silent shell hook |
| **One file per session** `sessions/<id>.json` | Each hook writes only its own file → zero cross-session contention; `SessionEnd` = delete the file; stale cleanup = delete old files; atomic per-file write via temp+rename | App reads N small files instead of 1 (trivial); no single-read snapshot (fine for a status light) |

**Decision:** Store one JSON file per session at `~/.claude/status/sessions/<session_id>.json`
(overridable via `$AGENTSTATUS_DIR`). Each hook writes only its own session's file
(temp-file + atomic rename); `SessionEnd` deletes it. The display app watches the `sessions/`
directory. Same object shape as #002 (`state`, `cwd`, `label`, `updated_at`).

**Reasoning:** The per-session layout removes the multi-writer race by construction — the
one hard problem with a shared file — while keeping every property #002 wanted (files the
app watches, persistence across restarts) and making removal and stale-cleanup trivial. The
only cost is reading a directory instead of one file, which is immaterial. This keeps the
hook a fast, lock-free, fail-silent write, satisfying Agent Guideline #3.

**Validation:** `report.sh` unit-tested across all branches (running/blocked/idle/error,
interrupt-skip, `SessionEnd` removal, missing-`session_id` safety), replayed against the 160
real M1 events (ended sessions correctly removed, live ones retained with correct state), and
installed live — confirmed real-time population of `~/.claude/status/sessions/` from running
sessions. Error signal still interim: `report.sh` mirrors `PostToolUseFailure`/`StopFailure`
to `~/.claude/status/calibration.log` (event/session/tool only, no `tool_input`) to confirm
the real red trigger from live data.

## 008 — macOS overlay: non-activating NSPanel + Accessory app

**Date:** 2026-07-05
**Status:** Accepted
**Stack note:** Rust toolchain installed via rustup (1.96, minimal profile); app scaffolded
with `create-tauri-app` (Tauri v2, vanilla template, static frontend served from `../src`,
`withGlobalTauri`). Transparency requires `app.macOSPrivateApi: true`.

**Context:** The bar must be a small, borderless, transparent, always-on-top, drag-to-position
window that stays visible over **everything** — including when the user switches to another
app's **full-screen** space (the primary use case: coding in full-screen VS Code). Getting
that behavior on macOS took several escalating attempts, each ruled out by live testing:

| Attempt | Result |
|---|---|
| tao `set_always_on_top(true)` (config `alwaysOnTop`) — floating window level | Gets covered when another window is focused |
| Native `NSWindow.setLevel(25)` + `collectionBehavior(CanJoinAllSpaces \| FullScreenAuxiliary \| Stationary)` via objc2 | Stays on top **within** a Space, but not over another app's full-screen space |
| + `ActivationPolicy::Accessory` (no Dock icon, not space-managed) | Still not over full-screen |
| **Non-activating NSPanel** (`tauri-nspanel`) + Accessory | **Works** — floats over third-party full-screen without stealing focus or switching Spaces |

**Decision:** Convert the main window into a **non-activating NSPanel** using the
`tauri-nspanel` crate (git, `v2` branch) and run the app as an **Accessory** app. Panel
config (from the crate's `fullscreen` example): level `4` (NSFloatingWindowLevel), style mask
`NSWindowStyleMaskNonActivatingPanel` (`1<<7`), collection behavior
`FullScreenAuxiliary | CanJoinAllSpaces`. Also: window auto-sizes to its content (measured
**after** paint via double-rAF, clamped to a minimum, so it never shrinks to 0 and blocks
clicks behind it — a bug that made it invisible at first); position is remembered via
`tauri-plugin-window-state`.

**Reasoning:** macOS only lets a **non-activating panel** sit over another application's
full-screen window; a normal window (any level / collection behavior) cannot. The Accessory
policy removes the Dock icon (correct for a status-bar utility) and stops the window from
being space-managed. `tauri-nspanel` does the NSWindow→NSPanel conversion cleanly and
resolved against our Tauri 2.11 with no version conflict, so it beat hand-rolling the class
swap in objc2. Superseded the objc2 `NSWindow` approach that only worked within a Space.

## 009 — Hover detail (task + activity) and subagent count badge

**Date:** 2026-07-05
**Status:** Accepted

**Context:** Two Milestone-4 features: (a) hovering a light should show what the agent is
working on; (b) surface subagents. Both hinge on what the hooks expose.

**What the hooks give us (verified live):**
- `UserPromptSubmit.prompt` = the turn's task; `PreToolUse.tool_name`/`tool_input` = current
  activity; `Stop.last_assistant_message` = the wrap-up. `report.sh` distills these into two
  fields per session — `task` (carried across the turn via a read-merge of the prior file)
  and `detail` (current activity) — storing only short truncated summaries, never full
  `tool_input`.
- **Subagents:** `SubagentStart`/`SubagentStop` fire and carry `agent_id` + `agent_type`
  under the **parent's** `session_id`. Crucially, a subagent's own tool calls fire
  `PreToolUse`/`PostToolUse` with the parent `session_id` and **no `agent_id`** — so an
  individual tool call cannot be attributed to a specific subagent.

**Decision:**
- Tooltip = native OS `title` (multi-line): `name — state`, `↳ task`, subagent summary,
  `detail`. Native tooltip chosen because the window is sized to hug the pill, so a custom
  in-window tooltip would be clipped; the OS tooltip renders outside the window bounds.
- Subagents tracked as a `{agent_id: agent_type}` map in the session file — added on
  `SubagentStart`, removed on `SubagentStop`, cleared when the turn goes idle. Shown as a
  **count badge** on the light (chosen over mini sub-dots / hover-only); hover lists the
  types grouped with counts.
- Because subagent tool calls aren't attributable, we track **lifecycle only** (which types
  are running, how many, and each one's final message) — **not** each subagent's live tool.

**Frontend note:** dots are reconciled in place (keyed by id), not rebuilt each poll, so
updating a title/badge never dismisses an open hover tooltip. Window resize adds a small pad
so the corner badge isn't clipped.

**Validation:** `report.sh` unit-tested (task capture + carry-forward; subagent add/remove/
clear). Live: a concurrent poller confirmed a real subagent appeared in the parent's status
file for its full run and cleared on completion.

## 010 — Subagent storage: one marker file per subagent

**Date:** 2026-07-05
**Status:** Accepted (refines #009, applies #007 one level deeper)

**Context:** #009 first stored subagents as a `{agent_id: agent_type}` map inside the
session JSON, updated by a read-modify-write in the hook. Live testing with **two parallel
subagents** exposed a race: they share the parent's `session_id`, so their `SubagentStart`
hooks (and the flood of `PreToolUse` events from their tool calls, which also rewrite the
session file) do concurrent read-modify-writes on the *same* file and clobber each other —
only 1 of 2 subagents registered. Per-session files (#007) removed cross-session contention
but not *within*-session contention.

**Decision:** Store each subagent as its own marker file:
`sessions/<session_id>.subagents/<agent_id>` with the `agent_type` as contents. `SubagentStart`
creates the file, `SubagentStop` removes it, `Stop`/`SessionStart`/`SessionEnd` clear the dir.
The session JSON no longer carries subagents; the app reads the marker dir. Subagent events
are handled in a dedicated early branch of the hook — they never touch the session JSON.

**Reasoning:** One file per subagent means concurrent starts/stops operate on **different**
files, so there is no shared mutable state to race — the same insight as #007, one level
down. Verified: 8 concurrent `SubagentStart` for one session produced 8 markers (0 lost);
3 concurrent `SubagentStop` left exactly the right 5.

## 011 — Packaging: self-installing app (delivers decision 005)

**Date:** 2026-07-05
**Status:** Accepted

**Context:** Decision 005 chose a self-installing app so install is one action and covers
every project/window. Milestone 5 delivers it: a real `AgentStatus.app` (built with
`tauri build`) that wires up its own hooks with no node/repo dependency.

**Decision & mechanism:**
- The hook script is **embedded** into the binary at compile time
  (`include_str!("../../../hooks/report.sh")`) so the `.app` is self-contained and the
  installed hook always matches the shipped version.
- On launch (`app/src-tauri/src/install.rs::ensure_installed`, **release builds only** —
  `#[cfg(not(debug_assertions))]`), the app writes the script to a stable, app-independent
  path `~/.claude/status/report.sh`, creates `sessions/`, and merges the 11 hook entries into
  `~/.claude/settings.json` — idempotent (dedup by the `report.sh` marker), reversible
  (one-time `settings.json.agentstatus-bak`), non-clobbering (leaves other settings/hooks).
  This is the `setup.mjs` logic ported to Rust (serde_json).
- **Dev vs release split:** dev builds do **not** self-install, so `hooks/report.sh` edits
  stay live without a rebuild (dev registers the repo path via `node hooks/setup.mjs`); the
  release app owns the `~/.claude/status/report.sh` copy.
- Ships `.app` + `.dmg`; `install.sh` builds from source and copies to `/Applications`.
  Accessory app (no Dock icon); **launch-at-login is a documented manual step** (Login Items)
  — a `tauri-plugin-autostart` toggle is a future enhancement, not in v1.

**Trade-off — code signing:** the app is unsigned (ad-hoc). Locally-built copies run without
friction (no quarantine attribute), but a **downloaded/redistributed** copy hits Gatekeeper
and needs a one-time right-click → Open. Acceptable for personal use; real signing/notarizing
is out of scope for v1.

**Validation:** built the bundle; installed to `/Applications`; launched it; confirmed it
rewrote the hook path from the repo to `~/.claude/status/report.sh`, registered all 11 events
with exactly one entry each (dedup worked), wrote the executable script, backed up settings,
and preserved `permissions`/`theme`.

## 012 — VS Code extension: per-window status bar + native focus command

**Date:** 2026-07-05
**Status:** Accepted (delivers decisions 005, 006)

**Context:** Milestone 6 — the extension does what the external app can't: show only
**this window's** sessions (it knows its own workspace) and focus a specific session's tab.

**Decision & mechanism (`extension/`):**
- **Display:** status-bar items (chosen over a sidebar tree / installer-only). One item per
  session whose `cwd` is within `vscode.workspace.workspaceFolders` — the auto "this window"
  scoping of decision 006. Reads the same `~/.claude/status/sessions/` files (+ subagent
  marker dirs), polled every 1.5s. Item = a `$(circle-filled)` colored by state
  (`charts.green/yellow/red`, default for idle) with `×N` for subagents; tooltip (Markdown)
  shows name/state/task/subagents/activity.
- **Click-to-focus:** call **`claude-vscode.editor.open <sessionId>`** directly — this is the
  command Claude Code's own `vscode://…/open?session=` handler calls internally (found by
  reading the extension's registration). Calling it directly focuses the session's tab with
  **no URI-consent prompt** (the prompt was why routing through the deep link was rejected).
  Falls back to the deep link if the command is unavailable.
- **Hook install:** the extension bundles `report.sh` and ensures the hooks — but **only if
  not already present** (guarded), so multiple windows activating at once don't race on
  `settings.json`. In practice the app already installed them, so it no-ops.
- Packaged as a `.vsix` (`@vscode/vsce`); installed via the `code` CLI. **Marketplace publish
  is deferred** — it needs a verified publisher account (a distribution step, not a build).

**Validation:** installed the `.vsix`, reloaded a non-primary window, confirmed the status-bar
item appears for that window's session with correct color/hover, and that clicking focuses the
session's tab with no consent prompt. (Testing requires reloading a window, which restarts the
extension host — never the window running the active session.)

## 013 — Red = StopFailure only; AGENTSTATUS_IGNORE opt-out

**Date:** 2026-07-06
**Status:** Accepted (refines #006 error signal; adds an opt-out)

**Context:** Two issues raised in use. (a) The interim red trigger (`PostToolUseFailure`) was
suspected noise. (b) Another window's Claude activity was appearing as a swarm of fleeting
new lights.

**Findings (captured live):**
- **Error signal:** deliberately failing a Bash command (exit 7) and a subagent's command
  (exit 42) produced **only `PostToolUseFailure` (`is_interrupt:false`)** — `StopFailure`
  fired **0 times**. So `StopFailure` is reserved for genuine turn/API failures (rate-limit,
  overload, etc.), which can't be forced. Treating every `PostToolUseFailure` as red would
  flash the light on every recovered tool failure — misleading.
- **The "new sessions":** the offending sessions were **ApplicationBot calling Claude Code
  programmatically** (its question-classification feature — first prompt: *"Map a
  job-application question to ONE of these known answer types…"*). Each is a real top-level
  `claude-vscode` session (`isSidechain:false`, `parentUuid:null`, top-level transcript) —
  **indistinguishable** from an interactive tab by any hook field or env var
  (`CLAUDE_CODE_ENTRYPOINT` and `CLAUDE_CODE_CHILD_SESSION` are identical for both). The only
  reliable discriminator is an explicit marker the spawner sets.

**Decision:**
- **Red = `StopFailure` only.** `PostToolUseFailure` no longer flips state; it's still written
  to `~/.claude/status/calibration.log` for observation. `StopFailure` sets `error` and a
  `detail` of "⚠ turn failed — <error_type>", which persists until the next prompt.
- **`AGENTSTATUS_IGNORE` opt-out.** If that env var is set in a session's environment,
  `report.sh` exits immediately and never tracks it. Programmatic/headless spawners (e.g.
  ApplicationBot) set `AGENTSTATUS_IGNORE=1` when invoking `claude`. Chosen over a grace
  period (auto but fuzzy: delays real sessions, still shows long programmatic ones) because
  the spawner controls its own env and this is 100% reliable with zero false positives.

**Propagation:** the hook lives in four places — repo `hooks/report.sh`, the live
`~/.claude/status/report.sh`, the app's embedded copy (`include_str!`), and the extension's
bundled copy. All four were updated (live `cp`, app rebuilt + reinstalled + relaunched,
extension repackaged) so a future app launch or fresh install can't revert the change.

**Validation:** unit-tested — `PostToolUseFailure` keeps state `running` (calibration-logged),
`StopFailure` → `error`, `AGENTSTATUS_IGNORE=1` writes no file. Red rendering confirmed via an
injected `state:error` session (the user saw the red light).

**Known minor follow-up:** the light's label is the session's `cwd` basename, so `cd`-ing into
a subfolder relabels it (e.g. "app" instead of "AgentStatus"). Could map `cwd`→workspace root
via the IDE lock files; deferred.

## 014 — Split idle into "done" (unreviewed) vs "idle" (reviewed)

**Date:** 2026-07-06
**Status:** Accepted (refines the #006 idle state; UI-only, no hook/schema change)

**Context:** A single gray idle light couldn't distinguish a session that **just finished a
turn and whose output nobody has looked at yet** (needs a glance) from one whose output was
**already reviewed** (dormant). The user wanted the former to draw attention and the latter to
recede — the whole point of the bar is surfacing what needs the user.

**Key constraint:** Claude Code hooks fire on *lifecycle* events. There is **no hook for "the
user read the output"** — reading is a passive VS Code UI action that never touches the
session lifecycle. So "reviewed" cannot come from the signal layer; it must be inferred in the
display layer from an explicit acknowledgment the app itself can observe.

**Options considered:**
| Question | Options | Choice |
|---|---|---|
| What counts as "reviewed"? | (a) user clicks the light — which already focuses the session; (b) auto-dim after a timeout; (c) next `UserPromptSubmit` | **(a) click only.** A timeout was rejected: the user explicitly does not want the attention signal to disappear on its own. (c) is redundant — a new prompt flips the light to green anyway. |
| Where does the `reviewed` flag live? | (a) app-local (display layer); (b) hook-written status file | **(a) app-local.** Keeps the hook a dumb, fast, fail-silent write (Guideline #3); "reviewed" is a per-display UI concern, not session state. |
| How to tell a *finished turn* from a *fresh idle*? | (a) `idle` + non-empty `detail`; (b) new hook field | **(a).** `Stop` writes `detail` = the wrap-up message; `SessionStart` forces `detail=""`. So `idle && detail` reliably means "a turn ended, there's output to review" — no hook change needed. |

**Decision & mechanism (bar frontend, `app/src/`):**
- New render state **`done`** = `state == "idle" && detail != "" &&` not acknowledged →
  a **steady bright-white** light with a soft glow. It stands out as "look here" but does
  **not** pulse — pulsing stays reserved for the act-now states (blocked/error) so a done
  light never competes with a session that needs input (UI Principle #2).
- Acknowledged idle stays gray, now **dimmed (opacity 0.55)** so it recedes.
- The app keeps `reviewedAt: sessionId → updated_at`. Clicking a light records the current
  finish time and (as before) focuses the session; the light drops to dim gray. The ack is
  keyed by `updated_at`, so the **next** finished turn (new `updated_at`) re-lights on its own.
  The map is cleared when a session's light is removed.

**Reasoning:** Click-to-acknowledge reuses the interaction that already exists (click →
focus the session), so the act of going to look at a session *is* the acknowledgment — no new
gesture, no menu (UI Principle #3). Keying on `updated_at` makes re-lighting automatic and
needs no "unreview" event. Gating on `detail` avoids falsely lighting a brand-new session that
never ran a turn. All of it is display-layer only: the hook contract and status-file schema
are untouched.

**Validation:** unit-tested the pure state machine across the full lifecycle — fresh
SessionStart idle → `idle`; running → `running`; Stop with wrap-up → `done`; click/ack →
`idle`; new prompt → `running`; next Stop (new `updated_at`) → `done` again; Stop with empty
message → `idle`; blocked/error unaffected (all pass). Rebuilt + reinstalled the app so the
running bar reflects it.

**Both surfaces:** implemented in the floating bar **and** the VS Code extension. The
extension keys its status-bar item color on the same `displayState` — `done` renders at full
brightness (default foreground) while acknowledged `idle` is dimmed (`disabledForeground`),
the status-bar analog of the bar's bright-white-vs-dim-gray. The extension's click-to-focus
now also acknowledges (the finish time `updated_at` is passed as a command argument), with the
same app-local `reviewedAt` map. The extension's reviewed state resets on extension-host
reload (not persisted) — acceptable for a glance cue. Recompiled, repackaged the `.vsix`, and
reinstalled via the bundled `code` CLI (takes effect on the next window reload).

---

## 015 — Settings panel: right-click the bar; orientation (horizontal/vertical) first

**Context.** The bar is a chromeless, transparent, non-activating NSPanel with no menu,
no Dock icon, and no titlebar — there was no place to change anything. The first requested
setting is bar orientation (horizontal vs vertical), which was already flagged as an open
item in NEXT_STEPS.md "Decisions needed" (light-bar visual design).

**Options considered (entry point).**

| Option | Pros | Cons |
| --- | --- | --- |
| Right-click bar → inline panel *(chosen)* | No new chrome competing with the lights; no extra Tauri window/Rust; window auto-resizes to fit the panel, shrinks back on close | Right-click as the only affordance is slightly non-obvious |
| Hover gear icon → panel | More discoverable | Adds a persistent visible control that competes with the lights (violates UI Principle #1 — nothing should compete with the one signal that matters) |
| Separate settings window | Clean separation | Heavier: a second Tauri window + Rust wiring + it appears in the app switcher, for a single toggle |

**Decision.** Right-click anywhere on the bar (dots included) toggles an inline settings
panel that appears below the lights; the window grows to fit and shrinks back when closed.
The panel opening rounds the pill's stadium radius to 15px so a tall box doesn't look wrong.

**Options considered (orientation mechanism).** Apply a `.vertical` class to `#bar` that
switches `#lights` from `flex-direction: row` to `column`. The existing content-hugging
auto-resize (`resizeToContent`) then reshapes the window with no other geometry changes —
so orientation is one CSS class plus the resize that already runs.

**Persistence.** The choice is stored in the webview's `localStorage`
(`agentstatus.orientation`), read on load. This is an app-local *display* preference, so it
stays out of the hook-written status files entirely — same principle as the app-local
`reviewedAt` map (decision 014), and it survives restarts without a config file or Rust
plumbing. Window *position* is still handled by `tauri-plugin-window-state`; this is
orthogonal.

**Scope.** Frontend only (`index.html`, `styles.css`, `main.js`) — no Rust, no hook, no
schema change. The panel is built to take more rows as future settings are added.

---

## 016 — Bar click-to-focus via the IDE CLI (not `open -a <folder>`)

**Date:** 2026-07-06
**Status:** Accepted (fixes the click-to-focus mechanism from M4 / decision 015's `ide` routing)

**Context:** Clicking a light on the floating bar was reported to "open a brand new VS Code"
instead of focusing the session's existing window. The bar focused via
`open -a "Visual Studio Code" <cwd>` (a Rust `focus_session` command). Root cause, verified
live: `open -a <folder>` opens a **new** window whenever macOS can't match the folder to an
already-open window — and a **full-screen** IDE window living in its own macOS Space is
*exactly* such an unmatchable case. Since full-screen coding is this app's core use case
(decision 008), the misfire was the common path, not an edge case.

**Investigation (all observed live, not assumed — Agent Guideline #4):**
- `ide` routing (decision 015) was **correct** — the visible sessions' `ide` was `vscode` and
  mapped to the right app; the bug was the focus primitive, not IDE misattribution.
- **AppleScript window-raise was evaluated and rejected.** Plan: `activate` the IDE, then
  `AXRaise` the window whose title contains the folder. Titles *are* readable, even for
  full-screen windows. **But** `System Events`' `every window` only enumerates full-screen
  windows whose Space is **currently active** — directly observed: the ApplicationBot
  full-screen window appeared in one enumeration and was absent moments later once a different
  Space was frontmost. So a session's full-screen window on another Space is invisible to the
  script → it would fall through to `open -a` → new window, i.e. it wouldn't fix the reported
  case. It also needs a one-time Accessibility-permission grant. Rejected.
- **The deep link** `vscode://…/open?session=<id>` stays rejected: the Claude Code URI handler
  routes through `createSession`/`resumeSession` (confirmed by reading its `webview/index.js`),
  so it can spawn/resume rather than just focus, plus a URI-consent prompt.
- **The IDE's own CLI focuses without duplicating** — verified: with the AgentStatus folder
  already open, `code <folder>` kept the standard-window count at 2 (focused, no new window).
  Because the IDE manages its own window, the focus is Space-aware (Electron `focus()` switches
  to a full-screen Space), with no Accessibility permission.

**Decision:** `focus_session` now focuses via the IDE CLI:
`/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code <root>` (or the Cursor
CLI when `ide == "cursor"`). The **workspace root** is resolved from `~/.claude/ide/*.lock`
(`workspace_root(cwd)` — longest `workspaceFolders` entry that equals or is a prefix of `cwd`),
so a session that `cd`'d into a subfolder still focuses the window that has the root open
(this also cleans up the decision-013 subfolder-label quirk's focus half). If the CLI binary
is absent, fall back to `open -a "<app>" <root>` (degrade, never break — Agent Guideline #3).

**Reasoning:** The IDE resolving folder→window internally is the only tested primitive that
(a) focuses across Spaces including full-screen, (b) never spawns a duplicate for an open
folder, and (c) needs no extra OS permission. The lock-file root resolution reuses the
window-scoping insight from decision 006 and fixes subfolder `cwd`s for free.

**Scope note:** the bar still focuses at **window** (not tab) granularity, so multiple sessions
sharing one folder all focus the same window. True per-tab focus needs the extension route
(`claude-vscode.editor.open <sessionId>`, decision 012), which the extension already uses; the
standalone bar has no non-spawning path to a specific tab. Deferred.

**Validation:** `workspace_root` maps the live session `cwd`s to their lock-file roots; `code
<folder>` confirmed to focus-not-duplicate an already-open folder. Rebuilt + reinstalled the
app. **User to verify** the one thing only visible on their machine: clicking a light for a
session on another full-screen Space switches to that Space instead of opening a new window.

---

## 017 — Settings: light size + per-state colors (CSS variables); keep-on-screen resize

**Date:** 2026-07-06
**Status:** Accepted

**Context.** Following the settings panel (decision 015), the next requested settings were
**light size** and **per-state colors**. Both are pure display preferences.

**Mechanism — CSS custom properties.** Refactored the hard-coded dot geometry and state
colors in `styles.css` into variables on `#bar` (`--dot-size`, `--c-running/blocked/done/
idle/error`) with the originals as defaults. Each state's glow is now derived from its base
color via `color-mix(in srgb, var(--c-x) N%, transparent)` instead of a separate hard-coded
`rgba()`, so editing one color updates both the fill and its halo. The settings panel writes
these variables at runtime from `localStorage` — same app-local, frontend-only pattern as
orientation; defaults mirror the CSS so a cleared/absent pref is indistinguishable from the
stock bar. Storage: `agentstatus.dotsize` (int px, clamped 8–24) and `agentstatus.colors`
(JSON `state→hex`). Size is a range slider; colors are five native `<input type="color">`
swatches; a "Reset to defaults" clears all three display prefs (orientation included).

**Controls considered (colors).** Native `<input type="color">` (chosen — minimal, full
range, standard) vs. an in-webview preset-swatch palette vs. a hex text field. Native picker
opens the macOS system color panel; on a **non-activating NSPanel** (decision 008) that may
not surface — flagged as the item to verify live. Fallback if it fails: in-webview swatches
or a hex field (no OS panel). Size uses a plain slider, so it carries no such risk.

**Glanceability note.** Recoloring states can undercut UI Principle #1 (unambiguous,
consistent colors) — e.g. making *blocked* green. Accepted as the user's choice; the sensible
defaults + one-click Reset keep the stock semantics one click away.

**Panel growth direction — lights stay put, panel grows toward center.** The window
auto-resizes from a fixed top-left origin, so naively opening the panel below the lights and
then clamping the window on-screen *moved the lights* (a bottom-edge bar got shoved up). The
accepted behavior: the **lights never move** on open or close; the panel grows toward the
screen's middle. Implementation (JS, Tauri window API — no Rust): on toggle, capture the
`#lights` element's screen position (`currentMonitor` scale + `outerPosition` + its client
rect); pick the growth direction from the lights' position vs. the monitor center — panel
*above* when in the bottom half (`flex-direction: column-reverse`), *below* in the top half;
aligned to the lights' right edge (grows left) in the right half, left edge (grows right) in
the left half (`align-items` flex-end/start); then after the resize, `setPosition` the window
so the lights land back on the captured anchor. The anchor math cancels any constant
content↔window offset, so the lights are pinned exactly. Anchoring runs **only on the
open/close toggle**, not on every resize — otherwise a poll-tick resize would snap a
panel-open bar back after the user dragged it. The close path re-captures the *current* lights
position, so a drag while open is respected (it collapses in place, doesn't jump back to where
it opened). Superseded the first cut (`keepOnScreen`, a blanket inward clamp) which moved the
lights and whose `appWindow.currentMonitor()` silently threw (v2 monitor APIs are module-level
functions, fixed to the module `currentMonitor()`).

**Also.** A **wrapper padding** slider (`--bar-pad`, 2–20px; sides derive as `pad + 4px` to
keep the pill shape) controls the gap between the lights and the pill edge. `keepOnScreen`'s
first cut silently no-op'd because it called `appWindow.currentMonitor()` — in Tauri v2 the
monitor APIs are module-level functions, not window methods, so it threw and was swallowed by
`resizeToContent`'s catch; fixed by calling the module `currentMonitor()`.

**Scope.** Frontend only (`index.html`, `styles.css`, `main.js`) — no Rust, hook, or schema
change.

## 018 — Cursor support (via Cursor's Claude-hook bridge + native hooks)

**Date:** 2026-07-06
**Status:** Accepted
**Evidence:** Cursor **3.10.11**, observed live via a temporary Cursor event logger
(`hooks/cursor-log-events.sh` + `hooks/cursor-logger-setup.mjs`, kept for re-verifying future
Cursor versions — parallels the Claude logger of #006) and Cursor's own hook logs under
`~/Library/Application Support/Cursor/logs/**/cursor.hooks.*.log`, which record each hook's
full input payload and exit status.

**Context:** The user asked to extend the bar to other IDEs, specifically **Cursor's native
agent** (Composer/Agent) — not Claude Code running inside Cursor. The signal layer (#001) is
Claude Code hooks, which fire only for Claude Code, so the initial concern was that Cursor's
agent would need an entirely separate signal source.

**Findings (verified live):**
- **Cursor natively bridges Claude Code hooks.** Cursor reads `~/.claude/settings.json` and
  runs its hooks, mapping its own events onto Claude event names. So the already-installed
  `~/.claude/status/report.sh` is invoked by Cursor with **exit 0** for `SessionStart`,
  `UserPromptSubmit`, `PostToolUse`, `Stop`, `SessionEnd`. Cursor sessions therefore already
  reached the bar — but with bugs (below).
- **The bridge drops the events we need for the rest.** Cursor logs `PermissionRequest` as
  "not supported", and `PostToolUseFailure`/`StopFailure`/`SubagentStart` as "unknown,
  skipping". So error/subagent don't come through the bridge; **blocked has no Cursor event
  at all**.
- **Payloads are clean and rich.** Every event carries `session_id` (= `conversation_id`),
  `hook_event_name`, `workspace_roots[]`, and `cursor_version`. `beforeSubmitPrompt` carries
  `prompt`; tool events carry `tool_name`/`tool_input`; `subagentStart/Stop` carry
  `subagent_id` + `subagent_type`; the bridged `Stop` carries `status` (e.g. `completed`).
- **Two bugs in how the stale `report.sh` read Cursor payloads:** it keyed cwd on `.cwd`, but
  Cursor puts the workspace in `workspace_roots[]` (session/prompt events have no `.cwd`; tool
  events carry a *tool-level* `.cwd` like `/tmp`) → blank/incorrect labels. And Cursor fires
  `sessionStart` for an unopened **`empty-state-draft`** composer → a phantom blank light.
- **Hard constraint:** Cursor only runs command hooks when a **workspace folder is open**. In
  a folder-less window every hook fails with `MainThreadShellExec not initialized` (0 ms, no
  exec), so an agent run in an empty/welcome window produces no signal.

**Decision:**
- **Reuse the bridge for running/idle/error/remove; add native hooks only for what it drops.**
  `hooks/cursor-setup.mjs` (idempotent/reversible, mirrors `setup.mjs`) registers
  `report.sh` in `~/.cursor/hooks.json` for **`subagentStart`, `subagentStop`,
  `postToolUseFailure`** only — pointed at the same app-maintained `~/.claude/status/report.sh`
  the bridge calls. Everything else the bridge already forwards, so registering it natively
  would just double-fire.
- **`report.sh` learns Cursor** (one script, both IDEs): normalize Cursor's camelCase event
  names → the Claude PascalCase names it already keys on; detect Cursor via `.cursor_version`;
  use `workspace_roots[0]` for cwd on Cursor; write a new **`ide`** field
  (`cursor`/`vscode`); skip the `empty-state-draft` phantom; accept `subagent_id` as the
  subagent marker key alongside Claude's `agent_id`.
- **Error (red) on Cursor = `Stop` with a failed `.status`** (the bridged `Stop` carries it),
  *not* `postToolUseFailure` — consistent with #013 (tool failures are recovered noise;
  red is turn-level). The exact failure status strings are unverified, so this is **interim**
  (matched heuristically on `error|fail|abort|cancel`); `postToolUseFailure` is registered for
  calibration only.
- **`ide` field drives click-to-focus:** the bar opens the session's host IDE (Cursor vs VS
  Code) from `ide` (consumed by the focus mechanism of decisions 015/016).
- **Blocked (orange) is not available on Cursor** — it exposes no "waiting on user" event
  (its permission prompts are driven *by* hooks, not reported *to* them). Documented limit,
  parallel to error being the soft spot on Claude Code in #006.

**Reasoning:** Because Cursor already bridges our hook, "another IDE" collapses from a second
signal layer into a payload-compat pass on one script plus three native hook registrations —
far less surface than a parallel implementation, and both IDEs run identical logic. Keying
native registration to only the bridge's gaps avoids redundant double-fires. Deferring blocked
and marking the error heuristic interim keeps us honest about what's verified (Guideline #4).

**Validation:** `report.sh` unit-tested against the real captured Cursor payloads — Cursor
prompt→running with `ide:cursor` and label from `workspace_roots`; tool-level `.cwd=/tmp`
does not override the workspace; `subagentStart/Stop` via `subagent_id` add/remove markers;
`Stop status=completed`→idle vs `status=aborted`→error; `empty-state-draft` skipped; and VS
Code Claude Code regressions intact (`ide:vscode`, `.cwd`, `agent_id`). The bar backend/
frontend compile clean with the `ide` field and IDE-aware focus. **Pending:** a live
folder-open Cursor run to confirm the end-to-end wiring (three prior attempts each landed in a
folder-less window, where Cursor runs no hooks); and rebuilding/reinstalling the packaged app
so the running bar reads `ide` and the app self-installs the Cursor hooks (the `install.rs`
port of `cursor-setup.mjs`, mirroring how #011 ported `setup.mjs`).

---

## 019 — Bar light click focuses the exact session tab (bar→extension relay)

**Context.** Decision 016 made a bar-light click raise the correct IDE *window* (via the IDE
CLI). But a single window can host several Claude Code sessions (the user routinely runs 4+ in
one folder), and window-raising can't distinguish them — clicking any light lands on the same
window, not the specific agent whose light you clicked. The user asked for the light to bring
them to the *associated agent*.

**The capability gap.** Focusing a specific session *tab* is done by the in-editor command
`claude-vscode.editor.open <session_id>` — which only code running *inside* VS Code can call.
The AgentStatus **extension** already uses it for its own status-bar clicks (decision 012),
popup-free. The floating **bar** is a separate process and can't call it; its only external
lever is the deep link `vscode://anthropic.claude-code/open?session=<id>`.

**Options considered.**

| Option | Focuses exact tab? | Popup? | Cost |
|---|---|---|---|
| A — bar opens the `vscode://…open?session=` deep link | yes | **yes, every click** | tiny (one Rust change) |
| B — bar → file → extension calls `claude-vscode.editor.open` | yes | no | small file contract + extension watcher |
| (status quo) window-raise only | no | no | — |

**Verification (Agent Guideline #4).** `NEXT_STEPS.md` recorded that the deep link "spawns new
agents + a popup." Re-tested live on the installed Claude Code: firing the deep link twice for
a real session spawned **no** new agent/session (session-file count and Claude process count
unchanged before/after both fires), but a **consent popup appeared on every fire** and the
editor did **not** switch tabs. So the "new agent" half was stale, but the popup half is real
and recurring — disqualifying option A for a glanceable one-click tool (UI Design Principle #3:
a light leads *straight* to the session, no detour).

**Decision — Option B (hybrid with 016).** On a light click the bar now does both:
1. **Window raise** — the decision-016 `code/cursor <workspace_root>` call brings the right
   window forward across Spaces (incl. full-screen).
2. **Tab focus** — the bar writes `~/.claude/status/focus-request.json`
   `{ "session_id", "requested_at" }` (`requested_at` = epoch **ms**, so two clicks in the same
   second stay distinct). Every window's extension polls this on its existing refresh timer;
   the window whose workspace owns that session calls `claude-vscode.editor.open <session_id>`
   and advances a per-window `lastFocusReq` watermark so each click fires exactly once. The
   watermark is seeded at `activate` from the file already on disk, so a stale request isn't
   replayed on window reload. A request for another window's session is ignored (that window
   handles it). This is the *only* option that satisfies the request — window focus alone can
   never disambiguate multiple sessions sharing one folder.

**Why a new file, not the existing schema.** The request is transient bar→extension IPC, not
session state, so it's a sibling of `sessions/` rather than a field on a session file — it never
touches the per-session status contract (decision 007) and the dumb/fast hook is unaffected.
`~/.claude/status/` is user-home runtime state, already outside git.

**Scope.** `app/src-tauri/src/lib.rs` (`write_focus_request`, `status_root`/`now_millis`
helpers, `focus_session` gains a `session_id` arg), `app/src/main.js` (passes `s.id` →
`sessionId`), `extension/src/extension.ts` (request reader + relay in `refresh`, watermark seed
in `activate`). No hook or per-session schema change.

## 020 — Single-instance guard (release only)

**Context.** Two bars were found running at once: the installed `/Applications/AgentStatus.app`
and the in-repo dev build (`app/src-tauri/target/release/…/AgentStatus.app`), launched minutes
apart. Both read the same `~/.claude/status/sessions/` dir and drew identical, overlapping bars.
Nothing detected or blocked a second copy — `run()` built the Tauri app with no instance guard,
so any number of copies could run.

**Root cause.** No single-instance mechanism. It surfaces whenever the installed copy (typically
a Login Item) is up and a dev build is launched during development — the exact workflow here.

**Options considered.**

| Option | Mechanism | Cost |
|---|---|---|
| A — `tauri-plugin-single-instance` | Second launch pings the running instance (keyed by the `com.agentstatus.app` identifier) and exits; a callback in the survivor re-shows its window | 1 dep + ~10 lines |
| B — hand-rolled PID/lock file | Own lockfile check on startup | Reinvents A; stale-lock edge cases |
| C — do nothing / kill dupes by hand | Discipline only | Recurs; contradicts "self-installing, no manual steps" |

**Decision — Option A.** Register `tauri-plugin-single-instance` as the first plugin in `run()`.
Because both bundles share the identifier `com.agentstatus.app`, the plugin catches the
installed copy *and* any dev build. The second process exits immediately; the callback re-shows
the survivor's `main` window.

**Release-gated (`#[cfg(not(debug_assertions))]`).** The guard is compiled only into release
builds — the shipped app, where a duplicate is always wrong. In dev it's off, so `npm run tauri
dev` still runs alongside the installed `/Applications` copy while iterating. This keeps the fix
from getting in the way of development, at no cost to real users (who only ever run release
builds).

**Scope.** `app/src-tauri/Cargo.toml` (add `tauri-plugin-single-instance = "2"`),
`app/src-tauri/src/lib.rs` (`run()` builds a mutable builder, conditionally adds the plugin
first). No schema, hook, or installer-contract change; takes effect once the app is rebuilt and
reinstalled.

## 021 — Faster click-to-focus (fast osascript raise + CLI fallback)

**Context.** Decision 016 raises the target IDE window on a bar click via the IDE's own CLI
(`code`/`cursor <root>`), chosen because it reaches full-screen windows on other Spaces. But the
CLI boots a Node runtime on every invocation — measured ~1.15s — so even when the target window
is already on the *current* Space (no Space switch needed) the window takes a second-plus to come
forward. The user reported the lag specifically for same-Space switches.

**Measurements.**

| Method | Time | Window-specific? | Cross-Space / full-screen? | Permission |
|---|---|---|---|---|
| `code`/`cursor <root>` (decision 016) | ~1.15s | yes | yes | none |
| `osascript` app-activate | ~0.12s | no (frontmost only) | no | Automation |
| `osascript` System Events `set frontmost` + AXRaise by title | ~0.17–0.47s | yes | no | Accessibility |

**Decision — hybrid: fast osascript raise *and* the CLI, both fired.** On a click `focus_session`
now first runs `raise_window_fast(root, ide)`: an `osascript` that, via System Events, sets the
IDE process `frontmost` and performs `AXRaise` on the window whose title contains the
workspace-root basename (the project folder, which appears in the IDE window title). This brings a
same-Space window forward in ~0.2s. The decision-016 CLI still fires right after, unchanged,
covering the case the fast path can't: full-screen windows and windows on inactive Spaces (which
System Events can't see/raise — the reason AppleScript was rejected as the *sole* mechanism in
016). When the window is same-Space the CLI just re-activates the already-front window ~1s later —
harmless, no new window.

**One permission, graceful degradation.** Going through System Events (`set frontmost` + AXRaise)
needs only **Accessibility** — not a per-app Automation/Apple-Events grant (which `tell
application "X" to activate` would have required, one prompt per IDE). Without the Accessibility
grant the `osascript` errors out and is ignored (`.spawn()`, result discarded), so the CLI alone
runs — identical to pre-021 behavior. So the speedup is opt-in via the OS permission and there is
**no regression** without it. The Accessibility grant is a manual macOS step that can't be
scripted (TCC is user-controlled); it's documented in `install.sh` output as optional.

**Scope.** `app/src-tauri/src/lib.rs` (`raise_window_fast` helper; one call at the top of
`focus_session`'s macOS block). No change to the CLI path, the extension tab-focus relay
(decision 019), the schema, hooks, or the installer contract.

---

## 022 — Bar position persistence, all-monitor drag clamp + magnetism, Reload button, installer auto-restart (2026-07-06)

**Context.** Four display/workflow gaps surfaced while iterating: (a) even padding — the `+4px`
side padding that pills-out a horizontal row left the sides fatter than the top/bottom in vertical
mode; (b) the bar could be dragged fully off-screen, and an earlier single-monitor clamp trapped it
on one display; (c) there was no way to reload the webview from the UI; (d) every rebuild recentered
the bar because the window has `center: true` and nothing persisted its position — and, because of
the single-instance guard (decision 020), a rebuilt app launched against the still-running old
instance would just exit, leaving stale code on screen.

**Decision.**
- **Even padding (vertical):** `#bar.vertical { padding: var(--bar-pad) }` overrides the horizontal
  `var(--bar-pad) calc(var(--bar-pad) + 4px)` so all four sides match. The `+4px` stays in
  horizontal mode, where it forms the stadium-pill shape. CSS-only.
- **Drag clamp across all monitors + magnetism:** on the window `moved` event, `clampToMonitor()`
  (1) snaps any window edge within `SNAP_LOGICAL = 16` logical px (scaled per-monitor by
  `scaleFactor`) flush to that monitor edge; (2) clamps to the **union bounding box** of every
  `availableMonitors()` so the bar slides freely across shared edges but can't leave the outer
  edges; (3) if the bar's center lands in a dead gap between mismatched displays, pulls it onto the
  nearest one. Replaces the earlier current-monitor-only clamp. Because the native
  `data-tauri-drag-region` drag is OS-driven, corrections land reliably on drop (mid-drag
  `setPosition` can be overridden by the next OS frame) — which is the desired "snap on release."
- **Position persistence — saves the *lights'* screen anchor, not the window top-left.** The window
  grows/shrinks around the fixed lights as the settings panel opens/closes, so its top-left depends
  on panel state; persisting it and restoring onto a differently-sized window shifts the lights
  (concretely: the Reload button lives *in* the panel, so a reload always saved the panel-open
  top-left then restored it onto the panel-closed window → the bar jumped). Fix: `onWindowMoved`
  saves `lightsScreenPos()` (`{x,y,scale}`, `agentstatus.pos`) on each move, and `restoreAnchor()`
  on launch sizes the bar then `anchorLightsTo(saved)` to land the lights back on that screen point
  (over `center: true`), re-validating on-screen via `clampToMonitor`. An `anchorReady` flag
  suppresses saving during startup layout so involuntary `setSize` moves don't clobber the saved
  anchor before it's restored.
  Chosen `localStorage` over `tauri-plugin-window-state` to avoid a Rust dependency and a
  size-restore conflict with the content-hugging `resizeToContent()` (the plugin restores size too,
  which auto-resize immediately overrides). `localStorage` lives in the webview data dir keyed by
  the bundle id, so it survives `install.sh` replacing the `.app` bundle — same mechanism that
  already persists orientation/size/colors (decision 015/017). First-ever launch (no saved pos)
  still centers.
- **Reload button:** a low-key link in the settings-panel footer (next to "Reset to defaults")
  calls `window.location.reload()` — recovers a stuck webview and re-reads prefs without quitting.
- **Installer auto-restart:** `install.sh` records whether an instance was running *before* the
  copy; if so (a reinstall of an already-Gatekeeper-approved app), it `pkill`s the old process,
  waits for the single-instance socket to release, and `open`s the rebuilt app — so rebuilds take
  effect in one command. First-time installs (nothing running) fall through to the manual
  right-click-Open instructions, because an unsigned app can't be `open`ed past Gatekeeper
  non-interactively.

**Scope.** `app/src/main.js` (clamp/persist/restore, Reload wiring), `app/src/index.html` +
`app/src/styles.css` (vertical padding, Reload button, footer), `install.sh` (conditional
restart). No change to the status-file schema, the hook contract, or the event→state mapping.

**Verification.** Rebuilt via `install.sh` (auto-quit + relaunch confirmed, single running PID).
First persistence attempt saved the window top-left and the bar jumped on Reload (the panel-open →
panel-closed size change described above); corrected to the lights-anchor approach and rebuilt.
**Left to verify live:** Reload no longer moves the bar, and the anchor restores across a full
quit/relaunch. Multi-monitor crossing/magnetism not yet observed on a real multi-display setup.

---

## 023 — Bar opacity slider

**Date:** 2026-07-06 · **Status:** Accepted

**Context.** The bar's pill is drawn with a fixed translucent fill
(`background: rgba(20, 22, 26, 0.82)`). The user wanted to control how see-through the
bar is — e.g. make it fade further into the desktop, or make it fully solid.

**Decision.** Add an **Opacity** slider (0–100%) to the settings panel, following the same
frontend-only `localStorage` + `applyStyle()` pattern as the decision-017 size/padding
controls. The slider drives a new `--bar-opacity` CSS variable on `#bar` (0–1, the percent / 100).

**The whole pill fades together — fill, border, shadow, and blur — not just the fill.** A first
cut varied only the fill alpha (`rgba(20, 22, 26, var(--bar-opacity))`), but when the bar is
minimized to a few lights the fill is a small fraction of the visible chrome — the border and the
blurred halo dominate — so the slider was barely perceptible there (user feedback). Fix: the
border, drop-shadow, and backdrop-blur alphas/radius are all scaled by `--bar-opacity` via
`calc()`, with multipliers **normalized so 82% reproduces the original look** (`0.11 → 0.09`
border, `0.46 → 0.38` shadow, `17px → 14px` blur). Now the whole pill fades as one; at 0% it
vanishes entirely and only the lights float. Range widened to 0–100 (was 20–100) for more travel
in the transparent direction; 100% is already fully opaque (fill alpha 1.0), so that's the solid
ceiling — there's nowhere past it.

**The lights never fade.** They're separate, fully-opaque elements, so even at 0% the signal
reads at full strength — preserving UI principle #1 (glanceable) and #2 (attention states stand
out). When idle with no sessions, the empty-state dot (`rgba(255,255,255,0.18)`, unscaled) stays
visible as a grab target. Stored as a whole percent to match the integer slider. `Reset to
defaults` clears it back to 82% (the original CSS value).

**Scope.** `app/src/index.html` (Opacity row, `min=0`), `app/src/styles.css` (`--bar-opacity`
variable driving fill/border/shadow/blur + shared slider styling), `app/src/main.js`
(`OPACITY_KEY`/`DEFAULT_OPACITY`, `currentOpacity`, `setOpacity`, `applyStyle` wiring, reset
cleanup, event listener). No change to the status-file schema, the hook contract, or the
event→state mapping.

## 024 — First public release (v0.1.0)

**Date:** 2026-07-06
**Status:** Accepted

**Context:** Milestones 1–6 are done and the app has been running locally off `install.sh`
for days. To let anyone else install it without a Rust/Node toolchain, we need a
distributable artifact and a public entry point. This is a distribution decision (it changes
how a user obtains and trusts the app), so it's logged here per Agent Guideline #9.

**Options considered:**

| Option | Pros | Cons |
|---|---|---|
| **GitHub Release with prebuilt DMG** (chosen) | One download, no toolchain; standard channel; `install.sh` stays for devs/Intel | Unsigned → Gatekeeper friction; per-arch artifact |
| Build-from-source only (status quo) | No new infra; always matches HEAD | Requires Rust + Node + jq; excludes non-developers |
| Homebrew cask | `brew install` familiarity | Casks want a signed/notarized app or a hosted binary anyway; premature for v0.1.0 |

**Decision:**

- **Tag `v0.1.0`** (matches `tauri.conf.json` / `package.json`) and cut a **GitHub Release**
  with **`AgentStatus_0.1.0_aarch64.dmg`** attached, rebuilt fresh from the tagged commit.
- **Apple Silicon only** for this release (decision at the user's direction). Intel users
  build from source via `install.sh`; a universal binary is a future enhancement (needs the
  `x86_64-apple-darwin` target + a lipo'd build, and the Intel slice would be untested).
- **Unsigned / unnotarized.** No Apple Developer signing yet, so a downloaded app carries
  `com.apple.quarantine` and macOS 15+/26 blocks it (the old Finder right-click → Open bypass
  was removed for downloaded apps). The README documents the two ways past it:
  `xattr -dr com.apple.quarantine /Applications/AgentStatus.app`, or **System Settings →
  Privacy & Security → Open Anyway**. Code signing + notarization is deferred (Milestone-5
  note) — it costs a paid Developer account and only removes friction, not capability.
- **README rewritten** to lead with the DMG download as the primary install path and keep
  build-from-source as the alternative.

**Reasoning:** A prebuilt DMG is the lowest-friction way to reach non-developers and is the
conventional macOS distribution channel; GitHub Releases give a stable "latest" URL for free.
Shipping unsigned is an accepted, documented tradeoff for a v0.1.0 — the Gatekeeper steps are
a one-time, reversible, fully-scripted action (no manual per-user hand-holding beyond a copy-
paste). Apple-Silicon-only keeps the first release to a single tested artifact; source build
covers the rest. No change to the hook contract, status-file schema, or event→state mapping.

---

## 025 — Settings: light sort toggle (Window vs Urgency)

**Date:** 2026-07-06
**Status:** Accepted

**Context.** The user asked to sort the bar by "what window a session is in." The hard
constraint (decision 006, re-confirmed): hooks expose **no true per-window identifier** —
`VSCODE_PID`/`VSCODE_IPC_HOOK` are the shared main VS Code process, identical across windows —
so the only usable proxy for "which window" is the session's **workspace folder** (its `cwd`).
Two windows with the same folder open are therefore indistinguishable and merge into one
group; the user explicitly accepted this limit rather than chase a workaround. The user also
chose to expose this as a **settings toggle** (like orientation) rather than a fixed order.

**What already existed.** `list_sessions` (Rust) already sorted by the `cwd` **basename**
(`label`) then `id` — a crude window grouping. Two flaws: a session that `cd`'d into a
subfolder sorted away from its window's siblings, and two different folders sharing a basename
(two `app/` dirs) falsely merged.

**Decision & mechanism (frontend only, `app/src/`).**
- New **Sort** segmented control in the settings panel with two modes, persisted app-local in
  `localStorage` (`agentstatus.sort`) — same pattern as orientation/size/colors, no hook or
  status-file schema change:
  - **Window** (default) — `sortSessions` orders by the full `cwd` path, then `id`. The full
    path fixes both flaws above: same-basename windows stay distinct, and lexicographic order
    naturally clusters a workspace root with any subfolder `cwd` beneath it. Stable across
    polls (only reshuffles when sessions are added/removed).
  - **Urgency** — orders by rendered `displayState` (`error → blocked → done → running →
    idle`), tie-broken by the same window key. Surfaces attention states first (UI Principle
    #2); a light moves only when its own state changes, staying window-grouped within a state.
- Sorting moved into the frontend `tick()` (was the Rust `list_sessions` sort, now redundant
  but left as a harmless default order). `setSort` re-orders immediately from the last poll so
  the bar and the menu-bar tray reflow without waiting a tick. `Reset to defaults` clears the
  key back to Window.

**Rejected / not done.** A true per-window key (would need the per-window extension or
lock-file port disambiguation) — declined by the user (accept the merge). A Rust `root` field
resolving `cwd`→workspace root via the IDE lock files was considered for tighter subfolder
grouping, but the lexicographic full-`cwd` sort already clusters subfolders adjacent to their
root, so it wasn't worth the Rust change / rebuild for a frontend-only feature.

**Reasoning.** Keeps the feature in the established frontend-only, `localStorage` settings
pattern (no hook, schema, or event-mapping change — Agent Decision Framework #1/#2), upgrades
the pre-existing basename grouping to a correct full-path one for free, and honors the
signal-layer limit honestly (same-folder windows merge) instead of pretending to a per-window
fidelity the hooks can't provide.

---

## 026 — Presentation-mode toggle: floating panel ↔ macOS menu bar

**Date:** 2026-07-06
**Status:** Accepted (amends #003 — menu bar is now an optional *mode*, not rejected outright)

**Context.** The bar has only ever been the floating NSPanel (#003/#008). The user wanted to
**toggle** it into the macOS menu bar as an alternative presentation, keeping floating. #003
had explicitly *rejected* a menu-bar app ("Not a repositionable bar placed anywhere on
screen") — but that was as the sole display layer. As an optional second mode alongside the
panel, it's additive, so this amends #003 rather than reversing it.

**The core constraint.** The menu bar shows an `NSStatusItem` — an image/title, **not** a web
view — so the interactive web bar can't render *in* the menu bar. The accumulated web-layer
value (custom colors #017, per-light tab focus #019, hover tooltips #009, subagent badges,
done/idle split #014) would be lost by a native re-draw. Confirmed with the user, the chosen
shape keeps all of it:

- **Live dots in the menu bar** = the tray icon is an **image the webview renders**. Each poll
  the frontend draws the dots to an offscreen `<canvas>` (reusing `displayState()` +
  `currentColors()` — one source of truth for the per-state palette), extracts the RGBA, and
  hands it to Rust (`set_tray_image`), which sets it as the `TrayIcon` image. Only pushed when
  a signature (states + colors + condense) changes — same "update on change only" discipline as
  the DOM reconciler. A **condense** sub-option draws a single summary dot for the most-urgent
  state (`error > blocked > done > running > idle`).
- **Click the item → the same panel drops down as a popover** below it (`toggle_popover`
  positions the window under the click point and `show()`s it; a light-click or a second tray
  click hides it). Because it's the *same* NSPanel, per-light focus/hover/badges all work
  unchanged.
- **Toggle in the settings panel** (`Mode: Floating | Menu bar`), `localStorage`-persisted
  (`agentstatus.mode`, `agentstatus.menubarcondense`) — same app-local pattern as every other
  display pref; no hook/schema change.

**Menu-bar mode forces horizontal** (`effectiveOrientation()`): a vertical column hanging off
the menu bar looks wrong, so the saved orientation is overridden to horizontal while in
menu-bar mode (and restored on return to floating); the Orientation control is hidden there.

**Two macOS gotchas found live (both fixed):**
1. **Tray ops must run on the main thread.** Tauri commands run on a background thread;
   `tray.set_visible`/`set_icon` from `set_mode`/`set_tray_image` silently no-op'd off-main
   (while `win.hide()`, which Tauri marshals internally, still fired — so the panel vanished
   and no tray appeared). Fix: wrap tray calls in `app.run_on_main_thread(...)`. Also: hide the
   panel *only* when a tray actually exists, so a tray failure can never strand the user with
   no UI.
2. **The icon renders as a black silhouette unless forced non-template.** A template
   `NSStatusItem` image is drawn as a monochrome alpha mask (opaque → black/white), swallowing
   the dot colors. The builder's `icon_as_template(false)` doesn't survive `set_icon`, so
   `set_icon_as_template(false)` is re-asserted on every image.

**Known limits (documented, not bugs):**
- **The menu bar auto-hides in full-screen apps**, so in menu-bar mode the lights are hidden
  while the user is in full-screen VS Code — exactly the case #008's floating panel exists to
  cover. This is why it's a per-situation toggle, not a replacement: floating for
  over-full-screen glance, menu bar for a tidy always-there presence otherwise.
- **Can't force the item rightmost.** macOS reserves the right edge for system items and lets
  the user ⌘-drag third-party items (position persisted by the OS). No public API pins it, so
  the item can land near the notch; ⌘-drag once is the supported "pin." An `autosaveName` route
  exists but needs native `NSStatusItem` access the `tray-icon` layer doesn't expose — deferred.
- Popover **auto-dismiss on click-elsewhere** and an **animated pulse in the tray image** for
  blocked/error are deferred (v1 tray dots are static; attention still reads via color).

**Scope.** `app/src-tauri/src/lib.rs` (tray built in `setup`, `set_mode` + `set_tray_image`
commands, `toggle_popover`, main-thread marshaling), `app/src-tauri/Cargo.toml` (`tray-icon`
feature), `app/src/main.js` (mode/condense prefs, `drawTray`/`pushTrayImage`,
`effectiveOrientation`, mode-gated anchor/resize), `index.html` + `styles.css` (Mode/Condense
rows). No hook, status-file schema, or event-mapping change.

**Validation.** Rust `cargo check` clean; frontend parses. Live (dev build): tray builds,
`set_tray_image` fires (confirmed via logs), the two macOS gotchas above were found and fixed
by observation. Shipped via `install.sh` (release build, single-instance). **Left to verify
with the user:** the colored dots are visible/findable in their menu bar (subject to the notch),
the popover opens horizontally, and click-to-focus works from it.

---

## 027 — Stale-light fix: prune on IDE-window close, not just the 2h idle timer

**Date:** 2026-07-07
**Status:** Accepted (extends #004's heartbeat staleness — additive, no schema/hook change)

**Context.** Lights sometimes lingered after a session was really gone. The only staleness
backstops were the clean `SessionEnd` hook (skipped on force-quit / window-close) and the
`MAX_IDLE_SECS = 2h` idle timer (#004). So a session whose IDE window closed without
`SessionEnd` kept its light for **up to two hours**. Observed live: two Cursor sessions with
**empty `cwd`/`label`** (anonymous gray ghosts, idle ~82 min) no window owned, still on the bar.
The 2h timer is deliberately long because an idle or *blocked* session emits no further hook
events while it waits — shortening it would kill a session the user is mid-decision on. So the
timer alone can't be the answer.

**The signal.** `~/.claude/ide/*.lock` — one file per **live IDE window**, each listing its
`workspaceFolders` and owning `pid`. Already parsed by `workspace_root()` for click-to-focus,
so the mechanism is proven. When a window closes, its lock disappears; a force-quit leaves the
lock but the `pid` dies. Every real session's `cwd` maps to a live lock; the two ghosts map to
none.

**Options considered.**
| Option | Verdict |
|---|---|
| **Lock-liveness + keep 2h backstop** (chosen) | Prune the instant a session's workspace maps to no live lock; keep the 2h timer for the in-window case. Purely additive, no regression. |
| Lock-liveness + shorten timer to ~15 min | Rejected: shortening reintroduces the risk of pruning a session left blocked/idle in an open window mid-decision — the exact thing #004's long timer protects. |
| Only shorten the timer (no lock check) | Rejected: same blocked-session risk, and window-close still takes 15 min to clear instead of being instant. |

**Decision.** In `list_sessions`, build `live_workspace_folders()` from the lock files
(skipping any lock whose `pid` fails a `kill(pid, 0)` probe — force-quit / crash left it
behind). A session is pruned (file + subagent markers deleted, self-heals on its next event)
when **either**:
- `window_gone` — its `cwd` matches no live workspace folder (exact match or a subfolder it
  `cd`'d into, same prefix rule as `workspace_root`; an **empty `cwd` matches nothing** → an
  anonymous ghost is pruned). Instant.
- silent past `MAX_IDLE_SECS` — the unchanged #004 backstop, which still covers a **superseded
  session sharing a live window's lock** (two sessions in one window share one lock, so
  liveness can't tell a dead one from a live one there — the accepted limit).

**Safety gates.**
- If **no** live lock exists at all (`live_folders` empty — a no-IDE machine, or a transient
  read failure), lock-pruning is **skipped entirely** and only the timer applies, so one bad
  read never nukes every light.
- `kill(pid, 0)` treats `EPERM` (process exists, another owner) as alive, so a valid window is
  never misread as dead.

**Caveat (accepted, flagged to user).** A Claude session in a **standalone terminal**
(Terminal.app / iTerm, outside any IDE window) has no lock, so when *any* IDE is open it is
pruned. Integrated-terminal sessions are safe — their `cwd` sits under the window's workspace
folder, so they match the lock. This fits the app's "one light per IDE tab" model.

**Scope.** `app/src-tauri/src/lib.rs` (`pid_alive`, `live_workspace_folders`, `cwd_is_live`;
`list_sessions` liveness prune), `app/src-tauri/Cargo.toml` (`libc` as a macOS-target dep for
`kill(pid, 0)` — already in the lock tree at 0.2.186). No hook, status-file schema, or
event-mapping change.

**Validation.** `cargo build` clean. Verified against live machine state (temporary test
module, since removed): the shipped `cwd_is_live`/`live_workspace_folders`/`pid_alive`
correctly flagged both empty-`cwd` Cursor ghosts `window_gone=true` (pruned) while keeping all
three real sessions — including one whose `cwd` was the `app/src-tauri` subfolder, matched back
to the open AgentStatus window by the prefix rule — and did not false-match a
`TradingBotXtra` prefix. **Left to confirm with the user:** rebuild + reinstall the app (the
running installed copy predates this change), then close an IDE window and watch its light
vanish within one poll.

---

## 028 — Quit button in the settings panel

**Context.** AgentStatus runs as a macOS **Accessory** app (`ActivationPolicy::Accessory`)
so it has no Dock icon and no application menu — the two places macOS normally puts a Quit
command. Until now the only way to stop the bar was Activity Monitor, `kill`, or (in menu-bar
mode) nothing at all. Users need an obvious in-UI way out.

**Choice.** A **Quit** button in the settings-panel footer (next to Reload / Reset to
defaults) wired to a new `quit_app` Tauri command that calls `app.exit(0)`, tearing down the
panel, tray, and process. Hover is red-tinted so it reads as the one destructive footer action
without adding a confirm step (quitting is cheap and fully reversible — relaunching the app
re-reads the hook-written status files and repopulates the bar).

**Why not an app/tray menu Quit instead.** The tray item's left-click is already owned by the
popover toggle (#026) and adding a right-click menu just for Quit would split the settings
surface across two places; keeping every control in the one settings panel is simpler and
consistent with Reload/Reset.

**Scope.** `app/src-tauri/src/lib.rs` (`quit_app` command + handler registration),
`app/src/index.html` (footer `#quit-btn`), `app/src/main.js` (click → `invoke("quit_app")`),
`app/src/styles.css` (shared footer-button style + red hover). No hook, status-file schema, or
event-mapping change. `cargo check` clean.

---

## 029 — Codex compatibility

**Date:** 2026-07-09
**Status:** Accepted

**Context.** AgentStatus already had a shared status hook for Claude Code and Cursor, but
Codex reads user-level hooks from `~/.codex/hooks.json` or inline `[hooks]` in
`~/.codex/config.toml`, not `~/.claude/settings.json`. The official Codex Hooks manual
(fetched 2026-07-09) also has a slightly different supported event set: no `SessionEnd`,
`StopFailure`, or `PostToolUseFailure`.

**Decision.** Keep one reporter (`hooks/report.sh`) and install it into both ecosystems:
Claude/Cursor keep the existing full event set in `~/.claude/settings.json`; Codex gets only
Codex-supported events in `~/.codex/hooks.json` (`SessionStart`, `UserPromptSubmit`, `Stop`,
`SubagentStart`, `SubagentStop`, `PreToolUse`, `PostToolUse`, `PermissionRequest`). The packaged
app self-installer and dev `node hooks/setup.mjs install` both perform the same split.

The reporter now accepts Codex-style ids (`thread_id`, `threadId`, `conversation_id`,
`conversationId`, nested thread/conversation ids), falls back to the hook command's working
directory for `cwd`, and writes `ide:"codex"` so the app can handle those lights separately.

**App behavior.** Codex sessions do not use Claude IDE lock files, so `list_sessions` skips
lock-based pruning for `ide:"codex"` and relies on the existing 2h idle backstop. Clicking a
Codex light opens `Codex.app`; exact thread focusing is not attempted because there is no
documented external open-by-thread command analogous to the VS Code extension relay.

**Validation.** Verified with the official Codex manual, `bash -n hooks/report.sh`,
`node --check hooks/setup.mjs`, `cargo check`, and temp-dir smoke tests that wrote Codex-shaped
session JSON with `ide:"codex"`, cwd from the process directory, and expected running/detail
fields.

---

## 030 — Product rename: AgentStatus

**Date:** 2026-07-09
**Status:** Accepted

**Context.** After Codex support (#029), the name `ClaudeStatus` no longer described the product:
the same lightbar now tracks Claude Code, Codex, and Cursor sessions. Keeping the old name would
make install docs, release assets, and user-facing app chrome misleading.

**Decision.** Rename the product to **AgentStatus** across the app, installer, docs, extension
metadata, Tauri product name/window title, bundle identifier (`com.agentstatus.app`), tray id,
localStorage keys, hook backup suffixes, and release asset names.

**Migration.** Keep compatibility where an existing user may already have runtime state:
`AGENTSTATUS_DIR` is the new status-dir override, but `CLAUDESTATUS_DIR` remains a legacy alias;
`AGENTSTATUS_IGNORE` is the new opt-out, but `CLAUDESTATUS_IGNORE` still works. The installer also
removes and stops an old `/Applications/ClaudeStatus.app` while installing `/Applications/AgentStatus.app`.

---

## 031 — Codex live-state fallback

**Date:** 2026-07-09
**Status:** Accepted

**Context.** During the AgentStatus rename work, the visible light for this active Codex thread
stayed gray. Investigation showed the status directory had only an old idle file from the deleted
`/Users/gabrielchan/Documents/code/ClaudeStatus` path, while the current Codex thread was actively
updating `~/.codex/state_5.sqlite`. No hook-written status files were being refreshed for the
already-running thread, likely because hooks need to be loaded/trusted at thread startup.

**Decision.** Keep lifecycle hooks as the primary signal, but add a best-effort Codex fallback in
`list_sessions`: query `~/.codex/state_5.sqlite` with `/usr/bin/sqlite3`, read recent
`threads.updated_at` rows, and synthesize `ide:"codex"` sessions when no hook status file exists
for that thread. A thread updated in the last 20 seconds renders `running`; otherwise it renders
`idle` until the normal 2h pruning window.

**Stale-file cleanup.** Also prune hook-written status files whose non-empty `cwd` no longer
exists. This removes renamed/deleted-folder ghosts immediately, including the old `ClaudeStatus`
workspace file that was showing as gray after the repo moved to AgentStatus.

---

## 032 — Codex open/close lifecycle: declared host, short idle expiry, process-liveness pruning

**Date:** 2026-07-09
**Status:** Accepted

**Context.** Opening/closing Codex conversations didn't affect the bar: a dot appeared only
when a conversation's first prompt was sent, and closing a conversation left its dot for up
to 2 hours. Investigation (against the installed `openai.chatgpt` VS Code extension binary
26.623.141536 and the `openai/codex` source) established that Codex provides **no open/close
signal anywhere**:

- The binary's complete hook event set (`HookEventsToml`) is `PreToolUse, PermissionRequest,
  PostToolUse, PreCompact, PostCompact, SessionStart, UserPromptSubmit, SubagentStart,
  SubagentStop, Stop` — **no `SessionEnd`**, confirming #029's manual reading.
- `SessionStart` hooks are deferred until the first turn (`run_pending_session_start_hooks`),
  so no hook fires on conversation open. A dot at first prompt is the earliest Codex allows.
- In `~/.codex/state_5.sqlite`, both `threads.updated_at` and `threads.recency_at` advance
  only on `TurnStarted` (verified in `codex-rs/thread-store/src/thread_metadata_sync.rs`).
  Opening or closing a conversation writes nothing to disk.

Two AgentStatus bugs compounded this. (1) Codex hook payloads are Claude-shaped
(`session_id` + `cwd` both present), so all three #029 `$isCodex` payload heuristics were
dead code — live Codex sessions were tagged `ide:"vscode"` (observed on a real thread). The
`cwd`-empty heuristic could even mis-tag a Claude payload. (2) The #031 fallback synthesized
a dot for any thread updated in the last 2h, open or closed. Also, this machine's Codex is
the VS Code extension — there is no `Codex.app`, so #029's click-to-focus (`open -a Codex`)
silently did nothing.

**Decision.**
- **Declared host, not sniffed:** both installers (`hooks/setup.mjs`, app `install.rs`)
  register Codex hooks as `report.sh <Event> codex`; `report.sh` takes the ide from arg 2
  and the payload heuristics are removed.
- **Short idle expiry (user-approved):** Codex lights (hook-written and synthesized) expire
  after `CODEX_IDLE_SECS = 10 min` without turn activity, instead of `MAX_IDLE_SECS = 2h` —
  a closed conversation can only expire by timeout, and a wrong light is worse than no light
  (UI principle 4). An open-but-idle conversation's dot also fades and reappears on its next
  turn. Alternatives offered: activity-only (~90s) dots, or keeping 2h with quit-only pruning.
- **Process-liveness pruning:** if no `codex` process is alive (`pgrep -x codex` covers the
  extension's `codex app-server` and the terminal TUI; fails open on pgrep errors), all Codex
  lights drop immediately and their status files are deleted.
- **Fallback hygiene:** the #031 synthesized query now also excludes `archived = 1` threads.
- **Click-to-focus:** Codex lights focus the VS Code window holding the thread's workspace
  (same `code` CLI path as `ide:"vscode"`); the Claude extension focus-relay request is
  skipped since Codex sessions aren't Claude sessions. Lock-based window pruning still does
  not apply to Codex (a terminal TUI session's cwd need not be open in any VS Code window).

**Validation.** `bash -n`, `node --check`, `cargo check` clean; smoke tests against a temp
status dir verified `report.sh <Event> codex` writes `ide:"codex"`, no-arg writes
`ide:"vscode"`, Cursor detection is unchanged, and a Codex `Stop` (id-only payload) carries
`cwd`/`ide` forward. Rebuilt and reinstalled via `./install.sh`; the app self-installer
rewrote `~/.codex/hooks.json` with the `codex` arg.

---

## 033 — Antigravity IDE as a fourth host; host-gated transcript reads

**Date:** 2026-07-15
**Status:** Accepted, unverified against a live install

**Context.** Support for Google's Antigravity IDE (the `~/.gemini` agent) shipped in commit
`3195f11` without a decision entry, alongside the #032 Codex work. This entry is retroactive:
it records what was built and why, and fixes the one place the new code contradicted #032.

Antigravity is the first host whose hook config is **not** Claude-shaped, so the existing
`merge_hooks` path (which writes a `hooks` map of `{event: [{matcher, hooks}]}`) does not
apply. It is also the first host that **sends no prompt text** on its prompt-submit event,
which is why the task label has to come from somewhere else.

**Decision.**

- **Separate installer path.** `~/.gemini/config/hooks.json` uses a top-level `agentstatus`
  object key (`{enabled: true, PreInvocation: [...], PreToolUse: [...], ...}`) rather than
  Claude's `hooks` map, so both installers get a dedicated writer
  (`merge_antigravity_hooks` in `install.rs`, `installAntigravityHooks` in `setup.mjs`)
  instead of a parameterization of the Claude path. Same guarantees as the other hosts:
  one-time `.agentstatus-bak` backup, idempotent rewrite, `uninstall` deletes the
  `agentstatus` key and nothing else. `PreToolUse`/`PostToolUse` use Antigravity's regex
  matcher `".*"` (Claude's glob `"*"` is not the same dialect).
- **Declared host, per #032.** Hooks register as `report.sh <Event> antigravity`; the ide
  comes from arg 2. No payload sniffing, for the same reason as Codex.
- **Event→state mapping.** `PreInvocation`/`PostInvocation` → `running`, joining the
  existing `UserPromptSubmit`/`PreToolUse`/`PostToolUse` → `running` set; `Stop` → `idle`.
  Antigravity has no permission-request or turn-failure event registered, so `blocked` and
  `error` are currently unreachable for this host — its lights are green/gray only.
- **Payload differences.** cwd from `workspacePaths[0]` (not `.cwd`); tool name from
  `toolCall.name`; tool args from `toolCall.args.CommandLine` / `.TargetFile` /
  `.AbsolutePath`. Its tool names (`run_command`, `write_to_file`, `view_file`, …) are
  folded into the existing Bash/Edit/Read detail branches.
- **Task label from the transcript, host-gated.** `PreInvocation` carries no prompt, so the
  last `USER_INPUT` record is read from
  `~/.gemini/antigravity/brain/<sid>/.system_generated/logs/transcript_full.jsonl` and, if
  it is wrapped in `<USER_REQUEST>…</USER_REQUEST>`, unwrapped to the bare request. **This
  read is gated on `IDE_ARG = antigravity`.** As first shipped it was gated only on the
  event name — and `UserPromptSubmit` is *Claude's* event, so in every Claude session the
  fallback chain missed the two `.gemini` paths and landed on the real Claude transcript,
  spawning `python3` over it on every turn. Measured on a 10 MB transcript: **137 ms/call
  → 39 ms/call once gated** (~98 ms saved per prompt submit). The work was entirely
  wasted — the parser scans for Antigravity's `USER_INPUT` records, which Claude
  transcripts never contain (grep: 0 matches), and jq prefers the payload's `.prompt`
  anyway. It violated Guideline #3 (hooks must be fast) and #5 (don't read transcript
  bodies) for zero benefit.
- **Pruning and focus.** Antigravity is added to `uses_ide_locks` (vscode/cursor) — it is a
  VS Code fork with the same `~/.claude/ide/*.lock` liveness signal — and keeps the 2h
  `MAX_IDLE_SECS` backstop. Codex's short `CODEX_IDLE_SECS` + process-liveness rules
  (#032) do **not** apply, because those exist only to work around Codex's missing
  close signal. Click-to-focus raises `Antigravity IDE.app` via
  `/Applications/Antigravity IDE.app/Contents/Resources/app/bin/antigravity-ide`. Like
  Codex, the Claude extension focus-relay is not used — an Antigravity session is not a
  Claude session, so focus lands on the window, not the exact tab.

**Alternatives considered.** Generalizing `merge_hooks` to emit both config shapes — rejected;
the two schemas share no structure, and a flag-driven writer would be harder to read than two
explicit ones (Karpathy: no over-configurability). Dropping the task label for Antigravity
rather than reading its transcript — viable and cheaper, but the label is what makes a light
tell you *which* work is running; gating the read preserves it at no cost to other hosts.

**Validation.** `bash -n` clean. Smoke-tested against a temp status dir: a Claude
`UserPromptSubmit` carrying a real 10 MB `transcript_path` now takes the payload prompt with
no `python3` spawn (`ide:"vscode"`, task from `.prompt`), and an Antigravity `PreInvocation`
with no prompt field still recovers its task from a synthetic `transcript_full.jsonl` and
unwraps `<USER_REQUEST>` (`ide:"antigravity"`).

**Open — Guideline #4.** The Antigravity event names, payload field names, hook-config schema,
and transcript path/format above are all **unverified against a running Antigravity install**;
they were committed without an observed event log. Before relying on this host, capture real
events from a live session and confirm each. The `_full.jsonl`-then-`.jsonl` transcript
fallback in particular reads as a guess. The `PostInvocation` → `running` mapping is dead code
today: no installer registers that event.

---

## 034 — README lightbar visuals: reproducible SVG art from a generator

**Context.** The README described the light bar entirely in prose and a state table but
carried no picture — a poor fit for a product whose whole pitch is a *glanceable* row of
lights. We wanted screenshots/a walkthrough without violating Guideline #8 (everything
replicable, no one-off manual steps).

**Options considered.**

| Option | Authenticity | Reproducible | Effort/upkeep |
|---|---|---|---|
| Real screenshots of the app | Highest — real desktop | ❌ Manual re-capture, needs sessions staged in each state | Manual every time |
| **Rendered SVG mockup from a generator** | High — pixel-accurate lights on a neutral backdrop | ✅ One `node` re-run, values mirror `styles.css` | Low |
| Hybrid (real hero + generated diagrams) | Highest hero | ⚠️ Hero manual | Mixed |

**Decision.** Generate the art with `docs/gen-readme-art.mjs`, committing five
self-contained SVGs — `docs/lightbar-hero.svg` (a realistic mixed-state bar),
`lightbar-states.svg` (every state labeled), `lightbar-hover.svg` (a light with its subagent
badge + hover tooltip), `lightbar-orientation.svg` (the same bar horizontal vs vertical), and
`lightbar-settings.svg` (the right-click settings panel — orientation/sort toggles,
size/padding/opacity sliders, per-state color swatches, footer links). The generator mirrors
the exact values from `app/src/styles.css` (dot geometry, the five per-state colors, glow
alphas, the `oklch(60% 0.15 262)` badge accent → `#4c7dd9`, and the panel's OKLCH `--ui-*`
neutrals → sRGB), so the pictures stay faithful to the real UI and regenerate with one command
after any visual change.

**Reasoning.** Satisfies Guideline #8 — the pictures are a build artifact, not a manual
capture. SVG is self-contained (no headless-browser dependency), GitHub-renderable inline,
and each file bakes in its own dark backdrop so the frosted pill and the white "done" light
read correctly in both GitHub light and dark themes. The one thing this can't show is the
bar floating over the *real* desktop; the faint window panes in the hero stand in for that.

**Limits.** GitHub's SVG sanitizer strips CSS animation, so the pulsing states
(blocked/error) render static and are labeled "(pulsing)". The art is a facsimile built
from the stylesheet, not a screenshot of the running app — if the two ever drift, re-run the
generator (it reads the intended values, so update it alongside `styles.css`).

---

## 035 — Settings: audio alerts (per-state chimes)

**Date:** 2026-07-28
**Status:** Accepted

**Context:** The lights are silent — a session going orange (blocked) or red (error) is
only noticed if you're looking at the bar. UI Principle #2 says attention states must never
be missed; a sound is the natural way to catch them when your eyes are on the code. The
question was what triggers a sound and how to expose the controls without cluttering the
one-screen settings panel.

**Options — where the controls live:**

| Option | Pros | Cons |
|---|---|---|
| Separate OS-window popup for audio settings | Room to grow | Needs new Rust window lifecycle + positioning; a non-activating NSPanel spawning a focused child window fights the single "hugging pill" model (#008); heavy for a few toggles |
| **Inline disclosure in the existing panel** | Reuses the #015 conditional-row pattern (`#condense-row`), `.seg`, `.crow`, and the range slider; window auto-regrows to hug it; zero Rust | Panel grows taller when expanded (acceptable — it already resizes) |

**Decision.** Add an **Audio** On/Off `.seg` toggle to the settings panel. When On it reveals
an inline `#audio-panel` sub-block — per-state chime checkboxes (**Blocked**, **Error**,
**Done**) and a **Volume** slider — the same disclosure pattern the menu-bar Condense control
already uses. Chimes are short **WebAudio** oscillator tones (blocked = rising two-note,
error = lower urgent double, done = single soft note); no bundled audio asset, so nothing to
load and no CSP concern. Dispatch is **edge-triggered**: a `prevChimeState` map records each
session's `displayState`, and a chime fires only when it *changes into* blocked/error/done —
a light that stays orange beeps once on arrival, not every poll. The map is seeded silently on
the first poll (`audioSeeded` guard) so pre-existing blocked/error sessions don't blast on
launch. Off by default (UI Principle #1). Toggling a state checkbox or moving the volume
slider plays a preview. All frontend-only in `localStorage` (`agentstatus.audio` /
`.chimes` / `.volume`); the hook and status files are untouched.

**Reasoning.** Blocked and error are exactly the states the user must act on, and "done"
covers walking away from a finishing turn — all three were requested, each independently
toggleable per the user's ask. WebAudio keeps it dependency-free and asset-free. Edge-trigger
+ silent seeding is what makes it non-intrusive: no repeat nagging, no startup blast. The
inline disclosure keeps the whole surface in one right-click panel, consistent with every
other setting, and needs no backend changes.

**Limits.** Sound plays only while the app (webview) is running — a chime is not an OS
notification and won't fire if the app is quit. Audio requires the webview's AudioContext,
created lazily on first play; if the platform blocks it, `playChime` fails silently.

---

## 036 — App icon redesign: three status lights on a dark squircle

**Context.** The bundled app icon was a cyan/yellow abstract "8"-ish swirl left over from the
Tauri template — it said nothing about the product. Everything else about AgentStatus is *a row
of colored status lights*; the Dock/Finder/menu-bar icon should be the same visual so the app is
recognizable at a glance. We also wanted the new icon reproducible (Guideline #8), not a
hand-painted one-off.

**How it was made.** Generated five candidate directions as self-contained SVGs (a subagent
authored them with the exact palette mirrored from `styles.css`/`gen-readme-art.mjs`, rasterized
via `cairosvg`, each proofed at true 32×32 for legibility — a diffusion model was not used, since
a geometric dot icon is exactly what SVG does deterministically and on-brand). The user reviewed
all five plus a sixth ChatGPT-generated option and chose the **three-large-dot** direction.

**Options considered (the candidates):**

| Option | Design | 32px legibility |
|---|---|---|
| 5 dots (all states) | Fullest "one light per session" story | Weak — dots ~3px, gray idle dot vanishes |
| **3 dots — green/orange/red** ✓ | Running/blocked/error, the attention states | **Best — three clearly distinct lights** |
| Single green bloom | Minimal | Clean but reads as a generic "online" dot |
| 3 dots in the frosted pill | Strongest product identity at full size | Pill fades to a dark bar when small |
| Vertical column | Orientation-flip nod | Reads like a traffic light |

**Decision.** Ship the **three-dot** icon: green `#2ecc71`, orange `#f39c12`, red `#e74c3c`
dots with the README-art radial bloom, on a top-down `#1b2733`→`#0f1720` panel with a 180px
Big-Sur corner and transparent corners (so macOS renders it as a rounded icon, not a dark
square). The vector master lives at `docs/icon-master.svg` (+ a 1024px `docs/icon-master.png`);
`cd app && npx tauri icon ../docs/icon-master.png` regenerates the whole macOS icon set, and a
256px `sips` export becomes `docs/logo.png` — the README header logo — so one source drives both.
All six review candidates and the generation scaffold were deleted after the pick.

**Reasoning.** The three signal colors *are* the product's identity and are the states a user
acts on, so the icon doubles as a legend. Three dots is the most that stay individually
distinct at 32×32 (the smallest generated size and the hardest constraint), which is why the
full 5-state and the pill variants lost despite telling a richer story at 1024px. SVG master +
`tauri icon` keeps it reproducible and editable, and sharing the master with the README logo
means the app icon and the docs can never drift apart.

**Limits.** The three-dot icon shows only running/blocked/error — it omits done (white) and idle
(gray), which read poorly small anyway; the icon is a brand mark, not the live legend (the bar
itself and the README states art cover all five). The macOS Dock/Finder icon is cached, so the
new icon appears after the app is rebuilt/reinstalled (login cycle may be needed for stale
caches). Tauri's `icon` command also emits iOS/Android assets; those are pruned — this is a
macOS-only app and `tauri.conf.json` references only the five desktop icons.

## 037 — Overlay panel collection behavior via objc2-app-kit; install module release-gated

**Date:** 2026-07-28
**Status:** Accepted

**Context:** `cargo build` (and `tauri dev`) emitted a batch of warnings, in two families:
a large "never used" group covering the entire `install` module (every function and
constant), and 5 `deprecated` warnings on `NSWindowCollectionBehavior` in
`make_overlay_panel`. Neither reflected a real defect, but the noise buried genuine
warnings and made the dev build look broken.

**Root causes.**
- The `install` module's only entry point, `install::ensure_installed()`, is called
  solely under `#[cfg(not(debug_assertions))]` (the self-installer is release-only by
  design — dev uses the repo hooks via `node hooks/setup.mjs`, decision 011). In a debug
  build nothing references the module, so Rust flagged all of it dead. The
  `let mut builder` in `run()` warned the same way — its only reassignment (the
  single-instance plugin, decision 020) is also release-gated.
- `tauri-nspanel`'s `set_collection_behaviour()` takes the deprecated `cocoa` crate's
  `NSWindowCollectionBehavior` as its parameter type, so any call through it forces our
  crate to name the deprecated type. `tauri-nspanel` silences this internally with a
  crate-level `#![allow(deprecated)]`; our crate does not.

**Options considered (deprecated warnings):**

| Option | Pros | Cons |
|---|---|---|
| `#[allow(deprecated)]` on `make_overlay_panel` | One line, zero risk | Keeps us on the unmaintained `cocoa` path; hides a real future-migration signal |
| Migrate to `objc2-app-kit` (**chosen**) | Moves to the maintained, typed API Tauri already depends on; no deprecation | Bypasses `set_collection_behaviour` (its param is cocoa-typed), so we set the flag on the underlying NSWindow ourselves |
| Leave them | No work | 5 warnings persist on every build |

**Decision.**
- Gate `mod install;` to `#[cfg(not(debug_assertions))]`, matching how its only caller is
  already gated. `#[allow(unused_mut)]` on `let mut builder` for the same release-only
  reason. This removes the whole dead-code family at the source rather than papering over
  working code with `#[allow(dead_code)]`.
- Set the panel's collection behavior through `objc2-app-kit`: fetch the window's NSWindow
  pointer via Tauri's `WebviewWindow::ns_window()` and call the typed, non-deprecated
  `NSWindow::setCollectionBehavior(FullScreenAuxiliary | CanJoinAllSpaces)`. The panel
  is-a NSWindow (NSPanel inherits it), so this is the same object and the same two flags as
  the old `set_collection_behaviour` call — behavior is unchanged (Guideline #7). Added
  `objc2-app-kit` as a direct macOS-only dependency, pinned to `0.3` (the version Tauri
  already pulls in transitively via `muda`, so no second copy), with the `NSWindow` and
  `NSResponder` features.

**Reasoning.** Both fixes remove the warning by making the code honestly reflect the build:
the installer genuinely isn't part of a dev build, and the collection-behavior call
genuinely doesn't need the deprecated crate — Tauri already ships `objc2-app-kit`, so
using it adds no new transitive weight and moves us onto the maintained API. The `app`
crate now builds with zero warnings.

**Limits.** One transitive warning remains outside our code — `block v0.1.6` (pulled by
`tauri-nspanel`) is flagged as "will be rejected by a future version of Rust"; it clears
when `tauri-nspanel` updates its dependencies, not from anything in this repo. The
`objc2-app-kit` call path is macOS-only (`#[cfg(target_os = "macos")]`), unchanged from the
old code.

## 038 — Cursor: fix stale-pruning + mirror the menu-bar attention item

**Date:** 2026-07-28
**Status:** Accepted (fixes the decision-018 Cursor integration; refines decision-027 pruning)

**Context.** The Cursor integration (decision 018) had stopped producing lights. Diagnosed
two independent problems, one fatal:

1. **Pruning deleted every Cursor light.** Decision 027 prunes a session the instant its
   workspace maps to no live `~/.claude/ide/*.lock`. Those lock files are written **only by
   Claude Code's VS Code extension** — Cursor's native agent writes none (verified: every
   lock on the machine was `ideName: "Visual Studio Code"`, none Cursor). But
   `list_sessions` had `ide == "cursor"` in its `uses_ide_locks` set, so a Cursor session's
   `cwd` matched no live lock and was deleted on the next poll whenever *any* VS Code window
   was open — i.e. essentially always. The hooks fired correctly (bridged
   `beforeSubmitPrompt`/`stop` → `report.sh`, exit 0, verified in Cursor's hook logs); the
   display layer nuked the result a poll later.
2. **Cursor sessions can never light as "done".** The "done" attention state (decision 014)
   is `idle && detail != ""`, and `detail` is the turn's wrap-up message. Cursor's bridged
   `Stop` carries no assistant message (verified by replaying a synthetic Cursor payload
   through `report.sh` — `detail` came out `""`), so a finished Cursor turn only ever renders
   as plain dim idle. The user never gets a bright "this Cursor agent is waiting for you" cue.

**What the menu-bar item actually is (investigated in the Cursor 3.12.10 bundle).** The
user asked to "wire the Cursor menu bar item into the lightbar." Reading `out/main.js`: the
tray (`TrayMainService`) has only **two** icon states (`trayTemplate` vs `trayNotifyWhite`)
plus a numeric title showing an **unread notification count**, driven by a per-window
composer snapshot merged across windows (`hasNotification`/`unreadCount` per composer). It is
a **notification/attention indicator, not a live "agent working" spinner** — there is no
running/generating state in the tray. Live status (`chatStatus`: generating/applying/running;
`composerStatus`: the `BackgroundComposerStatus` enum) lives **only in renderer memory**; the
persisted `composerData.status` in `state.vscdb` is terminal-only (`none`/`aborted`) and
stale. So the only externally observable, real-time attention signal Cursor exposes is the
menu-bar item's aggregate count.

**Options considered.**

| Option | Pros | Cons |
| --- | --- | --- |
| Read the menu-bar count as the *primary* Cursor signal | Simple; robust to internals | Aggregate only (no per-window, no green/running, no click-to-focus a specific session) — a downgrade from VS Code parity |
| Fix the hooks only | Restores per-session running/idle + click-to-focus | Cursor still never shows a "done"/attention cue (no wrap-up, no permission event) |
| **Both (chosen)** | Per-session live lights from the (now un-pruned) hooks **and** the menu-bar count as a supplementary attention pip covering the one bit hooks can't | Two mechanisms; the pip needs Accessibility permission |

**Decision.**
- **Pruning (Rust `list_sessions`):** remove `cursor` from `uses_ide_locks` so Cursor
  sessions are never lock-pruned. Prune them like Codex instead — dropped when no `Cursor`
  process is alive (`cursor_running()`, `pgrep -x Cursor`, fails open) — with the existing
  `MAX_IDLE_SECS` (2h) idle backstop and the `cwd`-gone check still applying. Clean closes are
  still handled by the bridged `SessionEnd` → file delete.
- **Menu-bar pip (Rust `cursor_attention_count` + frontend):** a new command reads Cursor's
  status item via the **Accessibility API directly** — `AXUIElementCreateApplication(pid)` →
  `AXExtrasMenuBar` → `AXChildren` → the first child whose `AXTitle` carries a digit — and
  fails **closed to 0** on any error (Cursor not running, item absent, AX not granted). It
  parses the count from the title `" N"`; it does **not** filter on `AXDescription` (a Swift AX
  probe showed the item's `AXDescription` is `nil` — AppleScript's "description" was a different
  attribute; an early cut matched `"status menu"` and always returned 0). Chosen over
  `osascript → System Events`, which additionally needs the **Automation** permission an
  unsigned rebuild lacks, so it read nothing. The frontend polls it on a gentle ~20s cadence
  (see below) and renders **one** pip (`.cursor-pip`, a hollow ring + count badge) as the last bar element when
  count > 0 — styled unlike a session light so it reads as the Cursor menu-bar mirror, not a
  specific session. Clicking it activates `Cursor.app` (`focus_session` with an empty `cwd`;
  the JS `focusSession` no-op-on-empty-cwd guard is relaxed for `ide == "cursor"`). No
  status-file/schema change — display-only, like the `reviewedAt` map. The frontend polls the
  count on a **gentle ~20s cadence** — the AX read can dismiss Cursor's own menu-bar popover if
  it lands while the user has it open (observed live), and the count changes seldom, so a slow
  poll keeps the pip fresh without interfering with Cursor's menu bar (Guideline #3).

**Reasoning.** The pruning change is the actual fix and restores real per-session Cursor
lights + click-to-focus at VS Code parity (minus blocked, which Cursor still has no event
for). The menu-bar pip adds back the only attention signal Cursor exposes externally, so a
finished/awaiting Cursor composer is glanceable even though its hooks carry no wrap-up — at
the honest cost of being an aggregate (it never claims to be one session). Reading via the AX
API needs only the Accessibility grant decision 021 already documents (not Automation), and
degrades to "no pip" without it, so it never blocks or lies. Making that grant *stick* across
rebuilds required stable signing — see decision 039.

**Validation.** (1) Wrote a synthetic `ide:"cursor"` session whose `cwd` is a real folder
*not* in any IDE lock, against the running app: under the old code it vanished within a poll;
after the fix it **survived** across multiple polls. (2) Replayed synthetic Cursor
`beforeSubmitPrompt`/`stop` payloads through `report.sh`: correct `ide:"cursor"`, `cwd` from
`workspace_roots[0]`, running→idle, `detail` empty (confirming the "done" gap). (3) A
standalone Swift AX probe from a trusted context returned the item's title `" 1"` (and
`AXDescription = nil`), validating the traversal. (4) With the app signed (decision 039) and
Accessibility granted, a marker-gated debug log in the running app read `trusted=true count=1`
and the **pip appeared live** on the bar; trust then **persisted across a rebuild**, confirming
signing solved the reset loop. **Verified with the user:** the pip shows the Cursor menu-bar
count. (Per-session Cursor lights still require a folder-open Cursor window — Cursor runs no
hooks in a folder-less window: `MainThreadShellExec not initialized`.)

## 039 — Stable self-signed code signing so Accessibility trust survives rebuilds

**Date:** 2026-07-28
**Status:** Accepted (revises the "unsigned" trade-off in decisions 011 and 024, for local builds)

**Context.** The Cursor menu-bar pip (decision 038) reads another app's UI via the
Accessibility API, which requires AgentStatus to hold a macOS Accessibility (TCC) grant. In
practice the grant never stuck: after granting, `AXIsProcessTrusted()` still returned `false`,
so `cursor_attention_count` read 0 and the pip never appeared. Root cause: `tauri build`
**ad-hoc signs** the app (`flags=0x2 adhoc`), and an ad-hoc signature's Designated Requirement
(DR) is its **code hash**. Every rebuild produces a new hash → a new DR → macOS treats it as a
different app and invalidates the prior grant. During iteration each `./install.sh` silently
reset trust; the user was granting build N while the next build replaced it. Even a single
grant was fragile (a stale entry pinned to an old hash).

**Options considered.**

| Option | Pros | Cons |
| --- | --- | --- |
| Keep ad-hoc, re-grant each build | No new machinery | Trust resets every rebuild/update — unusable for a feature that depends on it |
| `osascript`-only, guide user to grant Automation too | Small | Automation is *also* hash-bound; same reset problem, plus a second permission |
| **Self-signed identity, sign every build (chosen)** | Stable DR → grant once, persists across all future rebuilds/updates; unblocks free rebuilding | Creates a per-machine keychain cert; revises the unsigned-distribution stance |

**Decision.** `hooks/sign-app.sh` (run by `install.sh` after copying to `/Applications`)
ensures a self-signed code-signing identity **"AgentStatus Self-Signed"** exists (created once
via `openssl` with a `codeSigning` EKU, imported into the login keychain with
`-T /usr/bin/codesign`, trusted for the code-signing policy) and re-signs the bundle with it:
`codesign --force --deep --sign "AgentStatus Self-Signed"`. The resulting DR is stable —
`identifier "com.agentstatus.app" and certificate leaf = H"b0c976…"` — so TCC keys on the
signing identity, not the code hash. Grant Accessibility once and it survives every rebuild.
Idempotent (reuses the cert; re-signs each time); undo with
`security delete-identity -c "AgentStatus Self-Signed"`.

**Trade-offs / notes.**
- **Distribution unchanged.** A self-signed anchor is per-machine and does nothing for
  Gatekeeper on a *downloaded* copy — those still clear quarantine as decision 024 describes.
  Signing only stabilizes the on-device identity for TCC on locally-built installs. So this
  refines 011/024 for the local-build path without changing the download story.
- **App startup also prompts** for Accessibility (`AXIsProcessTrustedWithOptions`, release
  only) so the grant is discoverable; combined with stable signing, that prompt appears once.
- **One-time keychain touch.** Creating/trusting the cert may prompt for the login password
  once. `install.sh` warns (not fails) if signing can't complete, degrading to ad-hoc.
- **OpenSSL 3 gotcha:** the PKCS#12 the cert is imported through must be written with `-legacy`
  and a non-empty transient password, or macOS's `security import` rejects the MAC
  ("MAC verification failed").

**Validation.** After signing, `codesign --verify --deep --strict` passes and the DR shows the
stable leaf. `tccutil reset Accessibility com.agentstatus.app` cleared the stale ad-hoc
entries; a single fresh grant then flipped the running app's marker-gated log to
`trusted=true count=1` (live, no relaunch), the pip appeared, and — the key result — trust
**persisted across a subsequent `./install.sh` rebuild** (log still `trusted=true` afterward),
which ad-hoc never did.


---

## 040 — Remove Codex and Antigravity support

**Date:** 2026-07-29
**Status:** Accepted (reverses #029, #031, #032 for Codex; #033 for Antigravity)

**Context.** AgentStatus shipped four hosts: Claude Code (VS Code), Cursor, Codex, and
Antigravity. Only the first two were ever verified against a live install. Guideline #4
requires confirming an event actually fires with the expected shape before building on it,
and decision #033 was logged as **"Accepted, unverified"** in its own status column. The
Codex path was built on inference rather than observation too: because its lifecycle events
could not be relied on, the display layer compensated with a `~/.codex/state_5.sqlite`
read, a `pgrep -x codex` liveness probe, and a bespoke 10-minute idle timeout — three
mechanisms whose only purpose was to guess at a signal the hooks weren't confirmed to
deliver.

An unverified host cannot produce a trustworthy light, and a wrong light is worse than no
light (UI Principle #4). Carrying both also spread host-specific branching through every
layer — the hook's jq program, the pruning logic, click-to-focus, and both installers.

**Options considered.**

| Option | Pros | Cons |
|---|---|---|
| Keep as-is | No work; the code exists | Ships unverified hosts that can show wrong state; host branching taxes every future change to the hook and the pruner |
| Keep the code but gate it behind a setting | Reversible without a re-implementation | Same unverified lights, now with a switch; more surface, not less |
| **Remove, and clean up prior installs** (chosen) | Every remaining light comes from a verified host; the hook and pruner drop back to the Claude/Cursor shapes; no orphaned hooks left behind | Re-adding either host means re-implementing it — deliberately, against a live install |

**Decision.** Remove both hosts, and have the installers clean up what earlier versions
wrote.

- **`hooks/report.sh`** — dropped the declared-host `$2` argument (no host besides Claude and
  Cursor is registered now, and Cursor is sniffed from `cursor_version` in the payload), the
  Codex thread/conversation id fallbacks, the `env.PWD` cwd fallback, the
  `PreInvocation`/`PostInvocation` state mappings, Antigravity's `workspacePaths[]` /
  `toolCall.*` payload shapes and its aliased tool names, the camelCase
  `lastAssistantMessage`/`message` fallbacks, and the `<USER_REQUEST>` unwrap. This also
  removes the transcript read and its `python3` spawn — **the hook no longer reads any
  transcript at all**, which is the stronger form of Guideline #5.
- **`hooks/setup.mjs` and `app/src-tauri/src/install.rs`** — install into
  `~/.claude/settings.json` only.
- **`app/src-tauri/src/lib.rs`** — removed `read_codex_threads` and the sqlite read,
  `codex_running`, `label_from_cwd_or_title`, `CODEX_ACTIVE_SECS`/`CODEX_IDLE_SECS`, the
  Codex/Antigravity pruning branches, and their `focus_session`/`raise_window_fast` targets.
  Every session now prunes on the single `MAX_IDLE_SECS` backstop plus its host's liveness
  rule (IDE locks for vscode, process liveness for cursor).

**Cleanup of prior installs.** Deleting the install code alone would strand hook entries in
`~/.codex/hooks.json` and `~/.gemini/config/hooks.json` on any machine that ran an earlier
build — they would keep invoking `report.sh` from a host it no longer understands, writing
status files that produce exactly the mislabeled lights this decision removes. Both
installers therefore run a cleanup on install *and* uninstall: Codex entries are filtered
out of its Claude-shaped `hooks` map by the same `report.sh` marker the installer uses, and
Antigravity's top-level `agentstatus` key is deleted. Neither path creates a file that
isn't already present, and hooks belonging to anything else in those files are preserved
(Guideline #3: idempotent, reversible, non-clobbering).

**Trade-offs / notes.**
- **History preserved.** Decisions #029/#031/#032/#033 stay in this file. They record what
  was tried and what was learned about each host's lifecycle — the input any future
  re-implementation should start from.
- **Reinstating a host is deliberate work.** That is the point: it should mean logging real
  events from a real session first (Guideline #4), not restoring code.
- **Validation.** `cargo check` clean in both debug and release (release compiles
  `install.rs`; the only warning is the pre-existing upstream `block v0.1.6` notice).
  `report.sh` re-verified end to end against a scratch `$AGENTSTATUS_DIR`: running →
  blocked → idle for Claude, plus a Cursor `beforeSubmitPrompt` producing `ide:"cursor"`
  with the workspace from `workspace_roots[]`, and `SessionEnd` removing the file. **Both**
  installers were then exercised against a throwaway `$HOME` seeded with an old install —
  `setup.mjs` directly, and `install.rs` through a scratch binary compiling the real file
  (byte-identical but for the `include_str!` path, since the module is release-gated inside
  the Tauri crate). Same result from each: our Codex and Antigravity entries removed, a
  foreign hook in the same `~/.codex/hooks.json`, a sibling `otherSetting` key, and an
  unrelated `otherplugin` key in the Antigravity config all preserved, emptied event keys
  dropped, a second run a no-op, and a run with neither legacy file present creating
  nothing.


---

## 041 — Automate releases with a tag-triggered GitHub Actions workflow

**Date:** 2026-07-29
**Status:** Accepted

**Context.** Every release through v0.4.2 was cut by hand: build locally, find the DMG in
`app/src-tauri/target/release/bundle/dmg/`, create the GitHub release, upload, write notes.
That is a manual, unreplicable step of exactly the kind Guideline #8 says to script away,
and it makes the published artifact depend on whatever state one machine happened to be in.
The repo had no CI at all.

**Options considered.**

| Option | Pros | Cons |
|---|---|---|
| Publish on every push to `main` | Zero ceremony — merge and it ships | A README typo cuts a release; duplicate-version releases have to be hand-deleted. Merging is routine, releasing is not — conflating them removes the moment where you decide the build is worth shipping |
| Push to `main`, gated on the version changing | Still ceremony-free, and doc pushes are free | The trigger fires constantly and no-ops nearly every time, so a genuine failure is easy to miss in a sea of skipped runs. The real intent (release now) is inferred from a file diff rather than stated |
| **Push a `v*` tag** (chosen) | Releasing is explicit and deliberate; the tag is a permanent audit trail; the workflow runs only when a release is actually wanted, so a red run always means something | Releasing is a second command after merging (`git tag v0.5.0 && git push origin v0.5.0`) |

**Decision.** Trigger on `push: tags: ["v*"]`. `macos-15` (Apple Silicon) runs
`npm ci && npm run tauri build`, and `gh release create` publishes the DMG with
`--generate-notes`. `contents: write` is the only permission granted.

**The version guard.** A tag-driven release has one distinctive failure mode: the tag and
the version baked into the bundle can disagree, producing `v0.5.0` release that contains
`AgentStatus_0.4.9_aarch64.dmg`. So the first step compares `${GITHUB_REF_NAME#v}` against
`app/src-tauri/tauri.conf.json` and fails the run with an actionable message if they differ
— before spending runner minutes on a build that would have to be deleted anyway.

**Trade-offs / notes.**
- **Artifacts stay unsigned and un-notarized.** GitHub's runners hold no Developer ID
  certificate, so the DMG is exactly what the manual process produced and the README's
  Gatekeeper step still applies (decisions 011/024). The stable self-signing of #039 is for
  build-from-source installs and is deliberately absent here — a per-machine self-signed
  anchor would mean nothing to a downloader.
- **`Cargo.lock` is committed**, so `Swatinem/rust-cache` restores an exact-match cache and
  the release build resolves the same dependency versions as a local one.
- **Not verified against a real run yet.** The workflow's YAML, its trigger, and the version
  guard's shell logic were checked locally (guard tested against both a matching and a
  mismatched tag), but no tag has been pushed, so the build-and-publish path is unproven on
  a runner. Guideline #4's habit applies to CI too: treat the first tagged release as the
  verification, and watch that run rather than assuming it.

---

## 042 — Hollow "unknown" light for sessions we get no signal from

**Date:** 2026-07-30
**Status:** Accepted
**Evidence:** Cursor **3.12.10**, live — session `3c8f4449…` and Cursor's own hook logs under
`~/Library/Application Support/Cursor/logs/**/cursor.hooks.*.log`.

**Context.** The user reported a Cursor session that was actively running but showed a blank,
colorless light on the bar. Its status file was:

```json
{"state":"idle","cwd":"","ide":"cursor","label":"","updated_at":1785417092,"task":"","detail":""}
```

Empty `cwd` → no label; `state:"idle"` → a dim gray dot. Nothing was broken in the write path.
The hook log shows why: the **only** event that ever fired for that session was one bridged
`sessionStart`, carrying `"workspace_roots": []` (logged to
`cursor.hooks.workspaceId-empty-window.log`) — a **folder-less Cursor window**. Decision 018
recorded that Cursor runs command hooks only when a workspace folder is open; this quantifies
it. Across every Cursor hook log for the day, the only steps requested were `workspaceOpen`
(18), `sessionStart` (3), and `sessionEnd` (4) — zero `beforeSubmitPrompt`, `preToolUse`, or
`stop`. The previous log generation, from folder-open windows, shows the full stream (1190
`preToolUse`, 59 `beforeSubmitPrompt`, 60 `stop`). So that light was frozen at the state of a
single opening event and could never update, no matter what the composer did.

That makes it a **lying light**, the one thing UI Principle #4 forbids: not wrong about a color
it could have known, but asserting `idle` for a session we have no signal for whatsoever.

**Options considered.**

| Option | Pros | Cons |
|---|---|---|
| **Derive `unknown` in the frontend** (chosen) | No hook change, no schema change; the app already has both fields it needs (`ide`, `cwd`); hooks stay dumb and fast (Guideline #3) | The condition lives in the display layer, so a future host with the same gap needs its own clause |
| Write `observable:false` from `report.sh` | Explicit; generalizes to future hosts | A status-file schema change, and it puts judgment in the hook for something the app can derive from data it already receives |
| Show no light at all (skip the write, like `empty-state-draft`) | No lying light and less clutter | Also no hint the session exists — a running Cursor composer becomes invisible rather than uncertain, which is a worse answer to "which sessions are there?" |

Scope was also a fork: apply the hollow treatment to **any** session gone silent past a
heartbeat timeout (which would finally use the long-dead `.dot.stale` CSS), or only to this
verified case. Chose **only the folder-less Cursor case** — a generic timeout would eventually
render healthy long-idle sessions as hollow, trading one inaccuracy for another.

**Decision.** Frontend-only, in `app/src/main.js`:
- `isUnobservable(s)` = `s.ide === "cursor" && !s.cwd`; `displayState()` returns `"unknown"`
  for it, ahead of the `done` check.
- `.dot.unknown` (`styles.css`) is a **hollow ring**: transparent fill, 2px `--c-idle` border,
  no glow, no pulse, `opacity: .75`. Deliberately quiet — "unknown" is not an attention state
  and must not compete with blocked/error/done (UI Principle #2). Precedent: `.dot.cursor-pip`
  (#038) is already a ring for the same "indicator, not a session light" reason; it stays
  distinguishable by being bright and always carrying a count badge.
- Tooltip states the fact and its cause rather than a state:
  `Cursor <id8> — state unknown` / `↳ no folder open in this Cursor window, so it reports no
  progress · click to open Cursor` (UI Principle #5).
- `URGENCY_RANK` places `unknown` between `running` and `idle`; `TRAY_PRIORITY` gains it in the
  same slot so a bar of only-unknown sessions doesn't condense to the "empty" placeholder; and
  `drawTray()` strokes a ring for it so the menu-bar image matches the bar.
- Not in `CHIME_STATES`, so it never makes a sound. Not a new color swatch — it borrows
  `--c-idle` rather than adding a setting nobody asked for.
- Clicking it activates `Cursor.app` (unchanged behavior: `focusSession`'s empty-`cwd` guard
  already exempts `ide == "cursor"`, from #038).

**Reasoning.** The honest signal for "we cannot see this session" is a light that visibly
withholds a color, and hollow reads that way instantly next to five solid states. Deriving it
in the app keeps the change to ~20 lines with no new schema, no hook risk, and nothing new to
uninstall. Restricting it to `cursor && cwd == ""` means it fires exactly where the absence of
signal is *proven*, so no session that does report its state is ever downgraded to a guess.

**Validation.** Rebuilt and reinstalled via `./install.sh` (which re-signs and relaunches),
then screenshotted the live bar with the real folder-less Cursor session present: the top light
renders as a hollow gray ring where it was previously a solid gray dot, with the green (this
session), white (done), and bright Cursor menu-bar pip lights unchanged beside it. `node
--check` on the frontend; README art regenerated (`node docs/gen-readme-art.mjs`) so
`docs/lightbar-states.svg` documents six states, verified by rendering the SVG.

**Related gaps found, deliberately not addressed.** (1) Cursor **background/cloud** agents run
in a worker extension host with no shell — every hook exits 1 with `Shell execution is not
available in the worker extension host` (observed on a `sessionEnd` today), so they produce no
light at all; the #038 menu-bar pip remains their only signal. (2) `.dot.stale` in `styles.css`
is still dead CSS — no code applies it. A heartbeat-based `unknown` is where it would be used
if that scope is ever revisited.

---

## 044 — Settings: `Unknown` Show/Hide toggle

**Date:** 2026-07-30
**Status:** Accepted (extends #042)

**Context.** Decision 042 added the hollow `unknown` ring for sessions we get no signal from
(today, folder-less Cursor windows). The user asked to make its presence on the bar a choice:
knowing a session exists but is unreadable is useful to some, clutter to others — a folder-less
Cursor window can sit open for hours contributing a permanently uninformative light.

**Decision.** A fourth `.seg` row in the settings panel, `Unknown: Show | Hide`, placed directly
after `Sort` (both control *which* lights appear, as opposed to how they look).

- `localStorage` key `agentstatus.showunknown`, read via `showUnknown()`, which treats anything
  other than the string `"false"` as Show — so the default, a cleared pref, and **Reset to
  defaults** all keep the #042 behavior.
- `visibleSessions()` returns `latestSessions` unchanged when Show, and
  `latestSessions.filter((s) => displayState(s) !== "unknown")` when Hide. Every draw path —
  `tick()`, `setSort()`, `resetPrefs()`, and the tray's `pushTrayImage()` — draws from it, so the
  bar and the menu-bar image agree.
- `latestSessions` deliberately stays the **complete** poll. Filtering at draw time (not at
  assignment) means toggling the pref repaints instantly from memory and re-enabling restores the
  lights immediately, rather than blanking until the next 1s poll.
- Hiding the last visible session falls through to the existing "no sessions" placeholder dot,
  which is the honest result: there is nothing left to show.

**Options considered.**

| Option | Pros | Cons |
|---|---|---|
| **Frontend `localStorage` pref + draw-time filter** (chosen) | Consistent with every other display pref (#015/#017/#023/#025/#035); ~25 lines; no backend or schema change; instant repaint both ways | One more control in a panel that is getting long |
| Filter inside Rust `list_sessions` | Bar and any future consumer get the filtered list for free | A pure display preference leaking into the backend; the sessions would vanish from the chime and tray paths too, and the app would have to re-poll to bring them back |
| No toggle — always show (status quo) | Nothing to build; one less control | The user explicitly asked for the choice, and a permanent uninformative light is a real complaint |

**Reasoning.** This is the same shape as every other display pref, so it adds a control without
adding a concept. Defaulting to Show keeps #042's honesty as the out-of-box behavior — a session
you can't read is still a session — while Hide serves the "only show me lights that mean
something" reading. Neither setting can make a light *lie*; the choice is only whether the
honest-but-uninformative ring is drawn.

**Validation.** Frontend parses (`node --check`); rebuilt and reinstalled via `./install.sh` and
confirmed the default path live — the bar renders unchanged with the Cursor ring still present, so
existing users see no behavior change. Settings-panel art regenerated
(`node docs/gen-readme-art.mjs`) and rendered to verify the new row sits under `Sort`.
**Not yet exercised by hand:** clicking Show/Hide needs a right-click on the bar, so the user
confirms the toggle itself.

---

## 045 — Cursor pip clicks through the waiting composers (and clears each notification)

**Date:** 2026-07-30
**Status:** Accepted (extends #038)
**Evidence:** Cursor **3.12.10** — `TrayMainService.createContextMenu` in
`/Applications/Cursor.app/Contents/Resources/app/out/main.js`, plus a live Swift AX probe
against the running app (pid 699).

**Context.** The Cursor pip (#038) shows the count off Cursor's menu-bar item but its click
only activated `Cursor.app`. The user still had to find which composer was waiting, and
Cursor's count stayed where it was — the pip reported a problem it could not help resolve,
against UI Principle #3 ("a light leads straight to the session").

**What the tray menu actually is.** `createContextMenu` builds a native Electron `Menu`
(→ `NSMenu`), so every row is a real `AXMenuItem`. Verified live via the Accessibility API,
reachable from the status item **without opening the menu**: status item → `AXChildren[0]`
(its `AXMenu`) → the rows.

```
AXMenuBarItem title=" 2"
  AXMenu
    AXMenuItem "Recent Agents"            (disabled header)
    AXMenuItem "• <composer name>"        (bullet ⇒ unread notification)
    AXMenuItem "• <composer name>"
    AXMenuItem "<composer name>"          (no bullet ⇒ nothing waiting)
    …  "View More (5)" → submenu, "Clear All Notifications", "New Agent", "Open Cursor", …
```

Cursor prefixes `"\u{2022} "` onto entries whose composer has an unread notification, and an
entry's click handler sends `vscode:openComposer` to that composer's window and focuses it,
which marks it read. So a single `AXPress` on the top bulleted row does exactly what the user
asked for — verified live: press → the composer opened in its Cursor window, the status item
went `" 2"` → `" 1"`, and that row's bullet was gone.

**Options considered.**

| Option | Pros | Cons |
|---|---|---|
| **AXPress the top bulleted menu row** (chosen) | Uses Cursor's own handler, so opening *and* clearing are Cursor's semantics, not our guess; needs only the Accessibility grant #039 already secures; the menu never visibly opens; ~60 lines | Depends on Cursor's tray menu structure (bullet prefix, one menu under the status item) — a Cursor redesign silently reverts it to the fallback |
| "Clear All Notifications" row | One press clears everything | Clears without ever *showing* the user the waiting agents — deletes the signal instead of resolving it, exactly the wrong outcome |
| Synthetic clicks (CGEvent) at the item's screen rect | No AX tree dependency | Actually opens the menu on screen, steals focus, needs coordinates and a second click to hit a row; brittle and visibly intrusive |
| Deep-link `cursor://…openComposer` | No AX at all | Composer ids are only in renderer memory / the tray menu — we'd have to read the AX tree anyway, and Cursor shows a consent popup on external deep links (same problem #015 hit) |

**Decision.** New Tauri command `cursor_open_next_attention() -> bool`: walk the status item's
menu, press the **first** row whose `AXTitle` starts with `•`, return whether the press
succeeded. First is the right one — Cursor sorts its own menu notification-first, then
in-progress, then most-recently-updated. Fails silently to `false` (Cursor gone, AX not
granted, no notified entry), and the frontend then falls back to the old behavior of just
activating Cursor, so the click is never a dead click.

Frontend: the pip's click calls it, decrements the badge locally for instant feedback, and
re-reads the true count 1.5s later (Cursor rewrites its menu-bar item asynchronously) rather
than waiting up to 20s for the next scheduled poll. Tooltip now reads "click to open the next
one (clears its notification)". No hook, schema, or installer change.

**Reasoning.** This is the one place the bar can act on Cursor's behalf without inventing
state: Cursor already ranks its waiting composers and already owns "open it ⇒ it's read." We
just press the button the user would have pressed. Repeated clicks walk the queue — count 3 →
2 → 1 → pip gone — which is the interaction the user described. Nothing here can produce a
lying light (UI Principle #4): the count still comes from Cursor's own item, and a failed
press changes nothing.

**Validation.** `cargo build --release` clean; `node --check` on the frontend. A live Swift AX
probe confirmed the press path end-to-end (` 2` → ` 1`, correct composer opened) before any of
it was wired into the app.

**The first build crashed the app on every pip click** — a Core Foundation ownership bug in
this code, not in the AX approach. It pulled the status item's `AXMenu` out of a *temporary*
`AXChildren` `CFArray`: the array holds the only retain on its children, so it released them
as it dropped, and the next `AXUIElementCopyAttributeValue` got a dangling `AXUIElementRef` —
`EXC_BREAKPOINT` in `_AXUIElementValidate` → `CFGetTypeID` → `__CF_IS_OBJC` (crash report
`app-2026-07-30-093608.ips`), killing the whole bar. Fix: bind every `CFArray` to a local for
as long as its elements are used, which is what `cursor_attention_count_inner` already did.
Note the trap is silent in the type system — `ax_attr` hands back raw refs, so nothing in Rust
tracks that the array owns them.

To stop verifying this by hand (Guideline #8), the check is now a re-runnable, `#[ignore]`d
test in `lib.rs`:

    cargo test --release -- --ignored --nocapture cursor_press

It calls the shipped command against the live Cursor, so it exercises the exact traversal that
crashed. Confirmed post-fix: completed without crashing and printed `pressed=true` having
pressed the one bulleted entry (`err=0`, logged via the `cursor-debug` marker); a second
run correctly reported `no notified entry` once the queue was empty. Rebuilt and reinstalled
with the fix; **the click from the bar itself is user-confirmed.**

---

## 046 — Activate Cursor after the pip's press (an AXPress alone doesn't front the app)

**Date:** 2026-07-30
**Status:** Accepted (fixes #045)

**Context.** With #045 shipped, a pip click did half of what it promised: the notification
cleared and Cursor's count ticked down, but the user was left staring at the same window they
were already in. #045 assumed the entry's handler (`vscode:openComposer` → the composer's
window, "focuses it") would also bring the user there. It doesn't: macOS gives focus per
*application*, and an `AXPress` issued from a background process (the bar) never changes which
app is frontmost. Cursor raised the composer's window inside its own app, behind everything
else. Against UI Principle #3 — the click has to land you in the session.

**Decision.** On a successful press, `cursor_open_next_attention` now also activates Cursor
via `open -a Cursor` (extracted as `activate_cursor()`, shared with the empty-`cwd` branch of
`focus_session`, which already did exactly this). Order matters: press first, activate second,
so Cursor has already selected the right window before the app comes forward. `open -a` with
no file argument only activates — it never spawns a window — so it cannot conflict with the
window the press just chose, and activating switches Spaces to that window's Space.

**Options considered.**

| Option | Verdict |
|---|---|
| `open -a Cursor` after the press (chosen) | Reuses the proven activation path already in `focus_session`; no new permission; one process spawn per click |
| Set `AXFrontmost = true` on Cursor's app element | Same effect with no spawn, but adds an AX *write* (we only ever read + press today) for a saving of a few ms on a user-initiated click |
| `cursor` CLI / `open -a Cursor <folder>` | Wrong tool: the pip is aggregate and has no folder; passing one risks opening a *new* window (the failure #016 removed) |

**Reasoning.** The press and the activation are two separate things macOS deliberately keeps
separate — Cursor decides *which* window, the OS decides *which app is in front*, and only the
front app change is ours to make. Doing both is what "click to open the next one" always meant.

**Validation.** `cargo build` clean; rebuilt, signed and reinstalled via `./install.sh`.
**User-confirmed:** a pip click now brings Cursor forward on the waiting composer.

---

## 047 — A Cursor session light opens that conversation (the IDE CLI now starts a new agent)

**Date:** 2026-08-11
**Status:** Accepted (fixes #016 for Cursor; extends #045)
**Evidence:** Cursor **3.15.6** — `windowsManager#open` / `resolveGlassCliFolderTarget` and
`TrayMainService.createContextMenu` in `/Applications/Cursor.app/Contents/Resources/app/out/main.js`,
the `vscode:openComposer` handler in `workbench.desktop.main.js`, and the user's
`storage.json` (`lastActiveWindow.uiState.glassMode = true`).

**Context.** Clicking a Cursor session light landed the user on a **new agent page in that
repo** rather than the session's conversation. Root cause: `focus_session` focuses a window
by running the IDE's own CLI with the workspace root (decision 016). Cursor's main process
now intercepts that. When the CLI is given a single existing-directory argument and the last
active window is a **glass** (Agent) window, `resolveGlassCliFolderTarget` returns that folder
and Cursor sends the glass window `vscode:createNewComposer {folderUri}` — literally "start a
new agent here." Decision 016 predates glass mode; the CLI route is now actively wrong for
Cursor, and no flag combination makes `cursor <folder>` mean "focus the conversation."

**What makes the fix possible.** Two facts, both verified:

1. A Cursor session's `session_id` (from its Claude-compat hook bridge) **is** its
   `composerId` — Cursor's own store holds `composerData:<session_id>` and
   `bubbleId:<session_id>:*` for the ids in `~/.claude/status/sessions/`.
2. Every row of Cursor's tray menu sends `vscode:openComposer {composerId}` →
   `composer.openComposer` (or `glass.openAgentById`) to the composer's window. Decision 045
   already presses such a row via the Accessibility API without opening the menu.

The gap between them: the tray exposes only the composer **name** (the id lives in the click
handler's closure, not the AX tree). So the click needs id → name.

**Options considered.**

| Option | Pros | Cons |
|---|---|---|
| **Name-matched tray press** (chosen) | Opens the exact conversation using Cursor's own handler; reuses the #045/#039 Accessibility path, no new permission; ~90 lines, no new crate | Depends on Cursor's KV-store layout *and* tray structure; reaches only the 10 most recent composers (5 main + 5 "View More"); an unnamed composer has no matchable row |
| Stop calling the CLI, just raise + activate | 5 lines, no new data source | Never opens a new agent, but also never reaches the conversation — the light stops lying without becoming useful (UI Principle #3) |
| Extension relay inside Cursor calling `composer.openComposer <id>` | Uses the command directly, no name lookup, no AX | Requires shipping/installing our extension into Cursor, and whether glass windows host extensions at all is unverified |
| Deep link | No AX | None exists — Cursor's only composer URL handler is `fork-shared-chat`, and there is no CLI flag for a composer id |

**Decision.** For `ide == "cursor"`, `focus_session` no longer touches the `cursor` CLI. It
resolves the name with `cursor_composer_name(session_id)` — `/usr/bin/sqlite3 -readonly` on
`~/Library/Application Support/Cursor/User/globalStorage/state.vscdb`, selecting
`json_extract(value,'$.name')` for `composerData:<id>`, with the id rejected unless it is
`[A-Za-z0-9-]+` since it is interpolated into SQL — then presses the tray row whose title
(minus any notification bullet) equals that name, then activates Cursor. On any miss it falls
back to `raise_window_fast` + `activate_cursor()`, so the click is never dead and **never**
starts a new agent. Only the `.name` field is read: no prompts, no message bodies
(Guideline #5). The lookup runs in the app on click, not in a hook.

The #045 press was generalized into `cursor_press_tray_row(predicate)` + a recursive
`press_in_menu`, so the pip (`starts_with("•")`) and a session light (`name ==`) share one
traversal — and both now also search the "View More" submenu, which the original could not
reach. The dead `ide == "cursor"` arm of the CLI branch is gone; that branch is VS Code only.

**Reasoning.** The signal layer already knows precisely which conversation the user clicked;
what was missing was a way to say so to Cursor. Pressing Cursor's own menu row is the only
external channel that carries a composer id, and it is the same mechanism the pip has used
since #045. Everything else in the click path is unchanged — this is a Cursor-only fix.

**Validation.** `cargo build --release` clean. Live end-to-end check, now a re-runnable
`#[ignore]`d test (Guideline #8):

    AGENTSTATUS_TEST_SESSION=<composer-uuid> \
      cargo test --release -- --ignored --nocapture cursor_press_composer

printed `name=Some("Simplify presentation slides")` and `pressed=true` for a live Cursor
session in ~70ms, with Cursor opening that conversation. Rebuilt, signed and reinstalled via
`./install.sh`.

---

## 048 — Cursor lights reconcile against Cursor's own record (archived, subagent, finished)

**Date:** 2026-08-11
**Status:** Accepted (extends #038, #042)
**Evidence:** Cursor **3.15.6** — the `composerHeaders` table (`composerId`, `isArchived`,
`isSubagent`, `lastUpdatedAt`, indexed) and `composerData:<id>.status` in
`~/Library/Application Support/Cursor/User/globalStorage/state.vscdb`, sampled live against
the sessions on the bar.

**Context.** Two symptoms, one cause. A Cursor light sat **green for 95 minutes** on an agent
that had finished, and lights for **archived** agents stayed on the bar. Cursor's hook bridge
is lossy exactly at the end of a session's life:

- **Archiving fires nothing.** There is no `sessionEnd`, so the status file survived until the
  2h `MAX_IDLE_SECS` backstop. Six of the ten Cursor lights on the bar were archived agents.
- **Some turns never fire `stop`.** Both stuck-green lights were **subagent composers**
  (`isSubagent = 1`); an aborted turn is the same story. The light stays frozen at whatever
  the last `preToolUse` wrote — green, indefinitely. Directly against UI Principle #4.

Cursor knows all of this. Its `composerHeaders` table carries `isArchived`/`isSubagent` per
composer (indexed), `composerData` carries the turn `status` (`completed`/`aborted`/`none`),
and a Cursor session's `session_id` *is* its `composerId` (#047).

**Decision.** `list_sessions` reads every status file first, then asks Cursor about all of its
composers **in one query** (`sqlite3 -readonly`, `composerHeaders` left-joined to
`composerData`), cached with a 5s TTL so the ~1/s poll spawns at most one process per 5s. Then:

| Cursor says | Light |
|---|---|
| `isArchived = 1` | file deleted, light gone |
| no row at all, and silent 60s+ | file deleted (the composer was deleted) |
| `isSubagent = 1` | **no light** — it counts toward its parent's subagent badge, as Claude Code's do |
| `status` terminal, silent 60s+, no live subagent of its own | forced to **idle** |
| `subagentComposerIds` | the parent's **subagent badge**: those composers, minus the ones gone quiet |
| anything else / query failed | untouched — the hooks stay authoritative |

Guards keep this from inventing a wrong light. A terminal `status` alone is **not** trusted:
an agent that was actively working showed `status = "aborted"` on disk from its previous turn.
So the light must *also* have been silent for `CURSOR_STALE_SECS` (60s) **and** have no live
subagent — an agent parked on "Running Task" while a subagent works is silent by definition,
and that subagent's own hook events are what prove the parent is still busy. A failed query
(no `sqlite3`, no db, unreadable mid-write) returns None, which reconciles **nothing**;
absence of Cursor's record is never treated as evidence.

Cursor's `lastUpdatedAt` was tried as that second guard first — require Cursor's write to be
newer than the light's last hook event — and rejected on the evidence: Cursor does not flush it
per message. The stuck agent's header read 13:27 while its hooks had fired through 13:45, so the
guard blocked the very case it was meant to fix. The subagent-liveness check replaces it and
rests on our own signal rather than Cursor's flush timing.

**The subagent badge had the same disease.** `subagentStop` fired for neither subagent of the
stuck agent, so a marker file survived in `sessions/<id>.subagents/` and the light carried a
permanent "1 subagent running" badge pointing at nothing — while Cursor's record showed both
subagent composers terminal. For Cursor sessions the badge is now computed from
`subagentComposerIds`, counting only composers whose *own* status file is still fresh (they run
as sessions in their own right; this decision hides their lights, not their events). Claude Code
keeps the marker files, whose Stop hook is reliable (#010).

**Options considered.**

| Option | Verdict |
|---|---|
| Reconcile against Cursor's store (chosen) | The only source that knows about archiving, subagents, and aborted turns; read-only, display-layer only, no hook or schema change |
| Shorten `MAX_IDLE_SECS` for Cursor | Guesswork dressed as a fix: it would still show a wrong green for minutes, and would kill genuinely long-running agents |
| Ask Cursor for a `stop` on subagent/aborted turns | Not ours to change, and #040's rule stands — build on what the host actually emits |
| Read the tray menu's per-row status via AX | The tray exposes `in_progress`/`needs_attention` as a *sublabel*, unreliable through AX, and covers only the 10 most recent composers |

**Why hide subagent composers.** A Cursor subagent is not a session the user tends separately —
it belongs to the agent that spawned it, which already renders it in the blue subagent badge
(the native `subagentStart`/`subagentStop` hooks in `~/.cursor/hooks.json` write those markers).
One light per *thing the user acts on* is the whole point of the bar; a subagent light is
duplicate chrome that also happens to be the one that never turns off.

**Validation.** `cargo build --release` clean. New re-runnable check (Guideline #8):

    cargo test --release -- --ignored --nocapture cursor_facts

prints what Cursor says about every Cursor session currently on the bar, including each agent's
subagent linkage. Live run classified 5 archived, 3 subagent, 2 real sessions; after rebuild +
`./install.sh`, the five archived status files were pruned within one poll and the stuck-green
subagent lights were gone. The follow-up run confirmed the badge fix on the same data: the
remaining green agent linked to `[b3164824, 87e346b7]`, both silent for 12+ and 29+ minutes and
both terminal in Cursor's record, so the badge drops to zero and the agent itself to idle.

---

## 049 — Match a Cursor tray row by name **prefix**: a running composer's row is `"<name>, Running"`

**Date:** 2026-08-11
**Status:** Accepted (fixes #047)
**Evidence:** live AX dump of Cursor's tray menu (Cursor 3.12 build in `/Applications/Cursor.app`),
new `cursor_dump_tray` test.

**Context.** Clicking a Cursor session light still only brought Cursor to the front — never the
conversation — i.e. the exact symptom #047 was supposed to fix, silently falling through to the
raise + `activate_cursor()` fallback on every click the user actually makes.

**Root cause.** #047 matches the tray row whose title equals the composer's name, allowing only
for the unread bullet (`"• <name>"`). Cursor also appends a **live status suffix**. Dumping the
real menu against the bar's own sessions:

    row "Folder upload functionality, Running"     ← composerData name: "Folder upload functionality"
    row "Simplify presentation slides"

So `trim_bullet(title) == name` matched only *idle* composers. A light is clicked precisely when
its session is running — the one case that never matched.

**Decision.** Match on the bare name **or** the name followed by `", "` (`tray_row_is`), so both
`"<name>"` and `"<name>, Running"` resolve, bullet or not. Requiring the `", "` separator keeps a
name that is a prefix of another composer's name from pressing the wrong row.

**Options considered.**

| Option | Verdict |
|---|---|
| Name prefix + `", "` separator (chosen) | Two lines, no new permission or data source; tolerates whatever status words Cursor uses, since only the separator is assumed |
| Enumerate the status suffixes (`, Running`/`, Done`/…) | Hard-codes strings from Cursor's UI that we cannot verify exhaustively and that change with its releases |
| Split on the last `", "` and compare | Same result, but breaks on composer names that themselves contain `", "` |
| Read the composerId off the AX row | Not exposed — it lives in the menu item's click-handler closure (#047) |

**Note on subagent lights.** The two other Cursor sessions in the repro (`87e346b7`, `b3164824`)
have no tray row at all, because Cursor's tray lists top-level agents only. They are subagents,
and #048 already hides them from the bar — so no light points at a rowless composer. If that
query ever fails, such a light falls back to raise + activate, as before.

**Validation.** Re-runnable (Guideline #8):

    cargo test --release -- --ignored --nocapture cursor_dump_tray        # the real row titles
    AGENTSTATUS_TEST_SESSION=<composer-uuid> \
      cargo test --release -- --ignored --nocapture cursor_press_composer # the full click path

The running composer that failed before (`a09f5f12…`, name `"Folder upload functionality"`) now
prints `pressed=true` and Cursor opens that conversation; `cargo test --release` green, including
a unit test pinning the bullet/suffix combinations.

---

## 050 — Cursor "done" (unread) light, derived from the running→idle transition

**Date:** 2026-08-12
**Status:** Accepted
**Evidence:** decision 038's live finding that Cursor's bridged `Stop` carries no wrap-up
message; `hooks/report.sh`'s `detail` rule; a re-runnable lifecycle check of the shipped
functions (below).

**Context.** Decision 014 split gray into **done** (a turn just finished, output not yet
reviewed — bright white) and **idle** (acknowledged — dim gray). Its discriminator is
`state == "idle" && detail != ""`: `report.sh` writes `detail` from `Stop`'s
`last_assistant_message`, and forces `detail: ""` on `SessionStart`, so a non-empty detail
means "a turn ended and there's something to look at".

Cursor's bridge sends no `last_assistant_message` (verified in #038), so a Cursor session's
`detail` on finish is always `""` — every Cursor turn ended in dim gray, indistinguishable
from a session that had been sitting idle for an hour. The one state a user most wants at a
glance ("this agent came back to you") was exactly the one Cursor couldn't show, and its
absence is what the menu-bar pip (#038) was working around.

**Decision.** Take the finish from the *transition* rather than from the payload. Each poll,
`noteFinishes()` compares every session's raw state against the previous poll; a **Cursor**
session that moves from any non-idle state to `idle` has just finished a turn, and its
`updated_at` is recorded in `finishedAt`. `isFinishedTurn()` accepts either signal:

```js
s.state === "idle" && (!!s.detail || finishedAt.get(s.id) === s.updated_at)
```

Everything downstream is unchanged — the same white `.dot.done`, the same tooltip, the same
click that acknowledges via `reviewedAt` keyed on `updated_at`, the same urgency rank and
tray priority, and the same "done" chime. Because the key is `updated_at`, the next turn's
finish writes a new key and the light re-lights on its own.

This also picks up the finishes #048 supplies: when Cursor's own record says a turn is
terminal and no `stop` ever arrived, `list_sessions` forces the state to `idle` — a
transition, so it lights white too, which is right (the agent did finish).

**Scope: Cursor only.** Claude Code keeps the `detail` test. `detail` lives in the status
file, so a Claude light stays correctly "done" across a bar reload or restart; a transition
lives only in the running webview and would be forgotten. Adding transitions there would
trade a durable signal for a fragile one.

**Options considered.**

| Option | Verdict |
|---|---|
| Frontend transition memory, Cursor only (chosen) | ~20 lines, no hook/schema/backend change, reuses the whole #014 ack cycle; costs only that a finish while the bar is down isn't seen |
| Have `report.sh` write a synthetic `detail` on Cursor's `stop` (e.g. `"turn finished"`) | Puts a fabricated message in the status file that the tooltip would then display as if it were the agent's wrap-up — a lying label, and it changes the hook contract for a display concern |
| Persist the finish to `localStorage` so it survives a reload | Solves a case that barely exists (the bar runs continuously) and risks the opposite failure: a stale unread light for a turn from days ago |
| Read Cursor's own record for the last message | An unnecessary read of conversation content (Guideline #5) for a signal the transition already gives |
| Leave Cursor without a done light, rely on the pip | The status quo, and the reason this was reported: the pip is an aggregate count for agents with no light, not a per-session unread cue |

**Known limit (documented in the README).** The bar only counts finishes it *watched*. A
Cursor turn that ended before the bar launched — or during a reload — shows as plain idle
rather than white. This is deliberate: seeding unread state from a session that was already
idle would light up old turns on every launch, which is the "lying light" failure of UI
Principle #4 in the other direction.

**Validation.** Re-runnable (Guideline #8) — `node` over the functions extracted from the
shipped `app/src/main.js`, so it tests the real source, not a copy:

    node app/tests/unread-light.mjs

Checks: a pre-existing idle Cursor session is not a finish; `running → idle` lights and stays
lit across later polls; a new turn advances the finish key; a vanished session drops its
memory; and Claude Code is untouched (still driven by `detail`, no transition memory kept).
All passed. **Left to verify live:** run a Cursor agent with the bar up and confirm its light
goes white on finish and dims on click.

---

## 051 — Re-check the window against its content every poll, so a lost resize can't leave the pill clipped

**Date:** 2026-08-12
**Status:** Accepted
**Evidence:** live — the running bar measured `37 × 31` points (one light's worth) while its
DOM held five lights, so the pill was drawn with a rounded top and a flat, cut-off bottom.

**Context.** The window hugs its content: `resizeToContent()` measures `#bar` and calls
`setSize`. It is triggered by *edges* — `render()` sets `sizeChanged` when a light is added
or removed, a setting changes the geometry, or the settings panel opens.

**Root cause.** That edge is the only trigger, and the resize can be lost. It awaits two
animation frames before measuring (so it never measures a 0-width pre-paint bar), and the
webview stops delivering animation frames while the window isn't being painted — during
launch, or the relaunch `install.sh` performs. If the frames never arrive, the `await` never
resolves and the size is never applied. The startup measurements that *did* land were taken
before the first poll rendered anything, when the bar held only the "empty" placeholder — one
dot, 31 points tall. From then on `sizeChanged` stayed false (the five dots already existed),
so nothing re-measured and the window stayed sized for one light indefinitely. Confirmed live:
dropping one synthetic session file into the status dir — an add, so an edge — instantly
resized the window to `37 × 146`, and removing it settled at the correct `37 × 123`.

**Decision.** Keep the edge triggers and add a **level check**: `ensureSized()` runs each poll
after `render()`, measures the content synchronously (one layout read, no animation frames),
and calls `resizeToContent()` only when it disagrees with `appliedSize` — the size we last
successfully applied. A lost resize now self-corrects on the next tick instead of never.
`#bar` is an auto-sized `inline-flex` box, so it reports its full content size even while the
window is too small to show it, which is exactly what makes the mismatch detectable.

**Options considered.**

| Option | Verdict |
|---|---|
| Level check each poll (chosen) | Self-healing regardless of *why* a resize was lost; one `getBoundingClientRect` per second |
| Drop the double-rAF wait | It exists to stop the bar measuring 0-width before paint and shrinking to nothing — the bug it prevents is worse |
| Retry only when the resize is skipped (`document.hidden`) | Covers the early-return path but not the stalled-`await` one, which is the path that actually fired here |
| `ResizeObserver` on `#bar` | Event-driven and elegant, but it's another edge — a fresh one to lose — where the poll already gives a natural level check |

**Validation.** Rebuilt and relaunched via `./install.sh`: the window comes up at `37 × 123`
for the five current sessions (was `37 × 31` before the fix), and a screenshot shows the pill
whole — rounded at both ends, all five lights inside.

---

## 052 — Cursor's tray row vetoes the finished-turn reconcile, so a working agent can't be greyed (or flagged unread)

**Date:** 2026-08-12
**Status:** Accepted (amends #048, fixes a symptom introduced by #050)
**Evidence:** live, from one Cursor agent's own timestamps on 2026-08-12:

| Time | What |
|---|---|
| 13:45:30 | Cursor flushed `composerData.status = "aborted"` for composer `50ffc9dc` — its **previous** turn |
| 13:48:09 | Last hook event of the **live** turn: `state=running`, `detail="Write app.js"` |
| 13:49:09 | 60s of hook silence reached → #048's reconcile forced the light to `idle` |
| 13:49:56 | The real `stop` finally arrived — a **107-second** gap between hook events, mid-turn |

**Context.** #048 reconciles a Cursor light against Cursor's own record because the hook
bridge is lossy at end-of-life: archiving fires no `sessionEnd`, and a subagent or aborted
turn fires no `stop`, so a light can sit green forever on an agent that finished. Its guard
against greying a *live* agent is 60 seconds of hook silence plus no live subagent of its own.

**Root cause.** 60 seconds of silence is not evidence that an agent stopped working. A Cursor
agent writing a large file or running a long command emits nothing in between — 107 seconds
here — while `composerData.status` still holds the terminal value flushed at the end of its
*previous* turn (Cursor does not flush per message; #048 established that too). Both reconcile
conditions were therefore true of an agent that was actively editing a file, and its light was
forced to `idle`.

Before today that surfaced as a wrong dim-gray light. #050 then made it loud: the bar derives
Cursor's **done** light from a watched non-idle→`idle` transition, and the forced idle is such
a transition, so a working agent lit up **white — "finished, unread"**. That is the reported
symptom. It also *swallowed* the real unread light: the genuine `stop` 47 seconds later was an
idle→idle no-op, so `finishedAt` kept the forced timestamp, and once the file's `updated_at`
moved on, the finished turn rendered dim gray with no unread cue at all.

**Decision.** Add a fourth condition to the terminal-status reconcile: Cursor's **tray row**
for that composer must not say it is running right now. Cursor titles the row of a live
composer `"<name>, Running"` (#049 discovered this the hard way — exact-name matching missed
every running composer). That is the one *live* status signal Cursor exposes to us; everything
in `state.vscdb` describes the last flushed turn.

    reconcile to idle only if:  60s hook silence          (#048)
                              + status terminal on disk   (#048)
                              + no live subagent          (#048)
                              + tray row is not "<name>, Running"   (new)

`cursor_tray_titles()` reuses #045/#047's AX walk unchanged — `cursor_press_tray_row` only
presses a row its predicate accepts, so a predicate that records and always declines returns
the whole menu without pressing anything or opening it on screen. Cached behind the same 5s
TTL as the fact query and read lazily, so the AX walk happens at most once per 5s and only on
a poll that actually has a light to reconcile. The composer's name comes from one extra column
on #048's existing query (no second `sqlite3` spawn) — still `.name` only, no message content
(Guideline #5).

**Positive evidence only.** An empty tray read — Cursor not running, no Accessibility grant,
a composer older than the tray's ~15 recents — is indistinguishable from "nothing is running",
so it can never *cause* a veto; it just leaves #048's judgement as it is today. The veto only
ever keeps a light green, never turns one on.

**Options considered.**

| Option | Verdict |
|---|---|
| Tray-row "Running" veto (chosen) | The only real-time signal Cursor gives us; purely additive, degrades to today's behavior, keeps #048's stuck-green fix intact |
| Only count a real hook `stop` as a finish (frontend) | Fixes the white light in ~5 lines but leaves the light wrongly dim-gray mid-turn (UI Principle #4), and loses the real done light for the aborted/subagent turns #050 was added for |
| Raise `CURSOR_STALE_SECS` 60s → 5min | Guesswork, which #048 itself rejected: a long tool call beats any threshold, and it delays the genuine stuck-green fix by the same amount |
| Require Cursor's `lastUpdatedAt` to be newer than our last hook event | Already tried and rejected in #048 on live evidence — Cursor does not flush per message, so it blocked the very case it was meant to fix |

**Known limits.** A composer that has fallen off the tray's recents list gets no veto (today's
behavior). Without an Accessibility grant the veto never fires, so a long-silent agent can
still drop to gray — noted in the README next to the existing Accessibility caveats. And if
Cursor were to leave a stale `", Running"` on a finished row, the reconcile would be delayed
until the row refreshes, with the 2h `MAX_IDLE_SECS` backstop unchanged behind it.

**Validation.** `cargo build --release` and `cargo test --release` clean; new
`tray_running_veto` unit test pins the matching rules (suffix required, bullet tolerated, no
prefix bleed between composers, empty name and absent row both decline). The live check is
folded into the existing re-runnable one (Guideline #8):

    cargo test --release -- --ignored --nocapture cursor_facts

which now prints each Cursor session's composer name and its `tray_says_running` verdict
alongside the archived/subagent/terminal facts.

**Verified live** on the very composer from the report (`50ffc9dc`, "Folder upload issue"),
sampled every 3s across a real turn: while it ran, `terminal=true` (the stale `"aborted"` still
on disk) and `tray_says_running=true` held **simultaneously for 71 consecutive samples** — that
pair is precisely the state that greyed the light before this change, and the veto now blocks
it. The suffix also **clears when the turn ends** (105 samples at `false` before and after), so
the veto does not latch and #048's stuck-green fix still fires on a genuinely finished agent.
The one thing that turn did not reproduce is the 60s clock itself — its longest gap between
hook events was 17s — but that clock is unchanged #048 code; what 052 adds was exercised in
both directions.

Across the full 242-sample run, exactly one sample had the session `running` with the veto off,
and it pins down the release timing: the tray dropped `", Running"` at 14:10:21 and the real
`stop` hook landed at 14:10:24. The tray therefore **leads** the hook by a few seconds at
end-of-turn — the veto lifts just before the turn is confirmed over, not after — so it can
never delay a genuine reconcile. (That sample's light was 9s old, far inside the 60s clock, so
nothing was reconcilable in the window regardless.)

---

## 053 — The tooltip identifies a light by the host's own session name, not the folder alone

**Date:** 2026-08-12
**Status:** Accepted

### Context

The hover tooltip's first line was `<project folder> — <state>`, e.g. `AgentStatus — running`.
The folder comes from the hook (`label` = basename of the session `cwd`). Two problems:

1. **Two sessions in the same folder are indistinguishable.** Three of the five live sessions
   on this machine sit in two folders; their tooltips were byte-identical apart from the state.
   The tooltip is the only surface that tells lights apart (UI Principle #5), and it could not.
2. **A session that `cd`s into a subfolder is mislabeled.** The label follows the *current*
   `cwd`, so a session working in `app/src-tauri` reads `src-tauri`, not `AgentStatus`.

### What a "session title" could actually be (verified on the installed version)

| Source | What it holds | Verdict |
|---|---|---|
| `~/.claude/sessions/<pid>.json` → `name` | Claude Code's own name for the session: `agentstatus-5b`, with `nameSource: derived \| user \| auto` | **Chosen.** Present for all 5 live sessions on 2.1.200 / 2.1.223, joinable by `sessionId` |
| Cursor `composerData.name` | The composer's display name (`Fix the parser`) | **Chosen.** Already read by #048's query for the tray-row match — free |
| Transcript `summary` / `title` entries | — | **Rejected: does not exist.** Zero `summary` or `title` records across every `.jsonl` in `~/.claude/projects/`. There is no LLM-written title to read |
| First `UserPromptSubmit` of the session | The subject of the session | **Rejected.** A status-file schema change for something the existing `task` line already conveys, and it goes stale in a long session |

A `derived` name is the folder plus a short suffix, so it is not *descriptive* of the work —
but it is exactly what separates two sessions in one folder, and it becomes the user's own
text when a session is renamed (`nameSource: user`) or auto-named when backgrounded (`auto`).

### Decision

`list_sessions` adds a `name` field per session:

- **Claude Code** — from `~/.claude/sessions/*.json`, mapped `sessionId → name`, behind a 5s
  TTL cache (`SESSION_NAMES_TTL`, the `cursor_facts` pattern) so the ~1s poll does not re-read
  the directory each tick. A name is fixed for the life of a session, so 5s is generous.
- **Cursor** — `CursorFacts.name`, already fetched by #048's single query. No new work.

The tooltip head becomes `folder · name — state` (`AgentStatus · agentstatus-5b — running`).
The name is dropped when it is empty or repeats the folder (case-insensitive), and stands
alone when there is no folder, so the line never carries a redundancy or a bare id it could
have avoided (`headFor` in `app/src/main.js`). An `unknown` Cursor light now names the
composer instead of showing a truncated id. Everything below line 1 is unchanged.

### Why this shape

- **App-side read, no signal-layer change.** The hook is untouched: no new field, no extra
  work in the user's session, and the status-file schema is exactly as before (Guideline #3).
  The names live in directories the hosts already maintain.
- **No transcript is read** (Guideline #5). The two files consulted hold an id, a name, a cwd
  and a pid — no prompt or response text.
- **Fails to the old behaviour.** A missing/unreadable `~/.claude/sessions` yields an empty
  map and the tooltip shows the folder alone, exactly as it did before this change.

### Verification

- `cargo test --release -- --ignored --nocapture dump_session_names` prints the live
  id → name map (read-only). All 5 sessions resolved, including the two sharing a folder.
- `node app/tests/tooltip-head.mjs` covers `headFor`: folder + name, name absent, name
  duplicating the folder, folder absent, and both absent (short-id fallback).

---

## 054 — Terminal CLI and Claude Desktop as first-class hosts

**Date:** 2026-08-13
**Status:** Accepted

**Context.** The bar tracked Claude Code in VS Code and Cursor. The ask was to extend it to
Claude Code in the terminal, Claude Code in Claude Desktop, and Claude Desktop's ordinary
chat threads — without a background poller.

**What the investigation found (all observed live on 2.1.200, Guideline #4):**

| Surface | Hooks fire? | Evidence |
|---|---|---|
| Terminal CLI | **Yes** | Isolated `--settings` probe captured `SessionStart → UserPromptSubmit → PreToolUse → PostToolUse → Stop → SessionEnd`, payload keys byte-identical to the VS Code extension |
| Claude Code in Claude Desktop | **Yes** | Already writing a status file during the investigation itself |
| Claude Desktop chat threads | **No** | Zero hook lifecycle event names anywhere in the 38 MB `app.asar`; its only config is `claude_desktop_config.json` (MCP servers). No local conversation state either — the chat is a webview onto claude.ai |

The decisive fact: **all three Claude Code surfaces read the same `~/.claude/settings.json`**,
so the hook was *already installed and already firing* for the CLI and for Desktop. Nothing
was missing from the signal layer. They were invisible for a display-layer reason —
`report.sh` tagged everything without `.cursor_version` as `ide:"vscode"`, and decision 027
deletes any `vscode` session whose `cwd` matches no live `~/.claude/ide/*.lock`. A terminal
session writes no lock, so its light was pruned on the very next poll. This is the caveat
decision 027 recorded ("a Claude session in a standalone terminal is pruned when any IDE is
open"); it also silently affected Desktop sessions, which survived only when a VS Code window
happened to be open on the same folder.

**Decision.**

1. **Tag the host from `$CLAUDE_CODE_ENTRYPOINT`** (a variable read in `report.sh` — no
   process spawn, so the hook stays fast). Observed values: `cli` (interactive terminal),
   `sdk-cli` (headless `claude -p`), `claude-desktop`. Only `cli` and `claude-desktop` are
   mapped; **any other or absent value falls through to the existing default**, so VS Code and
   Cursor keep their current behaviour by construction — the change cannot regress them even
   though VS Code's own entrypoint string was never observed. Cursor stays sticky ahead of the
   host check, since its native camelCase hooks carry no `.cursor_version`.
2. **`sdk-cli` is deliberately left unmapped**, so headless `claude -p` runs stay pruned
   exactly as they are today. They are short-lived scripted invocations, and lighting them
   would re-create the noise decision 013 added `AGENTSTATUS_IGNORE` to suppress.
3. **Record the owning `claude` pid** (`$PPID` — the hook's parent *is* the claude process,
   verified `comm=claude`) as a new optional `pid` field, and prune `cli`/`claude-desktop`
   lights by `pid_alive()` (the helper decision 027 already had). This is the liveness signal
   that replaces the IDE lock those hosts never write: a terminal closed or force-killed
   without a `SessionEnd` drops its light on the next poll rather than lingering to the 2 h
   backstop. Guarded by `pid > 0`, so status files written before this change keep working and
   fall back to the idle timeout instead of being deleted on sight.
4. **Click-to-focus per host.** A `cli` light focuses the terminal: the recorded pid gives the
   tty (`ps -o tty=`), and Terminal.app publishes a `tty` per tab, so the exact tab is selected
   and raised. Any other emulator is identified by walking the pid's ancestors to the first
   `.app` bundle and gets app-level focus. A `claude-desktop` light activates Claude. Both
   branches are mandatory, not optional polish — without them these hosts fall through to the
   VS Code CLI and land the user in the wrong application entirely.
5. **The VS Code extension is scoped to VS Code sessions, and the focus relay with it.**
   Found from a live report: clicking a Claude Desktop light (still tagged `vscode` at the
   time) focused a VS Code window **and left a stray new Claude tab behind**. The stray tab is
   a second, independent defect. The extension decided what belongs to its window from `cwd`
   alone — its `Session` interface had no `ide` field and never read one — so **any** session
   whose `cwd` fell inside the window's workspace was treated as local to it, regardless of
   host. That produced three symptoms for every non-VS-Code host:
   (i) a status-bar item in a window the session does not live in;
   (ii) clicking that item calls `claude-vscode.editor.open` with an id naming no VS Code
   session, which opens a new Claude tab;
   (iii) the decision-019 bar relay reaching the same command by the same path.
   Cursor was exposed too — a Cursor `session_id` is a composerId, equally unknown to VS Code.
   Fixed on both sides: the extension now parses `ide` and renders **only `ide == "vscode"`**
   (a missing field reads as `vscode`, so pre-054 status files are unaffected), which closes
   all three at once because the relay is gated on the same `seen` set; and `focus_session`
   writes the relay only for `vscode`, which is the honest rule — the file exists solely to
   reach that extension — and keeps the fix working for anyone still on an older extension
   build. Extension bumped to **0.1.3**.
6. **Claude Desktop's chat threads are out of scope.** With no hook mechanism and no local
   state, the only signals available are an Accessibility read of the app window or an MCP
   server that fires on tool calls. The first is per-window rather than per-thread and breaks
   on any redesign; the second never observes idle, done, or blocked, so its light would stick
   green forever. Both produce lying lights (UI Principle #4), which is worse than no light.

**Rejected: a CLI wrapper script.** The fallback plan if hooks turned out not to fire was a
`claude` wrapper the user aliases. Testing showed hooks fire natively, so the wrapper would
have added an alias to install, a per-invocation cost, and a second code path — for a signal
the hook already delivers.

**Scope.** `hooks/report.sh`, `app/src-tauri/src/lib.rs`, and `extension/src/extension.ts`
(host scoping, v0.1.3). **No installer change** — the hosts already read the settings file
the installer already writes. **No frontend change** — the `ide` value flows through untouched.
**No schema break** — `pid` is additive and optional.

**Terminals: what is supported and why.** Terminal.app gets tab-precise focus because its
scripting dictionary exposes `tty` per tab, which joins exactly to the recorded pid. iTerm2
also exposes a per-session `tty` but is not installed here to verify against, so it is left
out rather than shipped blind — that is exactly what decision 040 had to reverse.

**Ghostty cannot be tab-targeted, verified against a live install.** Its scripting dictionary
offers three per-tab identifiers and none of them joins to a process: `working directory`
(identical across tabs in the same project — the common case for concurrent agents, and the
case observed), the tab title (Claude Code rewrites it per turn; the session whose tab was
being sought had an empty `task` while the tab still showed a *previous* prompt), and `id`,
which is an opaque handle (`tab-a8f398e00`, terminal `D6D47ED4-…`) with no relation to the
pty. `lsof` on the Ghostty process shows only the `/dev/ptmx` master, with no ordering that
maps an fd back to a tab. So a Ghostty session activates the app and stops there. A
working-directory match was considered and rejected as the default: it is wrong precisely when
a user has two agents in one project, which is the situation the bar exists for.

**A detached background agent is opened in a new terminal, not merely ignored.** Background
agents (`claude --bg`) are real sessions — Claude Code records and names them (a live example:
"Test session with question prompt") — and they keep their lights, pruned by pid like any
other. But they run detached under `ClaudeCode.app`, with no terminal of their own, so the
ancestor walk reported *that bundle* as their "terminal" and a click would have activated an
unrelated application. The controlling tty is what separates the two cases (`ps -o tty=` ≠
`??`); observed live, the interactive Ghostty session had `ttys000` while three background
agents and a nested child claude all had `??`.

Rather than make those lights inert — the one thing on the bar that leads nowhere, against UI
Principle #3 — a click now opens the agent in a **new terminal window** via `claude attach`,
which is Claude Code's own verb for it and is explicitly safe on a live agent ("Open the
background session in this terminal … The session keeps running either way"). Two details had
to be established live rather than assumed:

- **`attach` takes the short id, not the session uuid.** `claude attach <full-uuid>` answers
  "No job matching …"; the accepted form is the uuid up to the first dash, which is what
  Claude Code prints when it backgrounds a job and what `claude agents --json` reports as `id`.
- **The command must run through a login shell.** A GUI-launched app has a minimal PATH and
  `claude` lives in the user PATH, so the terminal is asked to run `<$SHELL> -lc "claude
  attach <id>"`.

Ghostty is used when it is running (`open -na Ghostty --args -e …`), otherwise Terminal.app
via `do script`, which already runs a login shell. **Ghostty's AppleScript write surface does
not work in 1.2.x** — `new window surface configuration {command:…}`, the same with
`initial input:`, and `new window` followed by a targeted `input text` were all tried first
because each would have reused the running instance; all three report success and then
silently start nothing. Only `-e` actually runs a command, and on macOS that requires
`open -na`, which starts a second instance (`ghostty +new-window` is "not supported on this
platform"). The instance is transient — Ghostty exits when its last window closes.

**CLI lights are reconciled against `claude agents --json`.** Reported live: clicking a
*running* Ghostty session opened a redundant new tab instead of focusing the existing window.
Cause: the hook records `$PPID`, and Claude Code 2.1.231 hosts sessions inside pre-warmed
`claude bg-spare` processes, so the hook's parent is a helper with **no controlling terminal**
even when the session is interactive. The tty test therefore misread it as detached and sent
it down the attach path. The same investigation showed two of five CLI lights were **spares**
— `bg-spare` processes that fire `SessionStart` (creating a light) but never become sessions.
Nothing local separates a spare from a real background agent: the argv is byte-identical
(`claude bg-spare --bg-spare /tmp/cc-daemon…`), neither has a tty, both have a
`~/.claude/sessions/<pid>.json` entry, and spares are started by a long-lived daemon so they
never inherit `AGENTSTATUS_IGNORE` and cannot opt out.

`claude agents --json` is the only authority, and it is cheap (~0.27 s measured). Following
the #048 pattern: a throttled (`CLI_FACTS_TTL` 10 s) call, made **only when the bar actually
holds a CLI light**, yields each live session's `kind` (`interactive` / `background`) and the
pid that really owns it. `list_sessions` drops a CLI light that Claude Code does not list once
it has been silent `CLI_UNLISTED_SECS` (20 s) — long enough that a genuinely new session is
never raced before Claude registers it — and a failed query reconciles nothing, so the bar
fails open. Clicks now route on the reported `kind` and pid rather than on an inferred tty, so
an interactive session hosted in a spare focuses its terminal instead of opening a tab.
Verified live: five CLI lights became the three real sessions, and the two spares vanished.

**A terminal light activates the owning instance, not just the app.** That second instance
turned out to break the *interactive* click too, and was reported as "clicking the
agentstatus-c5 light brought me to an empty Ghostty tab": with two instances running,
`open -a Ghostty` cannot say which one it means, and macOS fronted the attach instance rather
than the one hosting the session. `terminal_app_of` therefore returns the ancestor's **pid**
as well as its name, and the non-Terminal.app path activates that exact process
(`System Events`, `first process whose unix id is …`), falling back to `open -a <name>` when
there is no Accessibility grant. Verified by fronting the wrong instance deliberately and
clicking: focus moved to the instance that owns the session's tty.

**Noted, not a bug.** The working copy of `extension/report.sh` was found still carrying the
Codex support decision 040 removed. It is **gitignored and generated** (`npm run copyhook`,
run by `vscode:prepublish`), so it was a stale local artifact from before #040 and would have
been regenerated on the next package — no stale hook could reach a user. Refreshed anyway so
the working tree matches what a package would produce.

### Verification

- **Hook, unit** — 8 cases against a temp `$AGENTSTATUS_DIR`: `cli` and `claude-desktop` tag
  correctly; `sdk-cli`, an absent value, an empty value, and an unknown future value all fall
  back to `vscode`; Cursor still tags `cursor` and stays sticky under a stray entrypoint; the
  blocked → idle → `SessionEnd` lifecycle and the `AGENTSTATUS_IGNORE` opt-out are intact.
- **Hook, end-to-end** — a real interactive `claude` driven through a pty wrote
  `ide:"cli"`, `pid:34897`; `ps` confirmed that pid is the `claude` process itself and resolves
  to a tty; after killing it the pid is dead, which is what triggers the prune.
- **App** — `cargo check` clean in debug *and* release, no new warnings. New
  `cli_liveness_pruning` test (`cargo test -- --ignored --nocapture cli_liveness_pruning`)
  builds a temp status dir and asserts a live-pid CLI light survives, a dead-pid one is pruned,
  and a pre-054 file with no `pid` is *not* pruned.
- **Terminal focus** — the AppleScript tab lookup was checked against a live Terminal.app
  session: correct positive on the real tty, correct negative on a fabricated one, and the full
  select-and-raise brought the right tab to the front (`frontmost` confirmed `Terminal`).
- **Live, after `./install.sh`** — the Claude Desktop session running this work retagged
  itself to `ide:"claude-desktop"` with `pid:95357`, and `ps` confirms 95357 is Claude
  Desktop's own `claude` binary. The host that was previously invisible-by-luck is now a
  first-class light.
- **Extension** — `tsc` clean; the host filter and the `ide` default were confirmed present in
  the compiled output *and* inside the packaged `agentstatus-0.1.3.vsix`, then installed via
  the `code` CLI.
- **Not yet verified live:** a terminal light on the running bar and the packaged app
  performing the terminal focus (needs the Automation grant for AgentStatus.app →
  Terminal.app, separate from the existing Accessibility grant; decision 039's stable signing
  should make it persist across rebuilds); and the extension change, which takes effect only
  on the next VS Code window reload.

---

## 055 — A Ghostty light focuses the exact tab (and split) via Ghostty's own scripting dictionary

**Date:** 2026-08-13
**Status:** Accepted

### Context

Decision 054 gave terminal sessions tab-precise focus only in Terminal.app, which publishes
a `tty` per tab. Ghostty was left at app-level focus, and that is what the user hit: clicking
a Ghostty light brought Ghostty forward but left whichever tab was last active in front, so
a bar with several Ghostty sessions could not lead to any of them. That breaks UI Principle
#3 — the thing you look at to see a problem must be the thing you click to go fix it.

054 recorded that Ghostty "exposes no tty (only a working directory, which is ambiguous
across tabs)". Two facts have changed since:

- **Ghostty 1.3 ships an AppleScript dictionary** (`Ghostty.app/Contents/Resources/Ghostty.sdef`,
  installed version 1.3.1). It defines `window` → `tab` → `terminal` (a surface, i.e. a tab
  *or* a split), each with a title, and a `focus` command documented as "Focus a terminal,
  bringing its window to the front". Neither a tty nor a pid is exposed.
- **Claude Code 2.1.231 writes a session title**, as an `ai-title` record in the session
  transcript, and puts that title in the terminal's title bar. Decision 053 checked for
  exactly this on 2.1.223 and found none — the table row "Transcript `summary` / `title`
  entries → **Rejected: does not exist**" is now out of date for `ai-title`.

Observed live, one Ghostty window, one tab, two splits, each running a session:

```
SURF 6F2668F6…  name=[◐ Claude Code]                                wd=[…/agentstatus]
SURF EB362C00…  name=[◑ Fix Ghostty tab focus when clicking light]  wd=[…/agentstatus]
```

The working directory is identical, as 054 said. The title is not: the second surface's
title is the bar's own session title with the activity spinner glyph prepended. The first
session had no `ai-title` yet, so its terminal reads the generic "Claude Code".

### Options

| Option | Verdict |
|---|---|
| Match the surface by `working directory` | **Rejected.** It is the *shell's* cwd, so two agents in one repo — the exact case reported — are indistinguishable, and a plain shell sitting in that folder would match and be focused instead |
| Have the hook record a Ghostty surface id | **Rejected.** Ghostty exports no surface identifier into the session environment (`GHOSTTY_RESOURCES_DIR`, `GHOSTTY_BIN_DIR`, `GHOSTTY_SHELL_FEATURES`, `TERM_PROGRAM*` and nothing else), so the hook has nothing to record, and querying Ghostty from inside the hook would put an `osascript` spawn on the user's turn (Guideline #3) |
| Ask Claude Code for the mapping | **Rejected.** `claude agents --json` and `~/.claude/sessions/<pid>.json` carry the pid, cwd, kind and name — no tty, no title |
| Order surfaces against shell pids by creation time | **Rejected.** Unverifiable ordering, and it would fail silently into a *wrong* tab |
| **Match the surface title against Claude Code's session title** | **Chosen.** The only identifier both sides publish, and it is the one thing that separates two agents in one folder |

### Decision

`focus_terminal_session` gains a Ghostty branch, tried after the Terminal.app tty lookup and
before the existing app-activation fallback:

1. `claude_ai_title(session_id)` finds the session transcript by globbing
   `~/.claude/projects/*/<session_id>.jsonl` — cheaper and steadier than re-deriving Claude
   Code's directory-slug rule from `cwd` — and returns the **last** `ai-title` record, the
   title being rewritten as the subject of the session changes. Files past 16 MB are skipped
   rather than read on the click path; the largest transcript on this machine is 3.8 MB.
2. `focus_ghostty_surface` runs one `osascript` that walks `terminals`, collects those whose
   `name` **contains** the title (`contains`, not equality, because of the spinner glyph),
   and calls `focus` only when there is **exactly one** hit.

Anything else — no `ai-title` yet, no hit, more than one hit, Ghostty older than 1.3, or the
Automation grant refused — returns false and the click falls back to fronting the app, which
is precisely the behaviour every Ghostty click had before. A wrong tab is worse than no tab
(UI Principle #4).

An untitled session is deliberately *not* matched on the literal "Claude Code" its terminal
shows. That string names nothing, and matching it would risk focusing some other untitled
session's tab.

### Why this shape

- **No hook change, no schema change.** All of it runs app-side on click, off the hot path
  of the user's session (Guideline #3). The status file is byte-identical to 054's.
- **Reading the transcript is a flagged exception to Guideline #5.** The title is the only
  per-surface identifier Ghostty publishes, so the feature genuinely requires it. Only the
  title string is pulled out, nothing is stored, and it is already on screen in the tab it
  names. Lines are substring-filtered on `"ai-title"` before any JSON parsing, so no message
  body is ever deserialized.
- **Version-dependent by nature, so it degrades rather than assumes.** `ai-title` did not
  exist on 2.1.223 and Ghostty's dictionary did not exist before 1.3; both absences fall
  through to the old path instead of failing.
- **Splits come for free.** Ghostty's `terminal` is a surface, so the same match reaches a
  session in a split pane, which no tty-style mapping would have.

### Known limits

- **Two Ghostty instances.** `tell application "Ghostty"` reaches one of them; a session in
  the other simply produces no hit and the click degrades to app-level focus. 054's
  background-agent attach (`open -na Ghostty`) is what creates a second instance.
- **iTerm2 and other emulators are untouched.** Not installed here, so no code that has not
  been tested against them (Guideline #4).
- **Two sessions with titles that contain one another** produce two hits and fall back.

### Verification

- **The shipped AppleScript, run standalone** against the two live splits: a fabricated
  title returned `no`; `"Claude Code"` returned `ok` and reading back
  `focused terminal of tab 1 of window 1` confirmed the *other* split; the real session title
  returned `ok` and moved focus back. Both directions, confirmed by read-back, not by
  assumption.
- **`focus_ghostty_live`** (new, `--ignored`): the untitled session printed
  `title = None / focused = false`; the titled one printed
  `title = Some("Fix Ghostty tab focus when clicking light") / focused = true`.
- **`focus_terminal_live`** (extended with the session id and the resolved title) on the live
  session: `tty = Some("/dev/ttys001")`, `terminal app = Some(("Ghostty", 32265))`,
  `session title = Some("Fix Ghostty tab focus when clicking light")`, and the click path
  landed on the right split.
- **`cargo build --release`** clean, no new warnings; `cargo test --release` 2 passed,
  9 ignored.
- **Survey of `ai-title` across `~/.claude/projects`**: present in 6 of the 12 most recent
  transcripts, 1–56 records each, absent in every transcript of a session that had not yet
  produced a turn — matching the fallback the code takes.
- **The packaged app, clicked for real.** Two sessions running as two splits of one Ghostty
  tab, with the installed `/Applications/AgentStatus.app`. A background recorder sampled
  `focused terminal` and the frontmost app twice a second so the result could not be
  corrupted by whatever was clicked afterwards. The decoy put split `6F2668F6`
  ("Plan AI model support…") in front and Ghostty in the background; clicking the
  `agentstatus-e0` light fronted Ghostty **and** moved focus to `EB362C00`
  ("Fix Ghostty tab focus…"), and clicking `agentstatus-3c` 38 s later — with Ghostty
  **already** frontmost and `EB362C00` focused — moved focus to `6F2668F6`.

  That second transition is the decisive one: fronting an application cannot change which
  split is focused inside it, so it can only have come from the AppleScript `focus`. It also
  settles the Automation grant without needing to read the TCC database (which is itself
  protected): a refused grant makes `focus_ghostty_surface` return false and the click falls
  through to the activate-by-pid path, which fronts Ghostty and leaves the focused split
  alone — the split change could not have happened. Both directions, on the real app, from a
  real click.

## 057 — Token/cost tracking: what the data allows, and what it would cost

**Date:** 2026-08-13
**Status:** Proposed — options presented, awaiting the user's go/no-go

**Context:** A survey of comparable tools (2026-08-13) found that cost/usage analytics is the
single most common feature AgentStatus does not have: **AgentsView** keeps per-session and
per-model cost breakdowns, daily spend charts, and activity heatmaps in a local SQLite
database; **usage** shows token burn rate, session/weekly quota, and per-project cost
estimates; **claude-statistics** shows subscription usage and cost in real time. The user
asked for an effort estimate before deciding whether to build it.

**What the data actually allows (verified live on this machine, not inferred):**

| Source | Carries token/cost data? | Evidence |
|---|---|---|
| Claude Code transcripts (`~/.claude/projects/*/<id>.jsonl`) | **Yes** — `type:"assistant"` records carry `message.model` and `message.usage.{input_tokens, output_tokens, cache_creation_input_tokens, cache_read_input_tokens}` | Parsed a real 3.7 MB / 464-line transcript |
| **Hook payloads** | **No** — nothing token-shaped on any event | All 246 captured events in `logs/events.log`; the full key set is `background_tasks, cwd, duration_ms, effort, hook_event_name, last_assistant_message, permission_mode, prompt, prompt_id, reason, session_crons, session_id, source, stop_hook_active, tool_input, tool_name, tool_response, tool_use_id, transcript_path` |
| `~/.claude/sessions/<pid>.json` | **No** usage field (it carries `status`/`name`/`pid` bookkeeping only) | Read live |
| `claude` CLI | **No** cost/usage subcommand | `claude --help`: `agents, auth, auto-mode, doctor, gateway, import, install, mcp, plugin, project, setup-token, ultrareview, update` |
| **Cursor** `state.vscdb` | **No cost data.** `usageData` is empty `{}` on every sampled `composerData` row. It does carry `contextTokensUsed` / `contextTokenLimit` / `contextUsagePercent` (e.g. `127637 / 200000 / 63.8`) | Read-only `sqlite3` query |
| Model pricing | **Nowhere local** — a price table would have to be hardcoded in the app | — |

Two consequences fall straight out of that table. **The usage data is per-API-call, not a
session total**, so "tokens spent this session" means summing every assistant record in the
file, not reading its tail. And **Cursor can never be covered** — it stores context-window
fullness, which is a different metric, and no cumulative spend at all. Any version of this
feature is Claude-Code-only and therefore partial, on a bar whose whole point is showing all
hosts side by side.

**Options considered:**

| # | Option | Where the code goes | Effort | Biggest blocker |
|---|---|---|---|---|
| 1 | **Do nothing / defer** | — | none | Only the cost of staying honest that it isn't shipped |
| 2 | **Last-turn cost in the tooltip**, Claude Code only | New TTL-cached reader in `lib.rs` following the `cursor_facts_query`/`cli_facts` pattern (`lib.rs:288-362`, `935-978`); a `transcript_path` field added to the status file; one line appended in `titleFor()` (`main.js:789-804`) | ~half a day to a day | Claude-only; a hardcoded price table that rots on every model release |
| 3 | **Cumulative session cost** + an aggregate readout in the settings panel | Option 2 plus a per-session byte-offset checkpoint (so a poll parses only newly appended bytes) and new settings-panel UI | ~2–3 sessions | Same two blockers, plus new UI real estate the "glanceable dots" design has no room for, plus a first-scan cost on long resumed transcripts |

**Decision:** Present these three and build nothing until the user picks. Recording the
constraints that bind whichever option is chosen:

1. **Do not put the token read in the hook.** Decision 040 deliberately removed *all*
   transcript reading from `report.sh` (and its `python3` spawn with it). A last-turn read is
   cheap (~3 ms for a `tail -c 4000` on a 3.7 MB file) but a session total needs a full-file
   scan, and putting either back on the user's turn reverses #040 and runs at Guideline #3.
   The app-side TTL-cached reader has no such cost because it runs in AgentStatus's own
   process, and the codebase already has that exact pattern twice.
2. **This is a status-file schema change and needs explicit approval** (Agent Decision
   Framework #2) — options 2 and 3 both add at least a `transcript_path` field.
3. **Nothing numeric goes on the bar itself** (UI Principle #1). The tooltip and the settings
   panel are the only two surfaces that don't compete with the lights.
4. **The price table is a standing maintenance liability**, not a one-time cost — it goes
   stale every time a model ships, and there is no local source of truth to query instead.

**Reasoning:** The question the user asked was "how much work," and the answer is that the
work is small but the *maintenance* is not, and the *coverage is partial by construction*.
That reframing is the actual decision input: option 2 is genuinely cheap, but it buys a
Claude-Code-only number that needs hand-maintained pricing forever. Worth stating plainly
rather than burying under an effort estimate.

## 058 — A fallback view for high session counts (design options)

**Date:** 2026-08-13
**Status:** Proposed — options drafted, awaiting the user's choice; nothing implemented

**Context:** The same competitor survey found that AgentStatus's **one-light-per-session bar
is its one genuinely unmatched design choice** — `claude-status-bar` and `gmr/claude-status`
both collapse every session into a single aggregate menu-bar icon and make the user open a
dropdown to see which session needs them. That is the inverse of this project's whole premise
(UI Principle #1: read state at a glance, no click-through). But it does not scale, and the
scaling is linear and measurable.

**The actual numbers.** The bar is `--dot-size: 13px` with a `10px` gap and `9px` padding per
side (`styles.css:18,45,101`), so its long axis is **`8 + 23N` px** for N lights — confirmed
against a real measurement in decision 051 (5 sessions → 123 px):

| Sessions | Bar length |
|---|---|
| 5 | 123 px |
| 10 | 238 px |
| 20 | 468 px |
| 30 | 698 px |

At 30 sessions a vertical bar is 698 px — roughly two-thirds of a 1080p display's height. The
failure is not sudden; the bar just quietly stops being glanceable somewhere past ~15.

Worth noting what the live machine looks like today: **4 sessions, 3 of which share one
project folder** (`AgentStatus`). That distribution is the clue — the count grows much faster
than the number of *things the user is actually working on*.

**Options considered:**

| # | Option | How it works | Pros | Cons |
|---|---|---|---|---|
| **A** | **Group by project folder** | One light per folder; badge = session count; color = most urgent state in the group (reuse `summaryState`, `main.js:253`) | Collapses the common case hard (today: 4 lights → 2). Reuses the `byWindow` sort (#025), the tray's `TRAY_PRIORITY` condense (#026), and the badge element (#009) — very little new code. Groups match how the user thinks about their work | Collides with the subagent badge, which already owns that corner (#009). Click is ambiguous — which session does it focus? |
| **B** | **Overflow chip** | Show the top K lights by urgency, then one summary chip for the rest; chip color = most urgent hidden state, badge = count; click expands a list | **Bounds the bar by construction** — never exceeds K+1 lights at any session count. Urgency sort (#025) guarantees the hidden ones are never the ones needing attention | Needs an expansion surface (though menu-bar mode already has a popover). Hidden sessions are genuinely invisible |
| **C** | **Wrap into a grid** | Past a threshold, flow lights into multiple rows/columns | Zero information loss — every session keeps its own light, fully preserving the differentiator. Nearly free: a CSS `flex-wrap` plus a max-per-line setting; `contentSize()`/`ensureSized()` (#051) already measure whatever the DOM produces | A 5×6 dot grid is materially less scannable than a row. Still unbounded, just in two dimensions |
| **D** | **Auto-shrink density** | Past a threshold, shrink dot size and gap so total length stays bounded | Trivial — size is already a CSS variable driven by a setting (#017) | Fights UI Principle #1 directly: 6 px dots are not glanceable and the state colors get hard to tell apart. **Recommend rejecting** |
| **E** | **Attention-only mode** | Show only lights in an attention state (error/blocked/done); collapse all running/idle into one summary dot | Targets the real need most directly (UI Principle #2), and is bounded by how many things actually need the user — naturally a small number | A running session vanishing from the bar is jarring ("where did it go?"), and a near-empty bar can read as "the app broke" |

**Recommendation (not yet approved):** **B as the mechanism, A as an orthogonal toggle.** B is
the only option that bounds the bar no matter how many sessions exist, and it does so while
keeping urgency ordering so the lights that survive are exactly the ones that matter. A is
worth having independently because it attacks the *cause* of high counts in the common case —
several sessions in one repo — rather than the symptom. D should be rejected outright, and E
is compelling but its "disappearing light" failure mode needs a real answer before it ships.

Whatever is chosen should be **a threshold, not a mode switch**: below the threshold nothing
changes at all, so the user's normal 4–5 sessions look exactly as they do today, and per-session
lights stay the default (explicitly what the user asked for). The setting belongs in the
existing panel as a new `.seg` row alongside `Sort` / `Unknown`, following the same
`localStorage` + apply-function pattern (`index.html:40-53`, `main.js:625-650`) that every
display pref already uses — frontend-only, no hook and no schema change.

**Reasoning:** The survey result cuts both ways. Every competitor that aggregates has given up
the property that makes this tool worth using, so aggregating *by default* would be trading
away the differentiator to solve a problem the user does not have at 4 sessions. But the linear
growth is real and the bar does become unusable. A threshold keeps the differentiator at normal
counts and degrades gracefully past them, which is the only shape that satisfies both.

## 059 — The hook-only signal layer: real risk, and the reconcile the core path never got

**Date:** 2026-08-13
**Status:** Proposed — analysis complete, fix not yet implemented

**Context:** Competitors take visibly different bets on signal reliability.
**gmr/claude-status** runs *three* redundant channels — Darwin push notifications, filesystem
watching, and a 5-second polling fallback — on top of its hook plugin. **so-agentbar** and
**marmonitor** skip hooks entirely and passively parse each agent's own session/log files from
outside, accepting staleness in exchange for zero in-session instrumentation. AgentStatus is
hook-only (#001, #002). The user asked how risky that actually is.

**Observed failure modes — every one of these has actually happened in this project:**

| # | Failure | Evidence |
|---|---|---|
| 1 | **The hook never executes at all.** Not a bad payload — no invocation | Cursor 3.12.10 fired `sessionStart`/`beforeSubmitPrompt`/`stop` but every hook failed with `MainThreadShellExec not initialized`, so no light ever appeared. Proven not our bug: `report.sh` ran correctly for Claude Code in the same window seconds later |
| 2 | **Documented events that never fire.** `Notification`, `Elicitation`, `PermissionDenied`, `SubagentStart/Stop`, `Pre/PostCompact` fired zero times in a full captured run; `StopFailure` fired **0 times even under an induced tool failure** | #006, #013 — the red/error signal has been marked "interim" since #006 and has never been validated against a real turn-level failure |
| 3 | **End-of-life events silently drop.** Archiving a Cursor agent fires no `sessionEnd`; a subagent or aborted turn fires no `stop` | #048 — lights sat green on finished agents for up to **95 minutes** |
| 4 | **One event, then permanent silence.** A folder-less Cursor window fires `sessionStart` and nothing ever again | #042 — which is why the hollow "unknown" ring exists |
| 5 | **Structurally unreachable sessions.** Cloud/background Cursor agents run remotely and fire no local hooks at all | Confirmed while building #038 |
| 6 | **Two entire hosts removed over exactly this.** Codex and Antigravity were ripped out rather than papered over with inference | #040 |

**Theoretical but real, and currently undetectable:**

- **The hook registration silently disappears.** A Claude Code update, a hand-edit, or another
  tool's installer overwriting the settings file would stop every future invocation.
  `ensure_installed()` runs **once, at app launch** (release builds only) and there is no
  periodic re-check — the app never reads the settings file at runtime. Worse, this failure is
  **indistinguishable from "nothing is running"**: the bar renders the same "No active Claude
  Code or Cursor sessions" placeholder (`main.js:823`) either way. That is a silent failure
  with no light to be wrong about — arguably worse than the lying light UI Principle #4 guards
  against.
- **`jq` missing on the packaged-app path.** `report.sh` uses `jq` nine times with no
  availability guard; `install.sh:11` checks for it, but **`install.rs` — the DMG path, which
  is the *primary* distribution channel per #024 — never does**. Without `jq` every hook
  invocation silently no-ops forever. Scope check: `jq` ships at `/usr/bin/jq` only since
  **macOS 15 (Sequoia)** (verified present here on 26.6.1, root-owned). The README requires only
  "macOS on Apple Silicon (M1 or later)", so a DMG user on macOS 11–14 without Homebrew gets a
  completely inert app and no error.

**A genuine strength worth recording:** because the transport is a file (#002/#007), hooks keep
writing whether or not the display app is running — sessions that ran while the bar was closed
are fully present the moment it relaunches. A socket/push design would lose them. This is why
the answer below is *not* "adopt the competitor's architecture."

**What is already mitigated — the project has largely built a second channel without calling
it one:** IDE-lock pruning (#027), the 2h heartbeat backstop (#004), pid-liveness pruning for
`cli`/`claude-desktop` (#054), the `claude agents --json` spare-ghost check, the Cursor sqlite
reconcile (#048), the Cursor tray-row veto (#052), and the honest hollow ring (#042).

**The gap, stated precisely:** *Cursor* has a real independent **state** reconcile (#048/#052).
*CLI/Desktop* have pid liveness — an **existence** check, not a state check. **The core Claude
Code path has no state reconcile at all.** Its only protections are the hook itself, IDE-lock
existence pruning, and a 2-hour idle timer. The #048-class bug — a session that finishes or
dies through a path that doesn't cleanly fire `Stop`/`SessionEnd`, leaving a stuck green light
— was only ever found on Cursor *because someone went looking there*. It has never been checked
for on the core path.

**And the material for the fix is already being read and thrown away:**

- `claude agents --json` returns a **`status`** field (`"busy"`/`"idle"` observed live), but
  `CliFact` (`lib.rs:916-919`) stores only `kind` and `pid` — **`status` is parsed and
  discarded**.
- `~/.claude/sessions/<pid>.json` carries **`status` *and* `statusUpdatedAt`** (verified live:
  `{'pid': 34282, 'status': 'busy', 'statusUpdatedAt': 1786644705862, ...}`). `session_names()`
  (`lib.rs:216-227`) already opens this exact file every poll — for the `name` field only.

`statusUpdatedAt` matters specifically: it is the freshness stamp a guarded reconcile needs, the
direct analogue of #048's "Cursor's own write must be newer than the last hook event" guard,
which is what stops a reconcile from greying a working agent (#052).

**Decision:** Do **not** adopt a multi-channel architecture. Add **one** guarded state
reconcile for Claude Code sessions, mirroring #048's proven shape exactly: read `status` +
`statusUpdatedAt` from sources already being read, behind the existing TTL pattern
(`CLI_FACTS_TTL`, `lib.rs:923`); override only a light already silent past a threshold; never
override a fresh hook write; no-op entirely on a failed read. Separately, add the missing `jq`
guard to `install.rs` to match `install.sh:11`.

**Explicitly unverified, and must be before any code (Guideline #4):** the live machine had no
VS Code Claude Code session running, so `status` was confirmed present for **`entrypoint:"cli"`**
sessions and confirmed **absent (`None`) for `claude-desktop`** ones. Whether a VS Code session
reports `status` is **unknown**. If it does not, this reconcile does not cover the core path and
the decision needs revisiting — which is precisely the mistake #040 was written about. Observe a
real VS Code session first, then write code against what was observed.

**Reasoning:** Hook-only is not an acceptable long-term risk for the core path — the evidence
above is six *observed* failures, not hypotheticals. But the competitor's three-always-on-channels
answer is the wrong correction: #040 is this project's own precedent that unverified redundant
complexity is worse than a missing feature, because it produces lying lights. The right fix is
the narrow pattern already battle-tested here on Cursor, applied to the one host that never got
it, using data the app is already reading. It costs no new subprocess, reuses an existing
throttle, and targets a bug class this project has already proven exists.

## 056 — Re-adding Codex and Gemini: what verification actually costs, and which one is even possible

**Date:** 2026-08-13
**Status:** Proposed — scoped, blocked on one question to the user; no code written

**Context:** A survey of comparable tools (2026-08-13) found multi-agent coverage is the most
common thing AgentStatus lacks — `claude-statistics`, `usage`, `anotifier` and `marmonitor` each
track 3–4 agents, and `AgentsView` lists 60+. The user confirmed real pull toward covering Codex
and Gemini alongside Cursor.

**The thing to say first: AgentStatus already had both, and removed them.** Decision 040
(2026-07-29) stripped Codex (#029/#031/#032) and Antigravity/Gemini (#033) because neither was
ever verified against a live install. #033 shipped explicitly "Accepted, unverified"; the Codex
path substituted a `~/.codex/state_5.sqlite` read, a `pgrep -x codex` probe, and a bespoke
10-minute idle timeout for lifecycle events nobody had confirmed fire. So this is a **re-add,
not a new feature**, and the reason it was removed is exactly the reason it can't simply be
restored from git history.

**A re-add must first defuse the removal.** #040 left cleanup code that runs unconditionally and
would silently delete freshly-written hook entries on the very next launch:
`cleanup_legacy_hosts()` (`install.rs:80-121`, called from `try_install()` at `install.rs:71`)
strips any `report.sh` entry from `~/.codex/hooks.json` and deletes the `agentstatus` key from
`~/.gemini/config/hooks.json`; `cleanupLegacyHosts()` (`setup.mjs:73-82`) does the same from both
`install` and `uninstall`. Verified working on this machine — both files are currently cleaned
(`{"hooks": {}}` and `{}`) with `.agentstatus-bak` files preserving the old registrations.

**What is actually installed here (verified):**

| Host | Installed? | Consequence |
|---|---|---|
| Codex CLI (`codex` binary) | **No** | — |
| Codex VS Code extension (`openai.chatgpt`) | **Yes** — two versions | **Codex is verifiable today**, via the same integration #032 examined |
| Gemini CLI (`gemini`) | **No** — and no `~/.gemini/settings.json` | Verification blocked until installed |
| Antigravity CLI (`agy`) | **No** | Verification blocked until installed |
| Antigravity IDE | **Yes** — both `.app` bundles present | Verifiable today |

**"Gemini" has become ambiguous since #033 shipped, and this is the blocking question.** Three
different products now exist, with different config paths, schemas, and event sets:

1. **Antigravity IDE** — what #033 actually targeted: `~/.gemini/config/hooks.json`, hooks under
   a bespoke top-level `agentstatus` key, payloads using `workspacePaths[]` and `toolCall.name`.
   Installed here.
2. **Gemini CLI** — Google's separate terminal tool, with its own official hooks in
   `~/.gemini/settings.json` under a `hooks` map (a shape much closer to Claude's). Never built;
   not installed here.
3. **Antigravity CLI (`agy`)** — newer still, reportedly moved off `settings.json`/`hooks.json`
   onto `.agents/hooks.json`. Never built; not installed here.

Building against #1 would not cover #2, and vice versa. **The user must say which they mean**
before any work starts — it changes the config path, the schema, the effort, and whether
verification can begin today at all.

**Whether orange and red are even reachable — the question NEXT_STEPS.md item 10 says to settle
up front,** because a host with no permission-request and no failure event can never show an
attention state, quietly breaking UI Principle #2 for that host:

| Host | 🟠 blocked | 🔴 error |
|---|---|---|
| **Codex** | **Likely reachable** — `PermissionRequest` is documented for shell/network escalation | **Not reachable** — no failure/error event documented anywhere; matches #032/#040's original finding |
| **Gemini CLI** | **Not cleanly reachable** — `Notification` fires on permission alerts but is documented as observability-only, unable to represent a pending approval. Blocked would have to be *inferred* — the exact class of guess #040 deleted | **Not reachable** — no failure event; exit-code-2 abort/retry on `BeforeModel`/`AfterModel` is control flow, not an error signal |
| **Antigravity IDE** | **Not reachable** — #033 established no permission-request event exists; nothing suggests it changed | **Not reachable** — same, known at ship time |

So **only Codex can plausibly reach even three of four states**, and none of the three can show
red. Re-adding Gemini or Antigravity today means shipping a permanently green/gray-only light.
That is a limitation to accept knowingly up front, not to discover mid-build.

**One more reason not to trust the docs here:** the two sources found for Codex hooks *directly
contradict each other* on the single fact that mattered most to #032 — whether a session-close
signal exists. The apparent official location says hooks are production-ready and enabled by
default with a real `SessionEnd` (firing on close or 30-minute idle); a second, non-OpenAI source
says hooks are experimental, disabled by default, with no `SessionEnd` at all. That conflict is
itself the argument for Guideline #4: build on neither claim, observe the installed version.

**Decision:** Do not restore any removed code. Re-add per host through the same
observe-then-build sequence that Claude Code and Cursor went through, reusing the verification
tooling already in the repo — `hooks/log-events.sh` + `logger-setup.mjs` and their Cursor
equivalents (`cursor-log-events.sh`, `cursor-logger-setup.mjs`) are the direct template: a
throwaway, fail-silent, stdout-silent logger registered across a deliberately *broad* event list,
appending raw payloads to a log, then uninstalled.

Sequence per host: **(a)** write the host's logger + setup script and capture a real session
through representative actions — a command needing approval, a failing command, a session closed
and idled out; **(b)** write `report.sh` payload branches from what was observed; **(c)** installer
writers, gating the #040 cleanup so it stops deleting the host being re-added; **(d)** app-side
liveness pruning and click-to-focus; **(e)** live end-to-end verification.

**Effort:** **Codex ≈ 2–2.5 days** and can start now. **Gemini ≈ 2.5–3.5 days**, plus install
time, and cannot start until the user resolves which product is meant.

**Reasoning:** The pull toward multi-agent coverage is real and worth acting on, but the failure
mode here is already documented in this repo: both hosts were previously built from documentation
rather than observation, produced lights that could not be trusted, and were deleted three weeks
later. The cost of doing it properly is a few days per host; the cost of doing it the fast way has
already been paid once. Codex is the one to start with — it is verifiable today and is the only
host that can show an attention state at all.

---

## 064 — A light that names no session does nothing, and a background agent opens in a tab of the Ghostty you already have

**Date:** 2026-08-13
**Status:** Accepted

### Context

Reported from live use: *"a new light appeared on my bar. it opens a new ghostty window with
copies of my claude sessions."*

Traced to a **pre-warmed spare**. `1550acaa` had a status file whose recorded pid `4592` was a
`claude bg-spare` process, and it was absent from `claude agents --json`. Decision 054 already
knew spares fire `SessionStart` and get a light, and prunes such a light after
`CLI_UNLISTED_SECS` (20 s). This one was clicked inside that window.

The click then did the worst available thing. In `focus_session`, an unlisted CLI light falls
through to `attach_background_agent`, which ran `open -na Ghostty` — `-na` starts a **second
instance of the application** — running `claude attach 1550acaa`. There is no session behind a
spare, and `attach` on an id with no live job does not fail: it lands in Claude Code's **agent
view**, a list of every session. Hence a new window full of copies of the user's sessions.

The daemon log dates the whole sequence, including the agent view claiming a spare of its own:

```
18:28:36  bg claimed-spare 1550acaa (spare)    ← the phantom light appears
18:28:50  bg claimed-spare a011978b (fleet)    ← "fleet" = the agent view, 14 s later
18:30:24  bg settled 1550acaa (killed)
```

Two things were confirmed *not* to be defects: the other new lights (`0e984070`, `fe137997`)
are real background sessions — Claude Code spawns one per `/stop`, which the daemon log records
as `bg spawned … (slash)` — and `claude attach` on a settled agent legitimately wakes it
(observed: `Waking session 0e984070`, followed by a fresh spare).

### The latent defect found underneath it

While verifying the tab work below, a scripted Ghostty tab reported `command not found: claude`.
The cause is not Ghostty's: **`zsh -lc` is non-interactive**, so it reads `.zshenv` /
`.zprofile` / `.zlogin` and never `.zshrc` — and `.zshrc` is where Claude Code's installer puts
`~/.local/bin` on this machine. Decision 054 assumed a login shell was enough to reach `claude`
from a GUI app, and it is not:

| Environment | `zsh -lc "claude …"` | absolute path |
|---|---|---|
| Launched from a shell (`./install.sh` relaunches this way) | works — inherits the developer's PATH | works |
| Launched from Login Items / Finder (`PATH=/usr/bin:/bin:/usr/sbin:/sbin`) | **command not found** | works |

So `cli_facts_query` — the query decisions 054 and 064 both rest on — returned `None` for every
user who launches the bar the way the README tells them to, and being fail-open it did so
silently: no CLI light was ever reconciled, and no spare was ever pruned. It only ever worked
in development because a shell-launched app inherits the developer's PATH. This is why the
symptom needed a fix on the *click* as well as on the light: the light-side rule was inert.

### Decision

Four changes, all app-side. No hook change, no status-file schema change.

1. **`claude_bin()` resolves the binary once**, by absolute path: whatever the inherited PATH
   resolves (correct when the app *was* started from a shell, and honors a non-standard
   install), then `~/.local/bin/claude`, `/opt/homebrew/bin/claude`, `/usr/local/bin/claude`,
   then the bare name as a last resort. `cli_facts_query` runs it directly with **no shell at
   all** — verified to work on a bare `PATH=/usr/bin:/bin` — and every `claude attach` path
   passes the absolute path too.
2. **A click on a CLI light Claude Code does not list does nothing.** If the query *answered*
   and the id is absent, there is no session to reach, so `focus_session` returns rather than
   attaching. When the query itself fails, the old behaviour stands — fail-open, the #048
   contract.
3. **An unlisted light whose pid owns no terminal is dropped on sight**, instead of waiting out
   the 20 s grace. A spare never has a controlling terminal, so there is nothing to wait for,
   and the grace was long enough for the phantom to be clicked. A pid *with* a tty is a real
   session however Claude Code lists it, so that case keeps the full 20 s unchanged — as does a
   pre-054 status file with no `pid` at all, which says nothing about a terminal either way.
4. **A background agent opens in a tab of the running Ghostty**, via 1.3's scripting dictionary
   (`new tab in front window with configuration {command:…}`, or `new window` when Ghostty has
   none open). Decision 054 had to use `open -na Ghostty` because every scripted way of
   starting a surface in 1.2.x reported success and started nothing. That second instance is
   also what put sessions beyond the reach of decision 055's tab focus, so this removes 055's
   "two Ghostty instances" limitation at the source. Ghostty older than 1.3 falls back to 054's
   second instance, which still reaches the agent.

### Why this shape

- **The click gets the strict rule, the light gets the lenient one.** A light that turns out to
  be a spare costs a second of confusion; a click that opens the wrong thing costs a window and
  a lost train of thought. So the click acts only on positive evidence, while the light keeps
  its grace period wherever a terminal says a real session is behind it.
- **No new signal.** Every input already existed — Claude Code's session list, the recorded
  pid, the tty. What changed is that they are now *reachable*, and that "Claude Code says this
  is not a session" is treated as an answer rather than as a reason to guess.
- **Fail-open is preserved throughout.** A failed query still reconciles nothing and suppresses
  no click.

### Verification

- **`claude_bin` under the real launch method.** `env -i HOME=… PATH=/usr/bin:/bin` →
  `claude agents --json` through a login shell returns `command not found`, while the resolved
  absolute path returns the full session list. The new `dump_cli_facts` test prints both and was
  run under exactly that stripped environment: `claude bin = ~/.local/bin/claude`,
  `6 session(s)`.
- **End-to-end, on the packaged app, launched the way users launch it.** The bar was killed and
  restarted with `env -i HOME=… PATH=/usr/bin:/bin open -a AgentStatus`; `ps -E` confirmed the
  process really was running on `PATH=/usr/bin:/bin`. A synthetic unlisted CLI light with
  `pid: 1` (alive, no controlling terminal — a spare's signature) was **pruned within 1 s**.
  That single observation exercises the whole chain: binary resolved, query answered, rule
  fired. Before this change the same probe would have survived, because the query could not run.
- **Negative control, same app:** an unlisted light whose pid *does* own a tty survived 8 s, so
  the rule discriminates instead of clearing every CLI light.
- **`cli_liveness_pruning` extended** with a `spare` fixture, and it immediately earned its
  keep: the first cut of rule 3 pruned a pre-054 file with no `pid`, which 054 requires to fall
  back to the idle timeout. Fixed by requiring `pid > 0`. The test now models a real session
  with a pid discovered from `ps` rather than the test process's own, which has no tty when the
  suite runs under a background agent.
- **Ghostty tabs, live on 1.3.1.** `new tab in front window with configuration {command:…}`
  creates the tab, selects it, runs the command, and `activate` fronts Ghostty — checked with a
  marker command visible in `ps`. Then through the real `attach_background_agent`: window count
  stayed **1** and `pgrep -x ghostty` stayed **1**, where the old path would have started a
  second instance.
- `cargo build --release` clean, no new warnings; `cargo test --release` 2 passed, 10 ignored;
  both node test suites pass.

---

## 065 — A finished background agent's light retires; a session with a terminal keeps its own

**Date:** 2026-08-13
**Status:** Accepted

### Context

Reported live: *"i am still able to see lights for sessions where i ran stop… those should
disappear from my lightbar once that command goes through"*, clarified as *"stopped processes
(that just finished working and are now idle) can stay, but processes that exited or quit
(sessions that we typed exit or have no currently active tab or terminal anywhere) should
disappear."*

The bar at that moment, with each light checked against whether anything could still be
reached through it:

| light | host | kind | terminal | quiet |
|---|---|---|---|---|
| `0e984070` | cli | background | — | 30 min |
| `a011978b` | cli | background | — | 32 min |
| `bbfb4f32` | cli | interactive | ttys005 | 9 s |
| `ccbaa786` | cli | interactive | ttys001 | 38 min |
| `6fb0bb46` | claude-desktop | interactive | — | 86 min |
| `74dbc1ee` | cli | background | — | busy |

The two stale lights are exactly the **background** jobs. Nothing in decision 054 could
retire them: (e) prunes on the owning pid dying, and Claude Code keeps a finished `--bg` job's
process alive and still listed in `claude agents --json` indefinitely; (f) only prunes what
Claude Code does *not* list. So they sat until the two-hour `MAX_IDLE_SECS` backstop. A `--bg`
job also has no terminal by construction, which is why the user's "no active tab or terminal
anywhere" describes them precisely.

The interactive half already worked, and was confirmed rather than assumed: `bbfb4f32` was a
session the user had typed `exit` in — its `claude` process was gone while the shell on
`ttys005` remained, Claude Code had dropped it from its list, and its light was gone within
seconds.

### Decision

A seventh prune rule (g): a `cli` light is retired when **Claude Code reports it as a
background job that is idle** (`kind: "background"`, `status: "idle"` — a new `status` field on
`CliFact`, straight from `claude agents --json`) **and** it has been hook-silent for
`CLI_BG_DONE_SECS` (5 minutes).

Never applied when the light is `blocked` or `error`. Those are the two states the user has to
act on, and both go quiet by their nature — waiting on a permission prompt emits no further
events — so a silence timer is exactly the wrong thing to point at them (UI Principle #2).

### Why this shape

- **It reconciles both halves of the request.** "Just finished and idle can stay" and
  "abandoned should disappear" are the same light at different ages, so the answer is a clock,
  not a category. Five minutes keeps the finished-agent signal readable — the light is the only
  place it appears — while a job left overnight stops occupying a slot.
- **Hook silence, not wall-clock age**, so a background job that picks work back up resets its
  own timer and never vanishes mid-run.
- **Claude Code's own word for the verdict**, the #048 pattern: the bar does not infer that a
  job is done, it asks. A failed query reconciles nothing.
- **Interactive sessions are untouched.** The rule tests `kind == "background"`, so a terminal
  session idling in an open tab keeps its light however long it sits there — which is what the
  user asked for, and what the `ccbaa786` line above would otherwise have lost.

### Verification

Live, on the packaged app, before and after installing the change: the two background ghosts
(`0e984070`, `a011978b`) were **gone within one poll**, while `ccbaa786` (interactive, idle
38 minutes, live tab), `6fb0bb46` (Claude Desktop) and the busy background job running this
work all survived — so the rule discriminates on `kind` and `status` rather than clearing
anything quiet. `cargo build --release` clean with no new warnings; `cargo test --release`
2 passed, 10 ignored; `cli_liveness_pruning` still passes.

**Caveat on the reproduction:** `0e984070` was still alive partly because a diagnostic
`claude attach` run during decision 064's investigation woke it (`Waking session 0e984070` in
the daemon log). The rule was therefore verified against `a011978b` as well, which no
diagnostic ever touched, and against the general shape rather than that one light.

---

## 060 — The tooltip names the application a session runs in

**Date:** 2026-08-13
**Status:** Accepted

### Context

Requested: *"when you hover over a light in the lightbar, it should also tell you which
application that session is in (ex: vscode, ghostty, etc)"*.

The tooltip's first line was `folder · session name — state` (decision 053). It answers
*which project* and *which session*, never *where*. That gap grew with the host list: since
decision 054 a light can be Claude Code in a VS Code tab, in a terminal, or in Claude
Desktop, or a Cursor agent — four applications rendered as the same colored dot, so the only
way to find out which one a light means was to click it.

The application is also the one part of a light's identity the bar already knows and throws
away at render time: `ide` is on every session and drives both pruning and click routing.

### Options considered

| Option | Pros | Cons |
| --- | --- | --- |
| **A. Map `ide` in the frontend** (`cli` → "Terminal") | No backend change, no cost | `cli` covers *every* emulator, so it could not say **Ghostty** — the example in the request. It would name a category, not an application |
| **B. Record the app in the hook** | Resolved once, at the source | Puts a process walk (`ps` per generation) on the user's turn — Guideline #3. The hook currently spawns nothing |
| **C. Resolve it app-side from the pid the hook already records** (chosen) | Names the real emulator; the walk already exists (`terminal_app_of`, written for click-to-focus); costs nothing on the user's turn | Runs in the poll, so it needs a cache; a background agent has no application at all and needs its own answer |

### Decision

Option C. `list_sessions` returns a new `app` field and the tooltip head becomes
`folder · name (app) — state`:

- `vscode` → `VS Code`, `cursor` → `Cursor`, `claude-desktop` → `Claude Desktop` — the `ide`
  field spelled the way the application is named on screen.
- `cli` → the terminal emulator hosting it (`Ghostty`, `Terminal`, `iTerm2`), from
  `terminal_app_of(pid)` — the ancestor walk decision 054 already uses to decide where a
  click goes. It falls back to the literal `terminal` when the walk finds no app bundle.
- A detached background agent → `background agent`, never an application name. It has no
  terminal by construction and the walk would find Claude Code's own launcher
  (`ClaudeCode.app`), which is where the process lives, not where the user would land.

### Reasoning

- **The tooltip and the click must agree.** Interactive-vs-background is decided here exactly
  as `focus_session` decides it — Claude Code's reported `kind` when the query answered, the
  presence of a tty when it did not. A tooltip that reads `Ghostty` for a session a click
  cannot take you to a Ghostty tab would be a lying label (UI Principle #4).
- **Naming the emulator is the whole point of the request.** "Terminal" is what the status
  file already implies; `Ghostty` is what the user asked for and the only version that tells
  two terminal lights apart when one is in Ghostty and one in Terminal.app.
- **It goes app-side, not in the hook** (Guideline #3). The pid the hook records is enough
  for the app to answer the question on its own, so nothing new runs inside a session.
- **Memoized per session.** The tooltip is rebuilt on every 1 s poll and the walk costs one
  `ps` per process generation, so the result is cached under the session id *and* pid — the
  `claude` process owning a session never changes, so the only way to stale the entry would
  be pid reuse under the same session id, which cannot happen, and a differing pid recomputes.
- **Parenthesized after the identity, not appended to the state.** The application qualifies
  *which session this is*, and putting it last would split `finished — click to acknowledge`.
- No status-file schema change, no hook change, no new permission.

### Verification

- `cargo test -- --ignored --nocapture dump_host_apps` (new; prints the exact tooltip string
  for every live light) against the four sessions running at the time:
  `1616d3bb ide=cli app=Ghostty`, `ccbaa786 ide=cli app=Ghostty`,
  `6fb0bb46 ide=claude-desktop app=Claude Desktop`,
  `74dbc1ee ide=cli app=background agent` — a terminal session resolved to the emulator
  hosting it, and the background agent was not called `ClaudeCode`.
- `node app/tests/tooltip-head.mjs` — two new cases on the shipped `appTag`/`headFor`
  (`AgentStatus · agentstatus-5b (VS Code)`, and an empty `app` rendering exactly as before,
  which is what a status file written by an older build produces).
- `cargo check` and `cargo test` clean (2 passed, 11 ignored); `node app/tests/unread-light.mjs`
  passes.
- **Not verified live:** `VS Code` and `Cursor` on a running session of each — no window of
  either was open. Both are constants with no resolution step, unlike the `cli` path that was
  checked.

---

## 061 — A click reaches a background agent where it already is, instead of opening another one

**Date:** 2026-08-13
**Status:** Accepted

> **Numbering note.** This entry took 061 because 060 was the highest number in the file when
> it was written. Three numbers were duplicated by concurrent sessions around this point —
> 056, 057 and 062 each named two decisions. That was resolved afterwards by renumbering this
> session's three entries to **064**, **065** and **066**; the other session's kept the
> original numbers. See the end of this entry.

### Context

Reported live: *"i tried clicking around in the lightbar and the ghostty light kept opening a
new tab."*

Reproduced through `focus_session` itself — the real click path, not the leaf it happens to
pick — against the background job on the bar. Ghostty surface count: **2 → 3 → 4**, one new
terminal per click.

The cause is that the attach path had no memory. A `kind: background` session has no terminal
to focus, so decision 054 routes its click to `claude attach`, and nothing ever asked whether
that agent was *already* open somewhere. Decision 064 made this far more visible by turning
"a whole second Ghostty instance" into "a tab in the window you are looking at" — the same
defect, now stacking up in front of the user.

Decision 055's matcher cannot be reused as-is here. Once an agent is attached, its tab carries
the same session title as every other view of it, so this session had **three** matching
surfaces; 055 requires exactly one and would have declined, then attached again.

### Decision

`focus_ghostty_surface` takes a `require_unique` flag, and the two callers set it opposite:

| Caller | `require_unique` | Why |
|---|---|---|
| Interactive session's light (#055) | **true** | The alternative is fronting the app. A wrong tab is worse than no tab (UI Principle #4) |
| Background agent's light (this) | **false** | The alternative is *creating another terminal*. Any surface already showing the session beats one more, so the first hit wins — and several hits is the normal state here |

`attach_background_agent` then tries, in order: focus a surface already showing this session;
else, if `pgrep -f "claude attach <short>"` says an attach is already running — the case where
Claude Code has not titled that young tab yet, which is exactly the tab a second click would
duplicate — front Ghostty without adding to the pile; else open the tab.

### Why this shape

- **A click is "take me to that session", never "give me another copy of it"** (UI Principle
  #3). Opening is what you do when there is nothing to go to, so it belongs last.
- **The pgrep step covers the gap the title cannot.** Title matching needs an `ai-title`, which
  a just-opened attach does not have yet; its argv names the session regardless.
- **Nothing new is trusted.** Both signals — the surface title and the attach process's own
  argv — already existed; neither is inferred.

### Verification

Through `focus_session`, the real click path, with a decoy focus set on an unrelated tab before
each click so a no-op would be visible as one:

| | surfaces | selected tab |
|---|---|---|
| before | 2 | — |
| **before the fix**, clicks 1–3 | 3, then 4 | one new terminal per click |
| decoy | 4 | 3 |
| **after the fix**, click 1 | **4** | **1** (a surface showing the session) |
| decoy | 4 | 3 |
| **after the fix**, click 2 | **4** | **1** |
| decoy | 4 | 3 |
| **after the fix**, click 3 | **4** | **1** |

`cargo test --release` 2 passed, 11 ignored. New `focus_click_live` test drives
`focus_session` end to end, which is what caught this in the first place — the earlier
`focus_terminal_live` exercised a leaf and so could not see a routing bug.

**A false verification, recorded because it nearly shipped.** The first run of the check above
reported "no new tabs" and looked like a pass. The test binary had failed to compile — the
signature change above broke `focus_ghostty_live`, and the run was piped to `/dev/null`, so
the clicks never happened and the unchanged count read as success. Fixed by asserting the test
actually ran (`grep -q "1 passed"`) before believing a count. A verification that cannot fail
is not a verification.

### The numbering collision, and how it happened

Three sessions were editing this repository at once. Two effects, both recorded rather than
quietly cleaned up:

1. **Duplicate numbers.** Three of them, in the end: 056 was both "Re-adding Codex and Gemini"
   and "A light that names no session does nothing"; 057 was both "Token/cost tracking" and
   "A finished background agent's light retires"; 062 was both "A light keeps the slot it
   arrived in" and "A surface whose title ends with the session title". Each session appended
   after reading the file before the other's append, so each honestly believed it had taken
   the next free number.
2. **Commits d0d07f8 and 970aa75 swept in another session's uncommitted work**, because they
   staged with `git add -A` in a shared tree. The other session's decisions 056–059 are in
   those commits despite being unrelated to them — and the reverse happened too: 5a6cef1
   swallowed this session's 061. `git add -A` is the wrong verb in a shared tree; stage
   explicit paths.

**Resolved.** The duplicates were renumbered in a single coordinated pass: the entries written
by *this* session became **064**, **065** and **066**, and the other session's kept 056, 057
and 062, since those appear first in the file in all three cases. The sessions agreed the
split over the peer-messaging channel first, which is also how the in-flight 063 got its
references updated in the same pass instead of being broken by it. Every reference — in this
log, in `NEXT_STEPS.md`, and in `lib.rs` comments — was rewritten with it. Commit messages
still cite the old numbers; they are immutable history and are left alone.

---

## 062 — A light keeps the slot it arrived in

**Date:** 2026-08-13
**Status:** Accepted

### Context

Reported live: *"the bug where lightbar lights keep moving around"*, confirmed as lights
swapping places with each other rather than the strip sliding on screen.

`sortSessions()` re-ordered the whole strip on every 1 s poll. In the default mode the key
was the session's `cwd`, then its id:

```js
function byWindow(a, b) {
  const c = (a.cwd || "").localeCompare(b.cwd || "");
  if (c) return c;
  return a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
}
```

Both halves move lights that did nothing:

- **The id is a random uuid.** Every session in the same folder — the normal case, several
  agents in one repo — is ordered by a random string, so a new session lands at an arbitrary
  position *among the existing lights* and shoves every later one along.
- **`cwd` is not fixed for a session.** The hook writes the payload's `cwd` on every event,
  so an agent that `cd`s changes its own sort key and jumps to another group.

Decisions 064 and 065 made this constant rather than occasional: Claude Code starts a
background agent per `/stop` and pre-warms spares, so sessions appear and disappear all day.
The result is a light moving out from under the pointer between seeing it and clicking it,
which attacks the two properties the bar exists for — glanceability and "the thing you look
at is the thing you click" (UI Principles #1 and #3).

### Options considered

| Option | Pros | Cons |
| --- | --- | --- |
| **A. Arrival order, replacing the window mode** (chosen) | A light never moves once placed; a new session always appends; a removal only closes a gap | Folder grouping is gone |
| **B. Keep folder grouping, stabilize inside it** | Folders stay together | A new session still appends to *its* group, so every light in a later group shifts — the movement is reduced, not removed |
| **C. Add arrival as a third mode** | Nobody's current behaviour changes | Three ordering modes for a bar of dots, and the reported bug stays reachable |

### Decision

Option A, chosen by the user. `arrivalSeq` (a `Map` of session id → slot) hands each session
the next free slot the first time a poll sees it and releases it when the session is gone.
Sessions new in the same poll are numbered in the order the backend delivered them — `label`,
then id — so the first layout after a launch is deterministic rather than accidental. Urgency
mode is unchanged in shape and now breaks its ties by arrival instead of by folder.

The settings segment becomes **Stable | Urgency**. A stored `"window"` from an earlier version
reads as stable, so nobody has to re-pick.

### Reasoning

- **A light is a click target before it is an indicator.** Its position is part of its
  identity: users learn "the second one is the frontend" and click by memory. Any ordering
  computed from mutable data breaks that, however sensible the data.
- **Grouping cost more than it paid.** It never separated the common case (several sessions in
  one repo all share a `cwd`), it merged two windows on the same folder anyway, and the folder
  is the first thing in the tooltip. Sorting by a field the hook rewrites every event was the
  bug, not an implementation detail of it.
- **Arrival cannot be read off the status file.** `report.sh` writes atomically (temp file +
  `mv -f`), so the file's birth time resets on every event — checked rather than assumed. The
  bar tracks arrival itself, which also keeps this frontend-only: no hook, schema, or Rust
  change.
- **Slots are released, not remembered.** A returning session is a new arrival and appends,
  which keeps the map bounded by the number of live lights and avoids a light reappearing in
  the middle of the strip long after it left.

### Verification

`node app/tests/light-order.mjs` (new; evals the shipped `sortSessions`/`arrivalSeq` out of
`main.js`): the backend's order is the first layout; a new session with an id that sorts first
and a folder shared with an existing light still appends; a re-ordered backend list does not
move anything; a removal closes its gap; a returning session appends rather than reclaiming its
old slot; the map shrinks to the live count; urgency still leads with error/blocked and breaks
ties by arrival. `unread-light.mjs` and `tooltip-head.mjs` still pass.

**Left to confirm live:** the reported behaviour itself — the reporter watching the bar through
a few sessions starting and finishing. The bar's own display cannot be screenshotted from here
(the terminal has no Screen Recording grant), so this is the user's check.

---

## 066 — A surface whose title *ends with* the session title beats one that merely contains it

**Date:** 2026-08-13
**Status:** Accepted

### Context

Reported live: *"sometimes the click into ghostty works, but sometimes clicking the light for a
ghostty session does nothing."*

Measured rather than guessed. Every CLI light was driven through `focus_session` itself with a
decoy focus set on an unrelated surface first, so a click that did nothing would show as one:

| light | ai-title | unique surface match | click |
|---|---|---|---|
| `1616d3bb` | yes | yes | lands |
| `c6caa7f3` | yes | yes | lands |
| `ca4462c5` | yes | yes | lands |
| `74dbc1ee` | yes | yes | lands |
| `ccbaa786` | **none** | — | **no-op** |

The pattern is exact: a click lands when decision 055's title match finds its surface, and does
nothing when it declines. Nothing is silently *failing* — the fallback is `activate Ghostty`,
and when Ghostty is already the front application, activating it changes nothing on screen. A
declined match and a dead click are the same event to the user.

*(Two earlier readings of this table were wrong and were thrown out: one probe read
`window 1` while Ghostty had two windows, and one used the session's own surface as the decoy,
so a correct focus registered as "no change". Both were re-measured.)*

### The part that is fixable

055 matched with `contains`, and required the match to be unique. So an unrelated surface whose
title merely *spans* the session title counted as a second hit and the match declined —
a dead click caused by a bystander. Reproduced by giving a surface the title
`Working on Check agent status 6e location in lightbar right now` while a live session was
titled `Check agent status 6e location in lightbar`: `contains-hits=2` (declines),
`ends-with-hits=1`.

Claude Code writes the title as a **trailing** run — `◑ <title>`, the glyph being the activity
spinner. So the match now has two grades:

- **strong** — the surface title *ends with* the session title. Several strong hits are several
  views of the *same* session, so the first is right by construction and no uniqueness test is
  needed. This is what an attached background agent produces alongside its own view.
- **weak** — the title merely appears somewhere inside. It may belong to something else, so
  this grade still has to be unambiguous to act on (and still honours 061's `require_unique`).

### The part that is not fixable

`ccbaa786` has no `ai-title` at all — no transcript was ever written for it, because it is a
terminal the user only ever used to start a background agent. Worse, its tab is titled with
*that agent's* title, so no title of its own could ever match it. Every other key Ghostty
exposes was checked and none identifies it:

| key | result |
|---|---|
| `ai-title` | absent, and permanently so for a parked terminal |
| `working directory` | identical (`…/agentstatus`) across **all five** surfaces |
| tty / pid per surface | not in the dictionary, and not in the environment either — 1.3.1 exports only `GHOSTTY_RESOURCES_DIR`, `GHOSTTY_BIN_DIR`, `GHOSTTY_SHELL_FEATURES`, `TERM_PROGRAM*` |
| `parkedJobId` in `~/.claude/sessions/<pid>.json` | present but **stale** — it named a job that had already finished, while the tab showed a different one |
| elimination (the surface no other session claims) | actively wrong: this session *shares* its tab with the agent it displays, so elimination points at an unrelated shell |
| `+list-actions` | `set_surface_title` could stamp an identity, but writing to the user's terminal to find it is not acceptable |

So this class of click cannot be made to land, and the README now says so plainly rather than
leaving it as a surprise.

### Verification

- **The collision, before and after.** With the bystander surface present: the old rule counted
  2 and declined; the new rule counts 1 strong hit. Driven through `focus_session` twice, the
  click landed on the session's own surface (`8F47B31F`) both times.
- **No regression.** Full sweep afterwards, decoy before each click: `c6caa7f3` → its surface,
  `ca4462c5` → its surface, `74dbc1ee` → its surface (re-run with an independent decoy after
  the first attempt used its own surface), `ccbaa786` → still a no-op, as the limit above
  predicts.
- `cargo build --release` clean, no new warnings; `cargo test --release` 2 passed, 11 ignored.
- Every surface created for these experiments was removed; Ghostty was returned to the three
  real session surfaces it started with.

---

## 063 — A background agent's light says what Claude Code says: green while it works, orange while it waits

**Date:** 2026-08-13
**Status:** Accepted

### Context

Investigating "which session is `agentstatus-6e`" turned up a light that was missing rather
than mislabelled, and the trail ended in decision 065.

A `--bg` job keeps its own record in `~/.claude/jobs/<id>/state.json`, with every transition
logged to `timeline.jsonl`. For `74dbc1ee`:

```
14:28:25  working  something happened and a new light appeared on my bar…
15:08:00  done     diagnosed phantom lights: stale bg jobs, fixed pruning rules
15:17:07  working  Inspecting Ghostty tabs after the user's clicks
15:30:42  blocked  bug fixed & verified; awaiting sequencing decision on concurrent edits
```

So Claude Code has three words for a background job — `working`, `done`, `blocked` — and
`blocked` means *it stopped and the ball is in the user's court*. `claude agents --json`
carries it as `state`, next to the `status` (`busy`/`idle`) decision 065 already reads.

`report.sh` cannot see any of that. It maps **hook events**, and `blocked` has exactly one
source there: `PermissionRequest`. A background job that ends a turn to ask a question fires
`Stop`, which writes `idle`. Decision 065's guard — "never retire a `blocked` or `error`
light" — therefore never matched the case it was written for, because for a `--bg` job that
case does not arrive as `blocked`.

Measured live rather than reasoned about:

| time | event | source |
| --- | --- | --- |
| 15:30:42 | job → `blocked`, awaiting the user's decision | `jobs/74dbc1ee/timeline.jsonl` |
| 15:30:42 | `Stop` hook writes `state: "idle"` | `report.sh` event map |
| ~15:35:42 | rule (g) matches → status file deleted, light gone | `CLI_BG_DONE_SECS` |
| 15:41:33 | `agents --json` says `status: idle, state: blocked` — **no status file** | measured |
| 15:42:59 | the user answers | timeline |
| 15:45:31 | light returns, `running` | status file |

**Seven minutes with no light on an agent that was waiting for an answer** — the exact miss
UI Principle #2 exists to prevent, and it also took away the click target: a background
light's click opens the session in a Ghostty tab (#064/#061), so no light means no way in.

A second question from the same session — *"if these two are in a working state, why are they
not green?"* — has a different answer, and it constrains the fix. `a011978b` and `fe137997`
both reported `state: working` while doing nothing at all:

| agent | started | CPU used | transcript last grew | job `tempo` | `status` |
| --- | --- | --- | --- | --- | --- |
| `74dbc1ee` | 14:22 | 6:13 | 15:58, 2.5 MB | `active` | **busy** |
| `a011978b` | 14:28 | 1:00 | 14:28 — frozen 1h31m | `idle` | idle |
| `fe137997` | 15:51 | — | 15:51, 2.4 KB — never used | `idle` | idle |

`state` is what the job last *declared*: it is written `running` at spawn and only advances
when something updates it, so a job that never starts sits at `working` forever. `status` is
live. Driving the light off `state` alone would have put green on two dead jobs (UI Principle
#4); driving retirement off `status` alone is what deleted the waiting one.

### Decision

Read both, and let each answer the question it can. `CliFact` gains `job_state` (the `state`
field, already in the JSON the app fetches every 10 s), and two small pure functions decide:

| `status` | `state` | light | retired by the 5-min timer? |
| --- | --- | --- | --- |
| `busy` | any | **running** (green) | no |
| `idle` | `blocked` | **blocked** (orange) | **no** |
| `idle` | `done` | as the hook wrote it | yes |
| `idle` | `working` | as the hook wrote it | yes |

Only a light whose own hook last wrote `idle` is reconciled, so `running`, `blocked` and
`error` — anything a hook actually observed — always win, and interactive sessions are
untouched by both halves (`kind == "background"` gates them).

### Why this shape

- **Orange keeps meaning one thing.** The invariant the user set for this change: an orange
  light always says *this session is waiting on a decision from you*. `blocked` is the only
  pair that produces it — `done` and `working` never do — and it is Claude Code's own word for
  a job that stopped to ask, the same category as the permission prompt orange already means.
  The one `blocked` that is *not* a wait is the instant a prompt is submitted, and that cannot
  reach the bar: `UserPromptSubmit` writes `running`, and only an `idle` light is reconciled.
- **Green is the live signal, not the declared one.** `status: busy` is computed per process;
  it was the half that stayed true on all three jobs in the table above.
- **065's cleanup survives intact.** `done` and stale `working` jobs still retire at five
  minutes — verified against `fe137997`, which the new rule still marks retirable.
- **The bar asks rather than infers**, the #048 pattern: a failed query reconciles nothing.

### Verification

- `bg_job_state_reconciliation` (new unit test, not live-gated) pins all four pairs plus the
  interactive cases in both directions.
- `dump_cli_facts` now prints the resolved light and retirability per session. Live, against
  the seven sessions open at the time: `74dbc1ee` (busy/working) → `running`, not retirable;
  `fe137997` (idle/working, never started) → unchanged and still retirable; all five
  interactive sessions untouched.
- `cargo build --release` clean, no new warnings; `cargo test --release` 3 passed, 11 ignored.

---

## 067 — An interrupted turn greys its own light, because Claude Code says the turn is over

**Date:** 2026-08-13
**Status:** Accepted (implements decision 059, whose live check this closes)

### Context

Reported live: *"when I stop a session manually, the light still shows up as green but I want
it to go to gray immediately."* Clarified by the user to mean **Ctrl+C on a running session**
(and Esc, if it behaves the same), with one boundary attached: *"closing the tab should
continue to show the green light if it is actually still working in the background."*

The cause is structural, not a bug in any one path. `report.sh` maps hook **events**, and it
has exactly one route to `idle`: a `Stop` event (or `SessionStart`). An interrupted turn does
not fire `Stop` — the turn is abandoned, the session returns to its prompt, and nothing is
emitted. So the light keeps whatever the last `PreToolUse`/`PostToolUse` wrote, which is
`running`, **forever**. Decision 059 already catalogued this family (dropped end-of-life
events, an observed 95-minute stuck green light) and proposed a guarded reconcile, but was
parked as *"needs live check"* on one unverified precondition: whether an interactive Claude
Code session reports a `status` at all.

Measured, not assumed (Guideline #4). Sampling `~/.claude/sessions/<pid>.json` against the
lights every 2 s across the user's own sessions caught the exact event:

| time | Claude Code `status` | light the hook wrote | |
| --- | --- | --- | --- |
| 16:42:41 | `busy` | `running` | prompt submitted — the two agree within 0.1 s |
| 16:42:47 → 16:43:02 | `busy` | `running` | turn runs; hook events keep the light fresh |
| **16:43:04** | **`idle`** | **`running`** | **Ctrl+C — no `Stop`, and the light stays green** |

`statusUpdatedAt` on that last row is `1786653783840`; the light's last hook event was
`1786653780`. Claude Code's answer is **3.84 s newer than the light**, and that gap is the
whole fix.

The same sampling mapped the vocabulary, which turns out to line up with the bar's states one
for one — and answered 059's open question in the affirmative (`f548b1eb`, `kind:
interactive`, `status: idle`):

| `status` | what the hook independently wrote | seen on |
| --- | --- | --- |
| `busy` | `running` | interactive + background |
| `waiting` | `blocked` | interactive |
| `idle` | `idle` | interactive + background |
| `shell` | `idle` | interactive (one session, 10 min unbroken) |
| *key absent* | — | **every Claude Desktop session** |

Two sources carry this. `claude agents --json` was rejected in favour of the per-pid file:
it costs a subprocess (hence its 10 s cache, which is not "immediately"), it **loses**
information — it reported session `c6caa7f3` as `busy` for ten straight minutes while the
per-pid file correctly said `shell` — and the app already reads that same directory every
poll for tooltip names (#053), so the status is one extra field on a read that happens anyway.

### Decision

`claude_session_names` becomes `claude_session_facts`, returning `name` + `status` +
`statusUpdatedAt`, and a light is greyed when Claude Code says the turn is over:

```
ide != cursor  &&  hook wrote running  &&  not a background job
               &&  status == "idle"
               &&  statusUpdatedAt >= (light updated_at + 1) * 1000
                                        →  idle, and detail cleared
```

The name-map TTL drops from 5 s to 1 s (poll rate). It was 5 s because a name never changes;
a status changes at every turn boundary, and "immediately" is the requirement.

### Why this shape

- **Positive evidence only, the #048/#052 pattern.** The session must actually say `idle`. An
  absent status (every Claude Desktop session), an unreadable record, or a session Claude Code
  does not list all reconcile nothing — the failure mode is exactly the pre-067 behaviour, so
  this can never invent a grey light (UI Principle #4).
- **The answer must outrank the light.** `statusUpdatedAt` must fall in a **strictly later
  second** than the hook event the light was drawn from, so anything the hook actually observed
  wins over a stale answer. Strictly later, not merely greater: the hook stamps whole seconds
  (`date +%s`), so within one shared second the two clocks cannot be ordered and the tie goes
  to the hook. Costs up to 1 s of latency to guarantee a starting turn is never grey for a poll.
- **It clears `detail`, and that is not incidental.** The frontend renders an idle light
  carrying a `detail` as the white *"finished — click to acknowledge"* light (#014/#050). At an
  interrupt the last `detail` is a tool line (`$ sleep 90`), so flipping state alone would have
  raised an **attention light on a session the user is by definition already looking at**,
  captioned with a tool call that was cancelled. `detail` means one thing — the wrap-up message
  from `Stop` — and an interrupted turn has none, so the honest value is empty. The `task` line
  (the prompt) survives.
- **The user's boundary is honoured by the data, not by a special case.** A session still
  working after its tab closes reports `busy`, so nothing greys it; if its process is gone,
  #054's pid liveness removes the light entirely. Neither path needed touching.
- **Background jobs are excluded.** Their `status` is `idle` between turns *while the job is
  alive and working* (`fe137997`: `status: idle`, `state: working`), so it does not mean the
  same thing there — a `--bg` light is decided by #063 and #065 from `state`, untouched.
- **`shell` is deliberately not treated as idle.** It was observed on a live session for ten
  unbroken minutes and plainly is not a running turn, but what produces it is unconfirmed on
  2.1.227, and Guideline #4 does not spend a lying light on a guess. Revisit if a stuck green
  light ever shows `status: shell`; `dump_turn_reconcile` prints exactly that.
- **No hook change and no schema change.** The signal layer is untouched, so nothing new runs
  inside the user's sessions (Guideline #3), and the fix covers every missing-`Stop` case —
  including 059's 95-minute stuck light — not just interrupts.

### Verification

- `interrupted_turn_greys_its_light` (new unit test, not live-gated) pins both guards using
  **the real timestamps measured off the Ctrl+C above**, plus the declines: `busy`, `waiting`,
  `shell`, empty status, no record, a stale answer, and the same-second tie.
- `dump_turn_reconcile` (new, `cargo test -- --ignored --nocapture dump_turn_reconcile`) is the
  re-runnable diagnostic for the next "stuck green light" report: it prints, per light, what
  the hook wrote, what Claude Code says, both timestamps, and whether the reconcile fires. Run
  live against the two open sessions, it correctly declined on both (one genuinely `busy`, one
  already `idle`).
- **Confirmed live on the installed build**, user-reported gray on screen. Session `e7041031`:
  `16:58:48 hook=running claude=busy` (turn starts) → `16:58:56` the hook's last write is still
  `running`, **no `Stop` ever arrived**, and Claude Code says `idle` — the reconcile fires and
  the light greys on the next poll. This landed on the *tight* case rather than a comfortable
  one: only **1.5 s** separated the hook event (`…734`) from Claude Code's answer (`…735546`),
  barely over the strictly-later-second threshold, so the guard held without swallowing the fix.
- A second sighting of `status: shell` on an idle session (`93037335`, `hook=idle` +
  `claude=shell`, twice) — two observations now point the same way, still not proof of cause,
  so it stays excluded.
- Rejected as a route: driving the TUI under `expect` to synthesise an interrupt. Claude Code
  rendered nothing on the pty across four attempts (sandboxed and not, `TERM` set and not) and
  never accepted the prompt — `SessionStart` fired and `UserPromptSubmit` never did. The
  measurement above came from sampling a real session instead.

## 068 — The hook becomes a native binary, and `jq` stops being a dependency

**Date:** 2026-08-14
**Status:** Accepted (supersedes the `report.sh` half of #011; closes the `jq` item in #059)
**Evidence:** Claude Code 2.1.x on Windows 11 (native winget install, not WSL), Git for
Windows 2.x, measured on the machine that is now the Windows test box.

**Context:** The Windows port (#069) had to answer "where does `jq` come from on Windows",
since Git for Windows ships none. Investigating turned up that this was never a Windows
question: `report.sh` calls `jq` with no availability guard, `install.rs` — the DMG path, the
*primary* channel per #024 — never checks for it, and `jq` ships at `/usr/bin/jq` only since
macOS 15. A DMG user on macOS 11–14 without Homebrew already gets an inert app and no error.
#059 queued a guard for this, which converts a silent failure into a loud one but still
leaves the user with a non-working app.

**Measured, not assumed (Agent Guideline #4).** Hooks on Windows are executed through Git
Bash — verified by probe: `SHELL=C:\Program Files\Git\bin\bash.exe`, `$HOME` expanded in the
hook command, `%USERPROFILE%` did not, and a `.sh` with a shebang ran directly with its stdin
payload intact. So `report.sh` *runs* on Windows. What it costs there:

| | ms per hook event |
|---|---|
| `report.sh` (bash + jq, 8 external spawns) | **210.6** |
| `agentstatus-hook.exe` | **26.4** |
| bash spawn floor (`bash -c "exit 0"`) | 18.6 |

`PreToolUse` + `PostToolUse` fire on every tool call, so that is ~421 ms of added latency per
tool call versus ~53 ms. The cost is not `jq` specifically — MSYS emulates `fork()`, so every
command substitution is expensive. The ~18.6 ms bash floor is unavoidable either way: Claude
Code invokes the hook command through a shell regardless of what that command is.

**Options considered:**

| Option | Windows cost/event | Removes the dependency? | Risk |
|---|---|---|---|
| Bundle `jq.exe` per platform | ~210 ms | No — ships a third-party MIT binary | ~30 lines, zero behaviour risk |
| **Native binary (chosen)** | **~26 ms** | Yes, entirely | Rewrites a working signal layer |
| Perl + `JSON::PP` (ships with Git for Windows) | ~210 ms | Trades `jq` for a perl Apple has deprecated | Same rewrite risk, worse outcome |
| Node `report.mjs` | ~220 ms | No — Claude Code's installer doesn't need Node | Same rewrite risk |

Perl and Node are strictly dominated: the rewrite risk of the binary without its payoff.

**Decision:** Port `report.sh` to a **standalone crate**, `hooks/agentstatus-hook/`, outside
the Tauri package. Ship it as a Tauri bundle resource that `install.rs` copies to
`~/.claude/status/` on launch — the installed hook stays outside the app, exactly as
`report.sh` did, so it survives the app moving. Windows gets it first; macOS keeps
`report.sh` until the port is diffed against a live macOS session (there are no Windows users
to regress, and macOS users to protect).

**Why a separate crate rather than a second `[[bin]]` in the Tauri package** (learned by
hitting it): `tauri-build` validates `bundle.resources` for **every** target in its package,
so `cargo build --bin agentstatus-hook` there required the staged binary that the build
itself produces — a genuine circular dependency, which is the same build-ordering trap that
ruled out `include_bytes!`. Standalone also delivers what the design wanted anyway: the hook
carries none of Tauri's dependency graph, and `cargo test` for it compiles serde and nothing
else (13 tests in ~6 s from cold, versus a full Tauri build).

**Build wiring:** `hooks/stage-hook.mjs` builds the crate in release and copies the binary to
`app/src-tauri/resources/`, which `tauri.windows.conf.json` lists as a resource. It runs from
both `beforeBuildCommand` and `beforeDevCommand`, because Tauri resolves `bundle.resources`
for `tauri dev` too and errors on a missing path — pointing the config straight at
`target/release/` would break dev on any tree that had never done a release build. Staging
gives the config one stable path that always exists (Agent Guideline #8: a script, not a step
you remember).

**Nothing this app runs on Windows may allocate a console** (found in live use on
2026-08-14, immediately after the first real install, reported as "a terminal window keeps
popping in and out of my desktop"). There were **two** causes, and fixing the first did not
stop it — the second was only found by recording every process creation for 25 s and reading
what actually appeared, rather than reasoning about it again:

1. *The hook's own subsystem.* Covered below.
2. *The bar's polling.* `cli_facts_query` runs `claude agents --json` every
   `CLI_FACTS_TTL` (10 s), forever. The bar is a GUI process and therefore owns **no**
   console, so Windows allocated a **new** one for that child every ten seconds — measured
   as `app` → `claude.exe` → `conhost.exe` on a 10-second cadence, running whether or not
   the user was doing anything. Fixed with `CREATE_NO_WINDOW` on the spawn, via a
   `no_window()` helper that is a no-op off Windows. Nothing is lost: every such spawn is
   read through a pipe, never a terminal.

The general rule this establishes: on Windows a GUI process must pass `CREATE_NO_WINDOW`
to **every** console child, and any binary the user never launches themselves must be
GUI-subsystem. Neither is something a test catches — both programs behaved perfectly, they
just brought a window with them.

While measuring, `cursor_running()` also turned out to shell out to `pgrep` **once per
second** on Windows purely to fail (there is no `pgrep`), so it is now macOS-gated with the
same fail-open answer.

**The hook must be a GUI-subsystem binary.** A Rust binary defaults to the **console**
subsystem, so Windows allocated a console for every invocation — and the hook is invoked on
every tool call, twice. Fixed with
`#![windows_subsystem = "windows"]`, unconditionally rather than release-only: any build that
gets registered must be silent, and the attribute is a no-op off Windows. Nothing is lost —
the hook writes to neither stdout nor stderr by contract (Agent Guideline #3), and reading
piped stdin is unaffected by the subsystem (verified: it still produces a correct status file
when run from Git Bash). The Tauri template carries this same attribute on `app.exe` with a
"DO NOT REMOVE" comment; the lesson is that it applies to *any* binary the user never
launches themselves. `hooks/stage-hook.mjs` now reads the PE header and refuses to stage a
binary whose subsystem is not `2`, because no test would have caught this — the hook behaved
correctly the whole time, it just brought a window with it.

**Upgrade safety:** the installer's "is this entry ours" test now matches **both**
`report.sh` and `agentstatus-hook`. Without that, a Windows user upgrading from a `report.sh`
build would end up with two hooks registered per event, both writing the same status file.
Replacing a *running* hook binary is handled too — hooks fire on every tool call, so the
copy is skipped when the bytes already match, and on a genuine upgrade the old file is moved
aside and swept on a later launch.

**How "faithful port" is enforced rather than asserted:** `hooks/gen-golden.sh` replays 42
fixtures — 14 captured from a real Windows session, 28 synthetic for the branches a headless
run cannot reach — through the *current* `report.sh` and records its output. The Rust
implementation replays the same fixtures and must match **strictly**, with no per-field
exceptions. Confirmed end-to-end as well as in-memory: both implementations run over the
fixtures on disk produce identical status files, identical subagent markers, and an identical
`calibration.log`. Re-run `hooks/gen-golden.sh` whenever `report.sh` changes.

**Two real bugs this surfaced, both fixed in `report.sh` too:**

- **Windows paths were never split.** `cwd` arrives as `C:\Users\...` (verified live), and
  `split("/")` leaves it whole — so every Windows light was labeled with its *entire path*,
  and a file detail read `Read C:\Users\...\sample.txt` instead of `Read sample.txt`. Fixed in
  both implementations with the same shape test: normalise `\` to `/` only when the path
  starts with a drive letter or a UNC root. No macOS path starts either way, so macOS
  behaviour is provably unchanged — including that a POSIX path merely *containing* a
  backslash keeps it as part of the filename.
- **`json!` sorts keys; jq preserves insertion order.** The status file is now built from a
  typed `Status` struct so serde emits `report.sh`'s field order and the files are
  byte-identical.

**Known deliberate divergence:** an empty `tool_input.file_path` on an `Edit`/`Write`/`Read`
makes jq add a string to `null`, which errors out the whole program — so `report.sh` writes
*nothing at all* for that event. The port writes the status with an empty basename. The
port's behaviour is strictly better and no fixture covers the case, so it is recorded here
rather than replicated.

**Not yet resolved:** `pid`. The shell hook records `$PPID`; the binary records its own
parent, which under Git Bash may be `bash` rather than `claude.exe`. Everything consuming
`pid` is macOS-only or fails open, so the Windows build records `0` until the real value is
observed on a live interactive Windows session (Agent Guideline #4) — see `NEXT_STEPS.md`.

**Fixtures and privacy:** the captured payloads are sanitised (home path, session ids, agent
ids, transcript paths replaced) before being committed, keeping the field names, event order,
and backslash path shape that make them evidence while honouring Agent Guideline #12.

## 069 — Windows support: native only, and what is deliberately left out

**Date:** 2026-08-14
**Status:** Accepted

**Context:** The project is macOS-only (README badge, arm64 DMG, `macos-15` release runner). A
survey of what Windows would take found the signal layer nearly free — Claude Code runs hooks
through Git Bash, so the hook mechanism transfers — and the display layer to be the real work:
the crate did not compile on Windows at all, and 42 `#[cfg(target_os = "macos")]` sites had
only 8 non-macOS fallbacks.

**Decision:** Support **native Windows only** at the "core features" level — floating bar,
tray mode, lights, tooltips, and click-to-focus. Specifically:

- `tauri-nspanel` moves under `[target.'cfg(target_os = "macos")'.dependencies]` and its
  `.plugin()` registration is `cfg`-gated. An NSPanel is macOS by construction (#008); the
  Windows bar is a plain Tauri window — `alwaysOnTop`, `transparent`, `skipTaskbar`,
  `focus: false` — all already in `tauri.conf.json`. Windows has no Spaces problem to solve.
- `pid_alive` gains a non-macOS fallback that answers "alive", matching the fail-open contract
  of `owns_terminal` and `live_workspace_folders`: never prune a light on an answer the
  platform cannot give.
- `install.rs` resolves `HOME` then `USERPROFILE` (a Windows GUI process has no `HOME`), and
  the `chmod` is `#[cfg(unix)]` — Windows has no execute bit and Claude Code invokes the hook
  by path through Git Bash.

**Explicitly out of scope, and why:**

- **Cursor's menu-bar integration** (#038/#045/#047). It reads Cursor's `NSStatusItem` through
  the macOS Accessibility API. Windows Cursor has no menu-bar status item and Windows has no
  AX equivalent. A Windows Cursor click falls back to focusing the window.
- **Per-tab terminal focus** for `cli` sessions (#054/#064). There is no tty concept and
  Windows Terminal exposes no per-tab focus API. Focusing the window is the ceiling.
- **WSL.** Hooks inside WSL write to the WSL filesystem; a Windows GUI app reaching it through
  `\\wsl$\` is meaningful extra scope for a distinctly different user. Native install only.

Rather than ship a light that leads nowhere, these degrade to "focus the app" or are absent —
UI Principle #3 says a click goes to *that* session or nowhere.

## 070 — Windows click-to-focus is a direct Win32 raise, not a port of the macOS scheme

**Date:** 2026-08-14
**Status:** Accepted

**Context:** On macOS a light click does two things at once (#021): an osascript raise, which
is fast (~0.2 s) but cannot see full-screen windows on inactive Spaces, *and* the IDE CLI,
which is slow (~1.1 s, it boots a Node runtime) but is the only thing that crosses Spaces.
Both run because neither alone is sufficient. The exact-tab focus is separate again — the
VS Code extension relay (#015/#019), which the bar cannot do itself.

**What Windows already had:** the relay. `write_focus_request` is platform-neutral and the
extension is plain TypeScript reading `~/.claude/status/focus-request.json`, so per-*tab*
focus needed no work at all. What was missing was getting the right *window* forward: the
whole macOS block was `cfg`-gated out, so a Windows click wrote the relay file and did
nothing else.

**Decision:** `EnumWindows` + `SetForegroundWindow`, matching a window whose title contains
the workspace root's basename **and** ends with the editor's name ("Visual Studio Code" /
"Cursor"). Same matching rule as the macOS raise, which also keys on the project folder in
the title; the trailing app name is what stops a same-named window of another application
from swallowing the click.

Deliberately *not* a port of the two-path scheme:

- **No permission grant.** The macOS raise goes through System Events and needs Accessibility
  trust, which an unsigned rebuild invalidates (#039). Win32 needs none.
- **No subprocess.** It is a direct call, so it is far below the ~1 s an IDE CLI costs. There
  is no reason to fire a slow path in parallel with a fast one when the fast one is complete.
- **The CLI is a fallback, not a partner.** Windows has no Spaces, so the only reason to reach
  for the CLI is that no window matched — the folder is open under a title we did not
  recognise, or is not open at all. Then, and only then, VS Code is invoked.
- **`Code.exe`, not `code`.** The `code` shim is `code.cmd`, which `CreateProcess` cannot
  execute, so it would need `cmd /c` and the nested quoting that comes with a path containing
  spaces. Invoking the executable by absolute path mirrors what macOS already does with the
  CLI binary, and skips the shell entirely.
- **Cursor raises but never falls back.** On macOS its CLI opens a *new* agent instead of
  focusing (#047). That has not been retested on Windows, and spawning a second agent is
  worse than doing nothing, so Cursor stops after the raise (#069).
- **`cli` is unreachable by design** (#069): no tty, no per-tab focus API in Windows Terminal,
  and the recorded pid is still 0 pending measurement — so there is nothing to identify a
  window with. Focusing an arbitrary terminal would be a click that lies about where it went.

**Two supporting changes:**

- `workspace_root` is **un-gated from macOS**. The Claude Code VS Code extension writes the
  same `~/.claude/ide/*.lock` files on Windows, and the raise needs the same cwd → window
  mapping. It returns `cwd` unchanged when the directory is absent, so a machine without
  locks gets the old behaviour rather than a wrong answer.
- Path matching moves into `path_within`, shared by the raise and the live-window pruning
  (#027). On Windows it is **case-insensitive and accepts either separator** — the filesystem
  is case-insensitive and nothing guarantees the lock file and the hook payload spell the
  drive or folder the same way. macOS keeps the exact, `/`-only rule byte for byte, so its
  pruning is unchanged (Agent Guideline #7).

`windows-sys` is pinned to **0.61**, the version Tauri already resolves, so no fourth copy
joins the 0.45/0.59/0.61 already in the graph.

**Verified:** `raises_a_window_by_title` (ignored, run explicitly) starts a real Notepad on a
uniquely-named file and asserts the raise matches it — proving `EnumWindows`, the UTF-16
title decoding, and the predicate path — and asserts it reports *no* match for a title no
window carries, which is what makes the CLI fallback fire when it should. Matched in 0.8 s.
The test needs no editor installed, so it runs on a bare machine. Whether Windows then
*honours* `SetForegroundWindow` is OS focus policy, not this code's decision; the call is
made from a click on our own window, which is the case Windows permits.

**Unverified, and it matters:** the actual window-title formats of VS Code and Cursor on
Windows. **Neither is installed on the test box**, so the `contains(folder) && ends_with(app)`
rule is reasoning from the macOS titles, not observation. It is the one assumption in this
decision that could silently make every click fall through to the CLI fallback (VS Code) or
do nothing (Cursor). Confirm against a real window before shipping — see NEXT_STEPS.

## 071 — A Windows terminal session is focused through its host process, not left dead

**Date:** 2026-08-14
**Status:** Accepted (amends #069, which declared this out of scope)

**Context:** Reported on the first real Windows install: *"clicking a light doesn't take me to
the window associated with it."* The machine had exactly one light, `ide: "cli"` — Claude
Code in a terminal — and decision 069 had explicitly declared `cli` unreachable on Windows,
so `focus_session` did nothing for it. Clicking it was a no-op **by design**, which is
precisely the click-that-goes-nowhere UI Principle #3 forbids.

**069's reasoning does not survive contact with the evidence.** It argued there was "nothing
to identify a window with" — but that was a *consequence* of `pid` being recorded as `0`
(deferred in #068 pending measurement), not a property of the platform. Measuring the process
tree settled it immediately:

```
claude.exe 24292 → powershell.exe 32964 → WindowsTerminal.exe 21168   ("… ◐ Add Windows support to project")
claude.exe 13540 → claude.exe 13408 (Chrome_WidgetWin_1 | "Claude")   ← Claude Desktop
```

Two hops in both cases. The session's own process owns no window, but an ancestor always
does.

**Decision:** Record the owning `claude` pid on Windows, and focus a `cli` or
`claude-desktop` light by walking up from it to the first ancestor that owns a visible titled
window.

- **In the hook:** `parent_pid` walks the process tree to the nearest `claude.exe` ancestor
  rather than reporting its immediate parent. Claude Code runs hooks through Git Bash, so the
  parent is a shell — sometimes two — and those shells exit with the hook, so recording one
  would hand the app a pid that is already dead. This reproduces exactly what `$PPID` means
  on macOS, where bash `exec`s the hook in its own place. Returns 0 when no `claude.exe`
  ancestor is found, because every consumer treats 0 as "unknown" and falls back — an honest
  answer beats a shell pid.
- **In the app:** `focus_host_window` walks the same tree upward and raises the first
  ancestor owning a visible titled window. Claude Desktop takes the same path and keeps its
  title match as a fallback, for a status file written before the pid walk existed.

**What this does not do:** select the *tab*. Windows Terminal exposes no API for that, so
#069's ceiling stands — a click lands on the right window, and the user picks the tab. That
is the same degradation macOS accepts for terminals it cannot script (#054), and it is
enormously better than the nothing that was there before.

**Verified:** `focus_host_window_reaches_a_window` (ignored, run explicitly) reads whichever
live `cli`/`claude-desktop` session has a recorded pid and asserts the walk reaches a window.
Run against this session it resolved `pid=24292` and raised the hosting Windows Terminal. The
hook's half was verified separately: after deploying it, the live status file went from
`pid=0` to `pid=24292`, which `Get-CimInstance` confirms is `claude.exe`.

**Consequence for #064/#067 on Windows:** `pid` is now a real, live handle there, so
pid-liveness pruning and the status reconcile have something to work with. Neither is enabled
on Windows yet — `pid_alive` still answers "alive" unconditionally off macOS — and turning
them on is a separate change that needs its own verification.

## 072 — Tray mode on Windows, and the trap that made it mandatory

**Date:** 2026-08-14
**Status:** Accepted (extends #024 to Windows; completes the option-B scope in #069)

**Context:** Decision 069 listed tray mode in scope for Windows, but the first pass shipped
`set_mode`, `set_tray_image`, `toggle_popover` and the tray builder all gated to macOS. The
settings panel still offered the **Mode** control, so on Windows it was a control that did
nothing. A frontend audit found that this was not merely inert — it was a **trap**:

`hidePopover()` is called whenever a light is clicked in menu-bar mode. On Windows that hid
the window while no tray icon existed to bring it back, and `skipTaskbar: true` means there
is no taskbar button either (verified: the shell's own UI tree lists 21 taskbar entries and
AgentStatus is not among them). The mode is persisted in `localStorage`, so the next launch
came up in menu-bar mode and the very next light click hid it again. **The only recovery was
killing the process.**

**Decision:** Build the tray on Windows rather than hide the control, and make the frontend
incapable of entering the trap even if the tray fails.

- **Tray plumbing un-gated** to `any(target_os = "macos", windows)`. `icon_as_template` and
  `set_icon_as_template` stay macOS-only — template icons are a macOS concept and would draw
  the coloured dots as a monochrome alpha mask.
- **The popover opens away from the edge the tray lives on.** macOS anchors below the menu
  bar at the top; the Windows notification area is at the bottom. Chosen by which half of
  the monitor the click landed in rather than by platform, so a taskbar the user has docked
  to the top works too. The popover is also clamped to the monitor it was clicked on.
- **`set_mode` now returns whether the mode actually engaged** — specifically whether a tray
  exists to represent the panel. The frontend reverts to floating and *persists* that when
  it gets `false`, so a tray that fails to build can no longer strand the app. This is the
  belt to the existing braces (`set_mode` already refused to hide the panel with no tray);
  the frontend needed to know as well, because it hides the panel independently on a light
  click.
- **Windows always uses the single condensed dot.** A Windows notification-area icon *is*
  square — 16x16 logical, scaled by DPI — while the bar's strip is 170x44 for five sessions.
  Stretched into a square that is roughly two pixels per dot: an unreadable smear. The
  Dots/Single control is therefore hidden on Windows rather than offered and ignored, and
  `square_icon` letterboxes whatever arrives onto a transparent square so Windows scales it
  without distortion.
- **A `platform()` command** answers this once at startup. The frontend had *no* platform
  detection at all, which is why every macOS-only no-op looked like success to it.
- **Labels follow the platform:** "Menu bar" becomes "Tray" on Windows.

**Rejected:** hiding the Mode control on Windows, which #069 offered as the cheaper option.
It closes the trap but abandons a feature that was in scope and that the tray API supports
cross-platform. The real work turned out to be the icon *shape*, not the tray.

**Not addressed here, and worth knowing:** in menu-bar mode the frontend skips drag-clamping
and position persistence (correct — a popover is positioned by the tray), and edge-snapping
in floating mode uses full monitor bounds rather than the work area, so the bar can snap
underneath the Windows taskbar. Both are recorded in NEXT_STEPS rather than fixed here.

## 073 — The tray popover dismisses itself, and anchors to the work area

**Date:** 2026-08-15
**Status:** Accepted (fixes two defects in #072, both reported live)

**Context:** Both reported from real use of Windows tray mode, immediately after #072:

1. *"after clicking the lightbar into the tray and clicking that tray version, the regular
   light bar stays on top even after clicking out of the tray"* — the popover never went
   away. Every other tray popover on Windows closes when you click elsewhere; this one had
   to be dismissed from the tray icon.
2. *"clicking from the tray opens the menu underneath the tray, which obscures the lightbar
   settings"* — the popover opened overlapping the taskbar.

**Cause of (2):** `toggle_popover` anchored to the **click point**, offsetting by 8px. But
the tray icon lives *inside* the taskbar, so anchoring to the click puts the popover on top
of it. `Monitor::size()` is no help — it is the full screen and knows nothing about the
taskbar. Fixed by anchoring to the monitor's **work area** (`GetMonitorInfoW`'s `rcWork`,
the screen minus the taskbar and any other appbars), against whichever edge the tray is on.
Verified: work area ends at y=1392 on this machine and the popover now lands at y=1353 with
its bottom edge at 1384 — 8px clear of the taskbar.

**Cause of (1), and the part that was not obvious:** the fix is to hide on `Focused(false)`,
but the window is configured `focus: false` and `show()` does not activate it — so the
popover **never held focus and therefore never lost any**. The first attempt at this
compiled, deployed, and did exactly nothing; the event never fired. `set_focus()` after
`show()` is what makes the dismissal possible at all. Windows only: the macOS panel is
deliberately non-activating (#008) and focusing it would defeat that, so macOS behaviour is
unchanged.

**The race this creates, and the guard for it:** clicking the tray icon while the popover is
open delivers the focus loss *first*. Without a guard the popover would hide on blur and the
tray click would immediately reopen it, so the icon could never close it — the bug would look
identical to the one being fixed. A click landing within 400 ms of an auto-hide is treated as
the closing click and consumed.

**Third defect, found while fixing these:** opening the settings panel grows the window from
~31px to ~390px tall. Anchored just above the taskbar, that growth runs off the bottom of the
screen and takes the settings with it. The existing `chooseGrowthDirection` already flips the
panel to grow *upward* when the lights are in the bottom half, which does most of the work;
a new `fit_popover` command pulls the window back inside the work area afterwards, since the
frontend cannot see where the taskbar is. Verified: settings open at y=1023, bottom edge 1384
— still clear.

**Verified by driving the real UI**, since none of this is reachable from a unit test: switch
to tray mode, click the tray icon with a real mouse event (UIA's `Invoke()` does *not* move
the cursor, so the tray event carries a nonsense position and the popover anchors to the
wrong place — a test artifact that briefly looked like a product bug), confirm placement,
click elsewhere and confirm dismissal, reopen, open settings, confirm it stays on screen.

## 074 — Opening the tray popover on Windows opens the settings panel with it

**Date:** 2026-08-15
**Status:** Accepted (refines #072 for Windows; macOS keeps #024's behaviour)

**Context:** Requested from live use: *"opening the popup from the tray should by default
open the settings."*

The reasoning holds up. On Windows the tray item is a **single summary dot** (#072 — a
notification-area icon is square, so a row of dots would smear), which means the popover was
showing the user a second, larger copy of information they had just clicked on, and the panel
they actually wanted was another right-click away. On macOS the menu-bar item already shows
every dot at full fidelity, so the trade is different there.

**Decision:** On Windows, revealing the popover from the tray opens the settings panel with
it. macOS is unchanged — its popover still opens as the bare light bar (#024). Gated on the
frontend by the `platform()` answer added in #072, not by guessing.

**The mechanism, because the obvious one does not work:** the frontend needs to know the
popover was revealed. `visibilitychange` looks like the natural signal and the codebase
already used it for the same moment — but **WebView2 keeps the document "visible" while the
window is hidden**, so on Windows it never fires. The first implementation hung the behaviour
off it, built, deployed, and did nothing at all; the popover came up as the bare 45x31 bar.
The backend now emits a `popover-shown` event from `toggle_popover`, which the frontend
listens for; `visibilitychange` is kept as the macOS path. Emitting on both platforms costs
nothing since the frontend decides what to do with it.

Opening the panel this way goes through the ordinary `toggleSettings`, so it inherits
`chooseGrowthDirection` (grows upward from a bottom-docked tray), the lights anchor, and
`fit_popover` (#073) — the popover comes up 189x361 with its bottom edge 8px clear of the
taskbar rather than running off the screen.

**Verified live:** clicking the tray icon with a real mouse event now yields a 189x361
popover with the panel open, inside the work area; clicking elsewhere still dismisses it.

## 075 — The settings panel collapses without the lights jumping

**Date:** 2026-08-15
**Status:** Accepted (fixes a flicker in #024's panel, surfaced on Windows; also refines #073)

**Context:** Reported live: *"the lightbar flickers every time I change from the settings menu
to the compressed version (it only happens when compressing the settings, not the other way
around)."* The asymmetry was the clue.

**Cause.** `#bar.panel-above` is `flex-direction: column-reverse`, so with the panel open the
lights sit at the **bottom** of a ~390px window. Collapsing mutates the DOM first: the panel
goes, the lights snap to the **top** of a window that is still ~390px tall, and only then do
`resizeToContent` and `anchorLightsTo` — two async IPC round trips, three-plus frames later —
shrink the window and move it back down. The user sees the lights leap up and return.
Opening never shows it because `chooseGrowthDirection` runs *before* the panel appears, so
the lights never move.

There is no way to make the DOM change and the window geometry change atomic: the window
calls are async IPC and the DOM change is synchronous, so some frames will always disagree.

**Decision:** Suppress the paint for those frames rather than chase atomicity. `#bar` is set
`visibility: hidden` immediately before the DOM mutation and restored once the window is its
final size and back in position, so the first frame the user sees is the finished one.
`visibility` specifically — not `display` or `opacity` — because it stops painting while
keeping the layout `resizeToContent` has to measure.

Two robustness details, both needed to avoid trading a flicker for something worse:

- The restore is in a `finally`, and `resizeToContent`'s double-`requestAnimationFrame` wait
  is now **bounded by a 250 ms timeout**. Animation frames stop firing when a window is
  hidden, so a popover dismissed mid-collapse would otherwise leave that promise pending
  forever — and a bar left `visibility: hidden` comes back **invisible** on the next open.
- #073's close-debounce now only stamps when the popover is actually **visible**. A hidden
  window still reports focus changes — opening the tray's own hidden-icons flyout produces
  one — and stamping then made the *next* tray click look like a close and get swallowed, so
  the icon did nothing. Found while testing this.

**Verified, with a negative control.** A harness parks the bar low on the monitor (forcing
`panel-above`), opens the panel, then captures 14 frames at 25 ms across the whole panel area
during the collapse and reports where the green running-light painted in each:

| | light Y values | spread |
|---|---|---|
| fix disabled (negative control) | 22, 382 | **360 px — jumped** |
| fix enabled | 76–98 | 22 px — no jump (dot glow / hover scale) |

The negative control matters: without it, "spread = 0" only proves the measurement ran, not
that it could ever fail.

---

## 076 — macOS 13 becomes the floor, and the DMG becomes universal

**Date:** 2026-08-15
**Status:** Proposed — the code is written; **no part of it has run on macOS**
(extends #068 to macOS, closes the macOS half of #059, amends #024 and #041)

**Context:** The README promised "macOS on Apple Silicon" with no version floor, and the
project talked about itself as a macOS-15+ tool. Neither was a real constraint. Reading the
build settled what actually gated it:

- **The bundle already declared 10.13.** `tauri.conf.json` sets no `minimumSystemVersion`,
  so the Tauri default (`tauri-utils-2.9.3/src/config.rs:691`) is what lands in `Info.plist`.
  The app would launch back to High Sierra.
- **Nothing in the code is version-gated.** No `sw_vers`, no availability checks, no version
  conditionals. Everything the app shells out to (`osascript`, `pgrep`, `ps`, `open`,
  `/usr/bin/sqlite3`) and every native API it uses (Accessibility, `NSPanel`, `NSStatusItem`)
  predates Ventura by years.
- **The CSS is fine.** `oklch()` needs Safari 15.4 and `color-mix()` needs 16.2; Ventura runs
  16.4+. The light glows survive. They would break below macOS 12.
- **`jq` was the whole floor.** `report.sh` calls it nine times, `install.rs` never checked
  for it, and it ships at `/usr/bin/jq` only from macOS 15. A DMG user on 13 or 14 got an app
  that installed a hook, registered it, and then no-opped on every event forever — no light,
  no error. #059 found this; #068 fixed it for Windows and deliberately stopped there.

**Where the floor should sit:** Claude Code's own requirement is **macOS 13.0+**
([setup docs](https://code.claude.com/docs/en/setup)), so supporting 11 or 12 buys nothing
for the primary host — a Mac that cannot run Claude Code has no sessions to light. The same
page lists `darwin-x64` as supported, which makes Intel Macs a real population that the
arm64-only DMG excluded outright.

**Options for the hook:**

| Option | Verdict |
|---|---|
| Guard `report.sh` on `jq` and tell the user to `brew install jq` | No — #068 already rejected this: it turns a silent failure into a loud one and still leaves a non-working app, and it makes Homebrew a requirement |
| Bundle a `jq` binary in the DMG | No — ships a third-party binary to avoid using code we already wrote and test |
| Install the native `agentstatus-hook` on macOS too | **Chosen.** The port exists, has a golden-file parity test against `report.sh`, and has shipped on Windows since 0.7.0 |

**Decision:**

1. **macOS installs `agentstatus-hook`**, not `report.sh`. `install.rs`'s `#[cfg(not(windows))]`
   branch copies the bundled binary instead of writing the embedded script, and
   `tauri.macos.conf.json` declares it as a bundle resource the way
   `tauri.windows.conf.json` already does. `hooks/setup.mjs` (the dev path) follows.
2. **`minimumSystemVersion: "13.0"`**, declared rather than defaulted. An unsupported Mac is
   now refused by Gatekeeper at install time instead of installing an app whose host cannot
   run there anyway. It also sets `MACOSX_DEPLOYMENT_TARGET` for the Rust build.
3. **The DMG is universal.** `release.yml` builds `--target universal-apple-darwin`.
4. **The hook is universal too.** This is the non-obvious consequence: `report.sh` was a
   shell script and ran on any architecture, while a compiled hook does not. An arm64-only
   hook inside a universal app would give every Intel user exactly the silently-dead app this
   decision exists to remove. `stage-hook.mjs` builds both slices and `lipo`s them when
   `AGENTSTATUS_HOOK_UNIVERSAL=1` (set only by the release workflow), then **asserts both
   architectures are present** before staging — the same shape as the PE-subsystem assertion
   that already guards the Windows build. Local builds stay host-only, so no developer needs
   a second rustup target to run `tauri dev`.
5. **`install.sh` drops its `jq` prerequisite.** `jq` is now only a dev dependency of
   `hooks/gen-golden.sh`, which that script does not run.

**Two macOS-only hazards a straight port from Windows would have shipped.** Neither exists on
Windows and neither is visible from reading the Windows code:

- **`fs::copy` carries extended attributes on macOS.** It is `fcopyfile(COPYFILE_ALL)`, which
  includes `COPYFILE_XATTR`. If the resource inside a downloaded bundle still carries
  `com.apple.quarantine` — which "Open Anyway" does not necessarily clear on nested files the
  way `xattr -dr` does — the installed hook would inherit it, and that is a quarantined
  unsigned executable handed to Claude Code to run on every tool call (Agent Guideline #3).
  The install now writes the bytes to a fresh file, which has no xattrs to inherit.
- **`ETXTBSY`.** Writing over the destination fails while a hook process is executing it, and
  hooks fire on every tool call. The new bytes go to a staged path beside it and `rename`
  over the top: atomic, and a hook mid-run keeps the inode it already opened. Windows needed
  a `.old-*` rename-aside and a sweep for the same problem; unix drops the unlinked file
  itself, so that machinery stays Windows-only.

**What is verified and what is not.** `cargo check` passes on the Windows host, which proves
the Windows path is unregressed but exercises none of the new code. **Everything macOS in
this entry is unverified** — it was written on a Windows machine, which is the same reason
#068 stopped short of macOS. Before release it needs, on a Mac: `cargo check`,
`hooks/gen-golden.sh` re-run to confirm the goldens hold there, `cargo test` in the hook
crate, a real DMG build, and one live session diffed old-hook-vs-new. The DMG's exact
filename (`AgentStatus_0.7.1_universal.dmg` in the README) is an expectation, not an
observation — confirm it against the first universal build.

**Left alone deliberately:** an upgrade does not delete the orphaned `~/.claude/status/report.sh`.
Its registration is replaced (`HOOK_MARKERS` matches both names), so nothing invokes it; the
Windows upgrade path leaves the same file, and removing files from a user's `~/.claude` for
tidiness is not worth the blast radius.

## 077 — Several Windows terminal windows are told apart by the session title

**Date:** 2026-08-15
**Status:** Accepted (fixes #071)

**Context:** Reported live: *"when multiple terminals are open with claude, clicking a light
only brings a terminal into focus but does not focus the right window."* Decision 071 focuses
a `cli` light by walking up from the recorded `claude` pid to the first ancestor owning a
visible titled window. The measurement behind it had **one** terminal open, and it hid the
assumption that a host process owns one window. Three Claude sessions in three terminals on
this machine:

```
claude.exe 34064 → powershell.exe 24256 ┐
claude.exe 36412 → powershell.exe  5480 ├→ WindowsTerminal.exe 30784   (all three)
claude.exe 31588 → powershell.exe 33092 ┘
```

Windows Terminal runs every window of an instance in **one** process, so all three chains
converge on pid 30784, which owns all three windows. `focus_host_window` kept the first window
`EnumWindows` handed it (`owned.entry(owner).or_insert(hwnd)`) — that is z-order, so the click
raised whichever terminal was already nearest the top. Two of three clicks landed on the wrong
session, and the "right" one was luck.

**Decision:** When the host process owns more than one window, choose between them by the
session title, reusing the macOS Ghostty rule (#055/#066) unchanged. Claude Code keeps the
terminal's title bar set to its own session title behind an activity glyph, and the app
already reads that title from the transcript's `ai-title` record for exactly this purpose. The
live windows and titles join cleanly:

```
◐ Fix Windows orange input detection            ← ai-title "Fix Windows orange input detection"
◑ Fix Claude terminal window focus on light click
✳ Extend app support for older macOS versions
```

So: a title that **ends with** the session title is that session's window (strong); one that
merely **contains** it may be showing something else spanning it, so that grade must be
unique to act on. A single window is still taken as-is, titled or not — no ambiguity, and no
transcript read on the click path.

**When it cannot tell, the click does nothing.** Two cases: a session Claude has not titled
yet, and a session in a *background tab*, whose window shows the foreground tab's title.
Raising an arbitrary window there is precisely the reported bug, and a wrong window is worse
than none (UI Principle #4). This is a deliberate behaviour change from #071, which always
raised something.

**What this does not do:** select the tab. #069's ceiling stands — Windows Terminal still
exposes no way to select one.

**Verified live**, on the three-window machine above:

- `focus_host_window` is now `host_window(...).map(raise)`, so the *choice* can be asserted
  without `SetForegroundWindow` — which is refused for a process that is not the foreground
  one, so a test binary can never make it succeed (the first run of the live test failed on
  exactly that, with the correct window chosen underneath). `focus_host_window_reaches_a_window`
  (ignored, run explicitly) now asserts the chosen window's title contains the session's:
  session `7512a93d`, pid 34064, resolved to `✳ Fix Windows orange input detection` — its own
  window, not the topmost one.
- `pick_window_matches_the_session_title` covers the rule itself against those live titles:
  strong match, weak-unique match, ambiguous weak declined, no title declined, single window
  taken.

---

## 078 — A plan-mode approval turns the light orange

**Date:** 2026-08-15
**Status:** Accepted
**Context:** Reported live: "the Windows version isn't picking up on orange (user input
needed) things right now." Asked what the light showed instead, the answer was **green**.

### What was measured

The whole chain was instrumented rather than reasoned about: a 150 ms poller over
`~/.claude/status/sessions/<id>.json` and Claude Code's own `~/.claude/sessions/<pid>.json`,
plus timed full-screen captures of the bar. Claude Code 2.1.229, Windows, `ide:"cli"`.

**The Windows port is not at fault**, and this is worth recording so nobody re-investigates
it. Three prompt types already worked end to end:

| prompt | when `PermissionRequest` fires | light | held | Claude's own `status` |
|---|---|---|---|---|
| `AskUserQuestion` | when the prompt appears | `blocked` | 43 s | `waiting` |
| Bash permission (1st) | 62 ms after `PreToolUse` | `blocked` | 32 s | `waiting` |
| Bash permission (2nd) | same | `blocked` | 6 s | `waiting` |

A capture taken 8 s into the first case shows the bar drawing green + orange. Registration is
correct too (11 events, `PermissionRequest` among them) and the event name is present in the
2.1.229 binary.

**`ExitPlanMode` is the defect.** Its `PermissionRequest` fires at *resolution* time:

```
01:19:07.803  running   "Running ExitPlanMode"              <- PreToolUse; prompt appears
   … 48 seconds of the approval sitting on screen, light GREEN …
01:19:55.122  running   "⏸ waiting — approve ExitPlanMode"  <- PermissionRequest, on approval;
                                                               PostToolUse overwrites it in
                                                               under 150 ms
```

So the orange existed for a fraction of a second, *after* the user had already dealt with it.

### Why the obvious fix was dropped

The first plan was to extend #067's reconcile: Claude Code's own record reads `status:
"waiting"` while a session is stopped for the user, it is already parsed every poll, and
#067's measured vocabulary always said `waiting` → orange while only the `idle` → grey half
was implemented. It was approved, then **falsified by the reproduction above**: during the
plan prompt Claude Code's status stayed `busy` the whole time. It would not have caught the
reported case, and it added no coverage for any case that was measured, so it was dropped
rather than shipped as well. Recorded because the reasoning is attractive and someone will
propose it again.

### Decision

`ExitPlanMode` and `EnterPlanMode` exist solely to stop and ask the user, so their
`PreToolUse` **is** the prompt. Both hook implementations rewrite that event to
`PermissionRequest` before the state map, which yields exactly the state and the wording the
late event would have written; `PostToolUse` returns the light to green on approval.

Rewriting the event rather than adding a state branch is what keeps `state` and `detail` in
agreement — one substitution, and the existing mapping does the rest.

`AskUserQuestion` was considered and **left out**: it already blocks through the real event,
so adding it would be redundant on what was measured. `EnterPlanMode` was included at the
user's explicit choice even though it did **not** prompt in either observation; the cost when
a tool in the set is auto-approved is a sub-second orange flicker, accepted deliberately as
far cheaper than 48 s of wrong-green on a session that is waiting (UI Principle #2).

- `hooks/agentstatus-hook/src/main.rs` — `WAITS_ON_USER` plus the rewrite in `decide`.
- `hooks/report.sh` — the same rewrite in jq as `$ev`, every downstream `$event` switched to
  it, so macOS keeps parity for as long as it ships the shell hook (see #076).
- Four fixtures added to `synthetic.jsonl` (both tools, each followed by its `PostToolUse`)
  and `hooks/gen-golden.sh` re-run. The regenerated golden file is **+4 lines, 0 changed** —
  that diff is the proof the `$event`→`$ev` refactor altered nothing else.

No status-file schema change, no new hook registration, no app-side change.

### Verified

- `cargo test` — 14 passing, including `matches_report_sh_on_every_fixture` (strict equality
  against `report.sh` across all 46 fixtures) and a new
  `plan_mode_tools_block_from_pre_tool_use` that also pins the negative case: `Bash`
  `PreToolUse` is still `running` with `$ ls`.
- Live, against the rebuilt binary installed over `~/.claude/status/agentstatus-hook.exe`:
  a real plan approval held `blocked` **31.9 s**, for the entire time it was on screen, and
  returned to `running` on approval. The identical prompt held green for 48 s before the fix,
  which is the before/after pair rather than a single after-the-fact reading.
- A screen capture during that window shows the session's light **orange** on the bar.

### Not fixed, and deliberately recorded

Auto-mode escalation, file-edit diff approvals, and a subagent's own prompt were never
reproduced — `permissions.defaultMode: "auto"` allowed every command tried, and a project
`permissions.ask` rule added mid-session had no effect. Whether those announce themselves on
time is **unmeasured** (Guideline #4). If a green light on a waiting session is reported
again, that is where to look next.

---

## 079 — The release publishes with `find`, not a `dist/*` glob

**Date:** 2026-08-15
**Status:** Accepted
**Context:** The first `v0.8.0` tag built both platforms successfully and then **published
nothing**. The `publish` job failed in 9 s with:

```
Post "https://uploads.github.com/.../releases/370970632/assets?label=&name=msi":
  read dist/msi: is a directory
```

### The mechanism

`actions/upload-artifact` preserves the directory structure **below the common ancestor of
the paths it is given**. The two build jobs give it different numbers of paths, so the two
artifacts do not arrive in the same shape:

| job | paths given | common ancestor | arrives as |
|---|---|---|---|
| macOS | one (`…/bundle/dmg/*.dmg`) | the dmg dir | `dist/AgentStatus_0.8.0_universal.dmg` |
| Windows | two (`…/msi/*.msi`, `…/nsis/*-setup.exe`) | `…/bundle/` | `dist/msi/…`, `dist/nsis/…` |

`gh release create "$TAG" dist/*` therefore handed `gh` two files and **two directories**.
This was invisible until now because #069 wrote the matrix and the globs were checked against
the *local* build output, where both platforms' bundles sit in their own directories — the
asymmetry is created by `upload-artifact`, not by the build.

**Nothing broken reached users.** `gh release create` deleted the release it had just made
when an asset upload failed, so `v0.7.1` remained Latest. That is `gh`'s behaviour, not a
guarantee this workflow arranged, and it is the only reason a half-release did not survive.

### Decision

Collect the installers with `find dist -type f` rather than a glob, so the publish step is
immune to whichever shape either job produces, and assert the count:

```bash
mapfile -t installers < <(find dist -type f | sort)
printf 'publishing %s\n' "${installers[@]}"
[ ${#installers[@]} -ge 3 ] || { echo "::error::expected 3 installers, found ${#installers[@]}"; exit 1; }
gh release create "$GITHUB_REF_NAME" "${installers[@]}" …
```

The count guard is the part worth keeping: the failure above was loud, but a glob that
silently matched *fewer* files would have published a release missing an installer, which is
the same class of defect as #041's version guard — fail before publishing, not after.

Flattening the Windows upload instead was rejected: it fixes this shape and leaves the next
one to be discovered by another failed tag.

### Verified

Simulated `dist/` reproduced from the real CI listing (a flat DMG plus `msi/` and `nsis/`
subdirectories); the extracted publish script collects exactly the three files and passes the
guard. The script was also `bash -n`'d out of the YAML rather than eyeballed.


---

## 080 — A light can be closed by hand

**Date:** 2026-08-18
**Status:** Accepted
**Context:** Every way a light could disappear was a *prune the bar decided on*: the IDE
window is gone (#027), the owning pid is dead (#054), Cursor archived the composer (#048),
a background agent retired (#065), or the 2 h `MAX_IDLE_SECS` backstop finally fired (#004).
Each waits on evidence the bar can observe. The user often has evidence the bar cannot —
"that terminal is finished, I'm done with it" — and had no way to say so. The only lever was
Reload/Quit, which drops every light, or waiting out a timer measured in hours.

### Decision

The settings panel (right-click) now reveals a second row **alongside the lights** — one
small red × per light, aligned one-for-one with the light above it (beside it, when the bar
is vertical). Clicking one calls a new `dismiss_session` command, which deletes that
session's `sessions/<id>.json` and its `<id>.subagents` directory — byte for byte what the
automatic prunes already do, so there is one deletion path, not two notions of "gone".

### Which side the buttons appear on

The row grows toward the middle of the screen, reusing `chooseGrowthDirection` — the rule
that already decides which way the settings panel opens (#075). The lights stay pinned
where they are and the strip grows around them, so a bar resting on the bottom edge of the
screen puts its buttons *above* the lights rather than pushing them off the screen (which
is exactly what a first cut, always-below, did). A horizontal bar takes the above/below
answer and a vertical one the left/right answer, so `panel-above` alone cannot serve both:
a second class, `panel-left`, carries the horizontal half.

### What a dismissal means

**A deletion, not a hide.** If the session is genuinely alive, its next hook event rewrites
the status file and the light comes back. That is the honest answer, not a shortcoming:
UI Principle #4 forbids showing a light for a session that is over, and it forbids the
mirror case just as firmly — withholding a light from a session that is running. The
tooltip says so ("drops this light now; it returns if the session is still active") rather
than leaving the user to discover it.

**Tombstoned for the gap.** `list_sessions` is polled once a second, so a poll issued before
the click can return after it and paint the light back for a tick. The frontend records the
dismissed id and filters it out of `visibleSessions()` until the poll agrees the session is
gone — and unconditionally after `DISMISS_GRACE_MS` (5 s), so a delete that failed (a status
file the app cannot remove) surfaces the light again instead of hiding it forever. The
grace cap is the part that matters: without it, one failed `remove_file` would be
indistinguishable from a successful prune.

### Options considered

| option | why not |
|---|---|
| Close buttons always on the bar | The bar is a click target: a light *is* the button that opens the session (UI Principle #3). A permanent × a few pixels away turns a misclick into a deleted light, and the glanceable strip grows a second row of chrome competing with the one signal that matters (UI Principle #1). |
| An × drawn on top of each light | Same misclick hazard on the same pixels, and it hides the state color — the thing being looked at — behind the control. |
| Hide locally, don't delete the file | The light would stay hidden while the session kept reporting, and the next prune would delete the file anyway. Two mechanisms for one outcome, and a bar that lies about a running session. |
| A "clear all" button in the panel | Coarser than the complaint: the user wants *that* light gone, not the four they are still using. |

### Notes

- `dismiss_session` validates the id (ASCII alphanumeric, `-`, `_`) before joining it onto a
  path. It is the first command that takes a caller-supplied string and deletes a file with
  it; the check keeps it a session id rather than a traversal.
- The buttons use a fixed red (`oklch(60% 0.19 25)`), deliberately **not** `var(--c-error)`.
  They are controls, not lights: a user who recolors the error state must not end up with
  blue close buttons, and a row of red circles must never read as five sessions in error.
- The closers row is built on every render whether or not it is shown, and only its `hidden`
  attribute is toggled with the panel — so opening the panel reveals a row that is already
  correct rather than one built on reveal.

### Verified

- `dismiss_deletes_one_session` (ignored, sets `AGENTSTATUS_DIR`) — the command deletes the
  named session's status file and its `.subagents` directory, leaves the neighbouring
  session untouched, answers `false` for an id that never existed, and refuses `../keep`
  and the empty string without touching the filesystem.
- `app/tests/dismiss-light.mjs` — evals the shipped `visibleSessions`/`reapDismissed` out of
  `main.js`: hidden on click, tombstone lifted once the poll agrees, lifted after the grace
  window when the delete never took, still hidden inside it, and composed with the Unknown
  filter rather than replacing it.
- Layout measured in headless Chrome against the real `index.html`/`styles.css`/`main.js`
  (Tauri stubbed): with five lights, every × lands on exactly its light's x (13/36/59/82/105
  horizontal) and on exactly its light's y when vertical, at both ends of the size slider
  (8 px and 24 px). Clicking one removed the light *and* its button and invoked
  `dismiss_session` with that session's id, with no flicker back on the following poll.
- Growth direction measured the same way, with the stub reporting a window position in each
  quadrant of a 1440×900 monitor: horizontal bar → buttons below at the top of the screen,
  above at the bottom; vertical bar → buttons right on the left of the screen, left on the
  right; and each × still on its own light's axis in every case.

## 081 — The bar stops reading Cursor's menu while it has nothing to decide

**Date:** 2026-08-18
**Status:** Accepted (fixes a regression #052 introduced against #038's mitigation)

**Symptom (reported live).** "I can't click into the Cursor menu bar item because it keeps
clicking off of it" — Cursor's own status-item menu closes itself a beat after it opens.

**Root cause.** Two code paths read Cursor's status item through the Accessibility API, and
either one cancels that menu if it lands while the user has it open:

| Reader | Depth | Cadence |
|---|---|---|
| `cursor_attention_count` (#038, the pip) | the item's `AXTitle` | every 20 s, from the frontend |
| `cursor_tray_titles` (#052, the `", Running"` veto) | the item's whole `NSMenu`, row by row | every 5 s (`CURSOR_FACTS_TTL`), from `list_sessions` |

#038 already knew the read interferes — it chose 20 s *because* "the AX read can dismiss
Cursor's own menu-bar popover if it lands while the user has it open (observed live)". #052
then added a deeper walk on a 5 s cache and never inherited that constraint.

Worse, it ran when it had nothing to decide. The veto is the last condition of the guard that
greys a silent Cursor light:

    f.terminal && stale && !subs_live && state != "error" && !tray_says_running(…)

On a Cursor light that has **already** settled to `idle`, the first four stay true forever —
`terminal` is Cursor's flushed status, `stale` only grows — so every poll evaluated the guard,
the 5 s cache expired, and the walk ran again, for the whole life of the light. Its answer
could only ever set `state = "idle"` on a light already idle. Measured live before the fix:
two idle Cursor sessions on the bar, both `terminal=true`, ages 356 s and 127 s — i.e. a walk
into Cursor's menu every 5 s, indefinitely, for no effect.

**Decision.** Move `state != "idle"` into the guard **ahead of** the tray read, so the AX walk
only happens on a poll that could actually change a light. Rust's `&&` short-circuits, so an
already-idle light never reaches `cursor_tray_titles_cached`.

Behaviour is identical by construction (Guideline #7): the arm the new condition gates assigns
`idle`, which is the value such a light already holds. Nothing about the veto, the reconcile,
the schema, or the hooks changes — only when the app is willing to touch another app's menu.

`cursor_tray_titles` now also emits a marker-gated `cdbg` line (`tray_walk rows=N`), the same
switch #038 added for the pip, so the one thing the app does that can interfere with Cursor's
menu is traceable without a rebuild.

**Options considered.**

| Option | Verdict |
|---|---|
| Gate the walk on a light that can change (chosen) | Removes the walk entirely from the steady state, keeps #052's veto intact for the case it was written for, four words of code |
| Raise `CURSOR_FACTS_TTL` for the tray only | Makes a pointless read rarer instead of removing it; still fires forever, still lands on an open menu eventually |
| Skip AX reads while the pointer is in the menu-bar strip | Targets the *remaining* 20 s pip read too, but needs new AppKit calls and a cooldown heuristic. Held back until the user confirms a residual — this fix removes ~4 of every 5 reads |
| Drop the tray veto | Reinstates #052's bug: a live Cursor agent whose hooks go quiet mid-turn gets greyed (UI Principle #4) |

**Verified** (marker on, against the installed build):

- 45 s with two idle Cursor lights on the bar: **zero** `tray_walk` lines; only the pip's
  `trusted=true count=…` at 20 s intervals. Before the fix the same state produced a walk
  every 5 s.
- **Positive control** — one Cursor session's state flipped to `running` while stale: exactly
  one `tray_walk rows=19`, immediately followed by the reconcile settling the light back to
  idle and no further walks. So the absence above is the guard, not a broken log line, and the
  veto still runs where #052 needs it.

---

## 083 — The pip's click clears Cursor's notifications itself

**Date:** 2026-08-18
**Status:** Accepted (replaces #045's press-clears-it premise, falsified on Cursor 3.15.6)

**Symptom (reported live).** "Clicking the Cursor composer pip in the lightbar does not close
out the notification in the Cursor menu bar, so the pip just keeps coming back."

**Root cause.** #045 verified on Cursor **3.12.10** that pressing a notified tray row
(`"• <name>"`) both opened that composer *and* marked it read — count `" 2"` → `" 1"`, bullet
gone. On **3.15.6** only the first half is still true. Two independent findings, one measured
and one read out of the shipped bundle:

| Probe (live, 3.15.6, AX press exactly as the app does it) | Result |
|---|---|
| Press the first `"• <name>"` row | `AXPress` → `kAXErrorSuccess`, count stays **2**, both bullets remain |
| Same, then `activate_cursor()` (the full #045/#046 click path) | count stays **2** |
| Press the `"Clear All Notifications"` row | count **2 → 0** |

So the press mechanism is intact — it is the composer row that no longer clears. Cursor's own
code says why. In `TrayMainService.createContextMenu` a composer row's click is
`sendMessageToWindow("vscode:openComposer", {composerId}, window)`; the renderer's handler for
that message runs `glass.openAgentById` / `composer.openComposer` and nothing else. Only
`"Clear All Notifications"` sends `vscode:clearAllNotifications`, whose handler calls
`markAgentRead` for every unread agent **and** `clearAllBadges`.

A row's bullet is `hasUnreadMessages || badgeCount > 0` — two sources, and opening the composer
clears neither from that path. The badge half is worse in **Glass** mode (this user's Cursor:
`glassMode = true`), where the per-composer listener that clears a badge when its window is
focused is never registered at all:

    this.environmentService.isGlass || this.setupFocusListener()

leaving only `clearAllBadges` and a **1 h** auto-clear timer (`scheduleAutoClear`, `36e5`). That
is exactly the observed "it keeps coming back": nothing the bar could press, and nothing the
user could do short of Clear All, retires that bullet within the hour.

**Decision.** The pip's click presses **two** rows: first the top `"• <name>"` composer row (as
before — it still opens that composer, confirmed by the user), then `"Clear All
Notifications"`. The frontend zeroes `cursorAttention` optimistically instead of decrementing
by one, then re-reads at `CURSOR_RECHECK_MS` as before.

Cursor exposes **no per-composer clear** to the outside — not through the tray, not through an
agent deeplink (`cursor://…/agent` routes to the same `glass.openAgentById`) — so clearing one
composer's notification necessarily clears the rest. That cost is accepted deliberately: on
Glass those other bullets would otherwise sit for up to an hour whatever the user does, and the
sessions behind them keep their own hook-driven lights on the bar, which is where AgentStatus
tells the user about them anyway.

**Options considered.**

| Option | Verdict |
|---|---|
| Open the top composer, then Clear All (chosen) | One click both navigates and empties the pip; two lines of Rust; loses the other composers' bullets, which Cursor was going to sit on for an hour regardless |
| Open the top composer; alt-click the pip to Clear All | Keeps Cursor's queue for composers not yet seen, but a plain click still leaves the pip up — the reported bug survives as the default gesture, and right-click is already the close-× row (#080) |
| Pip click = Clear All + activate Cursor only | Always clears and is the simplest, but the pip stops leading to a specific composer (UI Principle #3) |
| Leave it; let the 1 h timer clear it | The pip lies for an hour about work already dealt with (UI Principle #4) |
| Write Cursor's `state.vscdb` to mark a composer read | Editing another app's live database behind its back; corrupts on a schema change and races Cursor's own writes |

**Verified.** The three AX probes in the table above, all against Cursor 3.15.6 with the
Accessibility grant the app already holds, plus the bundle read of `createContextMenu`, the
`vscode:openComposer` / `vscode:clearAllNotifications` handlers, and the badge lifecycle
(`hasComposerNotification`, `setupFocusListener`, `scheduleAutoClear`). `cargo test --release`
and all four `app/tests/*.mjs` pass.

**Not yet verified:** the two-press sequence end to end against a *live* notification — Cursor
quit before a new bullet appeared, and a bullet only arrives when an agent finishes a turn
while Cursor is in the background. What is unproven is narrow (that the second walk still finds
its row a beat after the first press); each press is proven on its own. Re-run
`cursor_press_next_attention` with a real notification pending before this ships.

**Left alone (deliberately).** A *session* light's click (#047) presses that composer's row and
nothing else, so it still leaves the bullet up. That is the same defect, but it was not what
was reported and the fix is heavier there — a session click would clear notifications for
composers the user never mentioned. Recorded in `NEXT_STEPS.md` instead.

## 082 — Settings become a window; the bar keeps only lights and a right-click controls row

**Date:** 2026-08-18
**Status:** Accepted (supersedes #015's in-bar panel; amends #024, #074, #075, #080)

**Context.** Reported live: "lets also move the settings off the bar: its starting to be a
bit much", and separately "add the option to not show the cursor menu bar pip". The panel
had grown from #015's one orientation toggle to sixteen controls — mode, condense,
orientation, three sliders, five color wells, sort, unknown, audio plus three chimes and a
volume, three footer links — all rendered *inside* the always-on-top strip, which grew to
~360 px tall when opened. Everything about that fights UI Principle #1: the bar exists to be
read in under a second, and it had become a settings surface that also shows lights.

**Decision.**

1. **A settings window.** `open_settings` builds a plain decorated window (560×430,
   non-resizable, `settings.html` / `settings.css` / `settings.js`) with a sidebar —
   General, Lights, Colors, Audio, About — and a footer of Reload / Reset to defaults /
   Quit. It follows the system light/dark appearance rather than the bar's frosted dark,
   because it is a document window sitting next to other document windows, not part of the
   overlay. Deliberately **not** the main window's NSPanel (#008): that panel exists to
   float without taking focus, and settings wants the opposite — normal stacking, keyboard
   focus, a title bar to close. Built lazily; an always-on bar should not carry a second
   webview it may never show. It needs its own capability file, since Tauri scopes
   capabilities by window label and the existing one names only `main`.

2. **Right-click reveals the bar's controls, and nothing else.** First cut had right-click
   open the window directly; the user's next message — "instead of opening the settings
   immediately when right clicking, lets just show the close buttons and a small settings
   icon" — replaced it. So a right-click toggles the controls row: #080's one × per light,
   plus a **gear** one slot past the last of them. The gear's box is exactly one light wide
   so the × buttons keep their one-for-one alignment with the lights (#080); only its
   drawing overflows, at 1.7 lights across. It is an inline SVG (eight teeth around a ring),
   not U+2699, which some system fonts render as a color emoji — a smudge at 13 px.

3. **The bar owns the preferences; the window edits them.** `prefs.js` holds every key,
   default and getter and is loaded by both webviews. The window writes a change locally and
   emits `pref-changed {key, value}`; the bar writes it too and runs `applyPref`, the one
   place that knows what a preference *does*. On open the window emits `prefs-request` and
   renders from the `prefs-snapshot` the bar returns — so the window shows what is actually
   on screen rather than whatever its own webview's storage happens to hold, and nothing
   rests on two WKWebViews sharing a localStorage they are not guaranteed to share.

4. **The Cursor pip becomes optional** (Lights → Cursor pip, default Show). Hiding it
   removes the pip *and* stops `cursor_attention_count` being called at all — the last
   remaining reader that can cancel Cursor's own menu-bar menu while the user has it open
   (#038, #081). The row is hidden on Windows, where there is no such item to read.

**Three follow-ons, each reported live during the change.**

- **A stadium, not an oval.** `border-radius: 999px` clamps to half the *smaller side*,
  which is exactly right for a one-row bar and wrong the moment the controls row makes the
  bar two rows thick — the corners inflate and the pill reads as an oval. The radius is now
  `--pill-radius: (dot-size + 2 × bar-pad) / 2`, half the light strip's own thickness: the
  identical shape for the plain bar, straight sides for the revealed one.
- **Both directions of the toggle suppress their paint.** #075 hid the bar with
  `visibility: hidden` across a *collapse* only, because the panel grew below the lights and
  revealing moved nothing. The controls row is placed by `panel-above`/`panel-left`, so it
  lands on whichever side the lights already sit on and shifts them **either way** — the
  reported "it's both ways" flicker. The suppression is now symmetric.
- **#074 is dropped.** "The Windows tray popover opens with its settings panel already
  shown" opened a panel that no longer exists. The popover now just shows the lights.

**Options considered.**

| Option | Verdict |
|---|---|
| Settings window with a sidebar (chosen) | Room for the sixteen controls and their explanations; the bar goes back to being only lights; the window is where a Mac user expects settings to be |
| Keep the in-bar panel, just trim it | Does not address the complaint — the panel is the problem, not its length — and every control removed is a feature removed |
| One scrolling page instead of a sidebar | Less code, but the user picked the sidebar; with five sections it also keeps each pane short enough to read without scrolling |
| Close buttons move into the window as a session list | Loses #080's one-for-one alignment between a light and the × that closes it: closing a light would mean finding it in a list |
| localStorage as the shared store, no snapshot | Rests on two webviews sharing one store; a stale read would show the wrong values in the window and write them back |

**Verified.**

- **The window opens, and the bar grows around it.** A synthetic right-click on the bar
  (before the gear existed) produced a second window, `AgentStatus Settings` at 560×431,
  and grew the bar from 37×100 to 57×100 at an x 20 px further left — the controls column
  appearing on the side facing the screen's middle, with the lights unmoved.
- **The window renders live values, not defaults**, and its sections switch: its AX tree
  lists the rail (General / Lights / Colors / Audio / About), the General pane, and the
  footer; pressing **Lights** swapped the pane to Order / Unknown state / Cursor pip.
- **The Cursor pip pref does what it says.** With **Hide** set, 45 s produced **zero**
  `cursor_attention_count` reads in the marker-gated log; with **Show** set again, they
  resumed immediately at the 20 s cadence. Set through the real window, read from the real
  backend.
- **The gear's wiring** — `app/tests/settings-gear.mjs`, evaluating the shipped
  `ensureGear` against a stub DOM: built once rather than once per poll, always last in the
  row with the close buttons undisturbed, drawn as an SVG of eight teeth and a ring, and a
  click invokes `open_settings`. Live clicking could not be scripted reliably here — the
  bar is a non-activating panel whose webview ignored synthetic left clicks even with
  `clickState` set, while right-clicks landed — so the one link a script cannot drive is
  pinned by a test instead.
- **A "vanishing" settings window was instrumented before being believed:** the log showed
  `built → Focused(true) → CloseRequested → Destroyed`, i.e. an ordinary user close, not a
  teardown. The instrumentation stayed only as the one `open_settings` trace line.
- The four existing frontend tests still pass; `light-order` and `dismiss-light` now read
  `prefs.js` alongside `main.js`, since that is where the keys they exercise moved.

## 084 — An orange background-agent light has to be a question

**Date:** 2026-08-18
**Status:** Accepted (narrows #063; restores #065's timer for one case)

**Symptom (reported live).** "Why is there an orange light in my lightbar from a session that
already finished?"

**The light.** Background job `40edcbe8` ("Cursor menu bar notification persistence"). Its own
status file said `idle`, written by its `Stop` hook; the bar painted it orange anyway, and had
been doing so for far longer than a finished job should keep any light at all.

**Root cause.** #063 reads Claude Code's own word on a `--bg` job and maps `state: "blocked"`
to orange, on the premise that `blocked` means *the job stopped and the ball is in your court
because it asked you something*. `blocked` is broader than that. Measured on 2.1.234 against
two live jobs — one told to ask a question and stop, one finished and left alone:

| job | `agents --json` `status`/`state` | `jobs/<id>/state.json` `needs` | what it actually is |
|---|---|---|---|
| `ccae1eca` | `idle` / `blocked` | `"Should the fallback be red or blue?"` | waiting on an answer |
| `40edcbe8` | `idle` / `blocked` | `"send a prompt to start"` | finished, sitting at an empty prompt |

The two are identical in `agents --json`, which carries the job's `tempo` as its `state`. The
`needs` behind that `tempo` separates them, and only the job's own record has it: it is the
question verbatim when one was asked, and the literal `"send a prompt to start"` when nothing
is being asked of the user at all.

The same premise made the light *permanent*. #065 retires a finished background agent's light
after five minutes of hook silence, and exempts `blocked` — a job waiting on an answer goes
silent by nature, and deleting its light is exactly the miss #063 was written to fix. An
unprompted job inherited that exemption, so nothing retired it short of the two-hour
`MAX_IDLE_SECS` backstop.

**Decision.** A `blocked` background job is treated as waiting on the user only when it is
actually asking something. `bg_light_state` and `bg_retirable` both consult `bg_unprompted`,
which reads `needs` from `~/.claude/jobs/<id>/state.json` (only background agents have a job
id, so nothing else pays for the read). When `needs` is `"send a prompt to start"`:

- the light stays as the hook wrote it — `idle`, which is the truth (UI Principle #4), and
- #065's five-minute timer applies again, so the light retires like any other finished job.

**Why an exact string match.** An unrecognised `needs` — a re-worded phrase in a later Claude
Code, a job with no record on disk, a null field — keeps #063's orange. The failure modes are
not symmetric: a light that lingers is noise, a missing orange light is a session waiting on
you that you never see (UI Principle #2). The narrowing only ever fires on the one phrase that
has been observed to mean "nothing is being asked of you".

**Verified.** `dump_cli_facts` against both live jobs at once:

```
40edcbe8 kind=background status=idle state=blocked needs="send a prompt to start"
         -> light=None retirable=true
ccae1eca kind=background status=idle state=blocked needs="Should the fallback be red or blue?"
         -> light=Some("blocked") retirable=false
```

The finished job stops painting orange and becomes retirable; the job that asked a question
keeps its orange light and its exemption from the timer. Unit test
`bg_blocked_needs_a_question` pins all four combinations, including the unrecognised-`needs`
fallback.
