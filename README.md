<div align="center">

<img src="docs/logo.png" width="128" alt="AgentStatus icon" />

# AgentStatus

**A small, always-on-top bar of lights showing the live status of every open Claude Code
or Cursor session — so you can tell at a glance which agent is working, waiting on you,
idle, or errored, without hunting through windows.**

[![Latest release](https://img.shields.io/github/v/release/Gameslayer999/AgentStatus?sort=semver&label=release)](https://github.com/Gameslayer999/AgentStatus/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/Gameslayer999/AgentStatus/total?label=downloads)](https://github.com/Gameslayer999/AgentStatus/releases)
![Platform](https://img.shields.io/badge/platform-macOS%20·%20Apple%20Silicon-black)

[Install](#install-macos-apple-silicon) · [The lights](#the-lights) · [Customize](#customize-it) · [How it works](#how-it-works) · [VS Code extension](#optional--vs-code-extension)

</div>

![The AgentStatus light bar floating over the desktop: green (running), orange (blocked), white (done), a running session with a blue "2" subagent badge, red (error), and a dim gray (idle) light.](docs/lightbar-hero.svg)

<sub>One light per session. Left to right: running · blocked · done · running with 2 subagents · error · idle. It floats over everything, including full-screen apps.</sub>

Run several agent sessions across projects and windows and it's easy to lose track of
which one just finished, which is blocked on a permission prompt, and which hit an
error. AgentStatus floats one colored light per session over everything on screen
(including full-screen apps), updates in real time, and lets you click a light to jump
straight to that session.

Works with **Claude Code in VS Code** and **Cursor's native agent** (both drive the same
hook). There's also an optional VS Code extension that adds a per-window status-bar item
for Claude Code in VS Code.

## Install (macOS, Apple Silicon)

The fastest path is the prebuilt DMG — no build tools, and the app wires up all its hooks
itself on first launch.

**Requirements:** macOS on Apple Silicon (M1 or later), and Claude Code or Cursor.

> [!IMPORTANT]
> AgentStatus is **unsigned and unnotarized**, so macOS Gatekeeper blocks it on first
> launch. Step 3 below clears the download quarantine so it opens — nothing is code-signed
> yet.

1. Download **`AgentStatus_0.6.1_aarch64.dmg`** from the
   [latest release](https://github.com/Gameslayer999/AgentStatus/releases/latest).
2. Open the DMG and drag **AgentStatus** into **Applications**.
3. The app is **unsigned**, so macOS Gatekeeper blocks it on first launch. Clear the
   download quarantine and open it:

   ```bash
   xattr -dr com.apple.quarantine /Applications/AgentStatus.app
   open /Applications/AgentStatus.app
   ```

   (Alternatively: double-click it, let macOS block it, then go to **System Settings →
   Privacy & Security**, scroll to the "AgentStatus was blocked" message, and click
   **Open Anyway**. On macOS 15+ the old right-click → Open shortcut no longer bypasses
   this for downloaded apps.)

On first launch the app **installs its own hooks** — it writes
`~/.claude/status/report.sh` and registers it for Claude Code
(`~/.claude/settings.json`) and Cursor (`~/.cursor/hooks.json`), backing up the originals
first. **Already-open Claude Code sessions pick it up immediately — no restart needed.**

AgentStatus is an accessory app (**no Dock icon**). To start it at login, add it in
**System Settings → General → Login Items**.

**Faster click-to-focus (and required for Cursor):** grant AgentStatus **Accessibility**
permission (System Settings → Privacy & Security → Accessibility). For VS Code this is
optional — it lets a light click raise a same-Space window in ~0.2s instead of ~1s, and
without it click-to-focus still works via the slower IDE CLI. For **Cursor** it is what
makes a click open the agent's conversation at all; without it a click only brings Cursor
forward.

### Build from source instead

If you're on Intel, or want to build it yourself:

```bash
./install.sh
```

This needs [Rust](https://rustup.rs), Node, and `jq` (`brew install jq`). It builds the
app and copies it to `/Applications`; the app self-installs its hooks on first launch,
same as the DMG. (On a fresh install you still need the Gatekeeper step above.)

## The lights

Each light is one Claude Code or Cursor session:

![The six light states: green running, orange blocked (pulsing), white done, dim gray idle, red error (pulsing), and a hollow gray ring for unknown, each labeled with its meaning.](docs/lightbar-states.svg)

![A running light with a blue "2" subagent badge and its hover tooltip, showing the project name and state, the task, and the running subagents.](docs/lightbar-hover.svg)

- **A hollow ring** means a session whose state can't be read at all — a Cursor window with
  no folder open never reports progress, so the bar says "unknown" instead of showing a color
  it would only be guessing at. Clicking it opens that agent's conversation if Cursor still
  lists it, and otherwise just brings Cursor forward; **Unknown → Hide** in settings keeps
  these rings off the bar entirely.
- **Hover** a light to see the session's project, its task, and what it's doing right now.
- **A blue count badge** on a light means that session has that many subagents running
  (hover lists their types).
- **Click** a light to jump to that session — in VS Code its window comes forward with the
  session's tab revealed; in Cursor that agent's conversation is opened (via Cursor's own
  menu-bar list, so it needs Accessibility permission — otherwise the click just brings
  Cursor forward).
- **Right-click** the bar to open settings — orientation (row/column), light size, spacing,
  per-state colors, and bar opacity.
- **Drag** the bar (grab the padding, not a light) to position it anywhere; it remembers
  where you put it and floats over everything, including full-screen apps.

## Customize it

**Right-click** the bar to open the settings panel — everything is adjustable and persists
across restarts:

![The AgentStatus settings panel: Orientation, Sort, and Unknown (Show/Hide) toggles, Size, Padding, and Opacity sliders, per-state color swatches (Running, Blocked, Done, Idle, Error), and Reload / Reset to defaults / Quit links.](docs/lightbar-settings.svg)

- **Mode** — run the lights as the floating bar (default) or up in the **macOS menu bar** (see
  [below](#run-it-in-the-menu-bar-instead)).
- **Orientation** — flip the bar between a horizontal row and a vertical column; the window
  auto-resizes to hug the new shape.

![The same light bar shown as a horizontal row on the left and a vertical column on the right.](docs/lightbar-orientation.svg)

- **Sort** — group lights by window, or push the attention states (blocked/error) to the front.
- **Unknown** — **Show** (default) or **Hide** the hollow no-signal rings, if you'd rather see
  only lights whose state actually means something.
- **Size, padding, and opacity** — scale the lights, tighten or loosen the bar, and fade the
  pill (the lights themselves always stay fully opaque so the signal never dims).
- **Per-state colors** — recolor any of the five states with a native color picker.
- **Audio alerts** — off by default; flip **Audio** on to get a short chime when a session
  turns blocked, errors, or finishes a turn. Toggle which of those three chime, and set the
  volume, in the sub-panel that appears. The chime fires once on the transition, not on a
  loop, and only while the app is running.

### Run it in the menu bar instead

Prefer a tidy always-there presence over a floating bar? In the settings panel set **Mode →
Menu bar**. The same lights render as a live status item in the macOS menu bar, and clicking
it drops the full bar down as a popover — so per-light click-to-focus, hover tooltips, and
subagent badges all still work.

- **Dots vs. Single** — show one dot per session (default), or condense to a single dot for the
  most-urgent state (error → blocked → done → running → idle) to save menu-bar width.
- Menu-bar mode is always horizontal (the Orientation control is hidden there).
- macOS decides where third-party menu-bar items sit; ⌘-drag the item once to pin it where you
  want (the OS remembers).

> [!NOTE]
> The macOS menu bar auto-hides in full-screen apps, so in menu-bar mode the lights disappear
> while you're in a full-screen window — exactly the case the floating bar exists to cover. Use
> floating for glancing over full-screen apps, menu bar for a tidy presence otherwise.

## How it works

Two pieces, decided independently (see [DECISIONS.md](DECISIONS.md) for the why):

- **Signal layer** — a single **hook** (`report.sh`) fires on session lifecycle events and
  writes each session's state to `~/.claude/status/sessions/<id>.json`. Hooks are global, so
  **one install covers every project and Claude Code / Cursor window**.
  The hook does the minimum work and exits — it never blocks or slows down a turn.
- **Display layer** — a **Tauri** app (a non-activating macOS `NSPanel`) watches that
  directory and renders the lights, either as the floating bar or as a live menu-bar item
  (the item is an image the webview paints from the same lights, so a click reveals the panel
  itself as a popover — see [Run it in the menu bar instead](#run-it-in-the-menu-bar-instead)).

The status file holds only what the lights need — `session_id`, coarse state, a short
project label, and a timestamp. No prompt or transcript content is stored.

## Optional — VS Code extension

The extension adds a per-window status-bar item (scoped to that window's workspace) with
the same hover detail and click-to-focus. It reads the same status files, so it needs the
app (or the dev hooks) installed for the signal.

```bash
code --install-extension extension/claudestatus-0.1.2.vsix
```

## Uninstall

```bash
node hooks/setup.mjs uninstall     # remove the hooks from settings.json
rm -rf /Applications/AgentStatus.app ~/.claude/status
```

Your original settings are backed up at `~/.claude/settings.json.agentstatus-bak`.

## Develop

```bash
cd app
npm install
node ../hooks/setup.mjs install   # register the Claude repo hooks (dev points at hooks/report.sh)
npm run tauri dev
```

In dev the app does **not** self-install (so edits to `hooks/report.sh` are live without a
rebuild); the release build does. `node hooks/setup.mjs status|uninstall` manages the dev
hooks.

### Cutting a release

Releases are built and published by GitHub Actions. Bump the version in
`app/src-tauri/tauri.conf.json`, `app/package.json`, and `app/src-tauri/Cargo.toml`, merge
to `main`, then push a matching tag:

```bash
git tag v0.6.1 && git push origin v0.6.1
```

The workflow builds the arm64 DMG on a macOS runner and publishes it with generated notes.
It fails fast if the tag doesn't match the version in `tauri.conf.json`. Merging to `main`
alone publishes nothing — the tag is the trigger.

## Notes & limits

- **macOS only** (uses a non-activating `NSPanel` + private transparency API to float
  over full-screen apps). Prebuilt DMG is **Apple Silicon only**; Intel builds from source.
- **Downloaded builds are unsigned/unnotarized** — hence the Gatekeeper step; nothing is
  Apple-notarized. When you build locally, `install.sh` re-signs the app with a per-machine
  self-signed identity so its **Accessibility permission survives rebuilds/updates** (grant
  once); this does not affect Gatekeeper for downloaded copies.
- A light == one `session_id`, labeled by its project folder. Two windows on the *same*
  folder collapse into one label.
- On **Cursor**, per-session lights show running (green) and idle — blocked (orange) is
  unavailable (Cursor emits no permission event), and its turn-finished event carries no
  wrap-up, so a Cursor session never lights as "done". Per-session lights also require a
  **folder-open** Cursor window (Cursor runs no hooks in a folder-less window) — a folder-less
  one still fires a single opening event, so it appears as a **hollow "unknown" ring**: a
  session is there, but nothing about its state can be read. To cover the
  "done"/attention gap, AgentStatus **mirrors Cursor's own menu-bar item**: a hollow-ring pip
  with a count appears on the bar when Cursor has composers awaiting you (even for
  folder-less/background agents). **Clicking the pip opens the next composer that's waiting**
  — Cursor jumps to that agent and clears its notification, so the count ticks down by one and
  the next click takes you to the one after it, until the pip disappears. Reading and clicking
  Cursor's menu bar needs AgentStatus to have **Accessibility** permission (the same grant the
  fast window-switch uses) — the app prompts for it on launch; without it the pip just doesn't
  appear.
- Cursor's hooks go quiet at the end of an agent's life — archiving one fires no closing
  event, and a subagent turn or an aborted one fires no finish event — so lights used to
  linger or sit green on an agent that was done. The bar now checks Cursor's own record
  every few seconds and follows it: **archiving an agent removes its light**, a **finished
  or aborted agent drops to gray** even when no finish event arrived, and **Cursor
  subagents get no light of their own** — they count toward their parent agent's blue
  subagent badge, like Claude Code's. That badge is read from Cursor too, so a subagent
  whose finish event never arrives no longer leaves a light claiming work that ended. A
  light is only overruled once it has been silent for a minute *and* none of its subagents
  is still reporting, so an agent waiting on a subagent stays green.
- Earlier versions (≤ 0.4.2) also registered hooks for **Codex** (`~/.codex/hooks.json`)
  and **Antigravity** (`~/.gemini/config/hooks.json`). Neither host was ever verified
  against a live install, so both are removed. Launching the app — or running
  `node hooks/setup.mjs install|uninstall` — deletes those leftover entries and leaves any
  other hooks in those files untouched.
- Sessions with no activity for 2h are pruned (they reappear on their next event).
- Subagents are tracked by lifecycle (which are running + their types), not by their
  individual live tool calls — those aren't attributable to a specific subagent.
