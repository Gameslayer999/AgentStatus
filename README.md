<div align="center">

<img src="docs/logo.png" width="128" alt="AgentStatus icon" />

# AgentStatus

**A small bar of lights that stays above all other windows. It shows the live state of
each open Claude Code or Cursor session. You see immediately which sessions run, which
wait for you, which are idle, and which have an error.**

[![Latest release](https://img.shields.io/github/v/release/Gameslayer999/AgentStatus?sort=semver&label=release)](https://github.com/Gameslayer999/AgentStatus/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/Gameslayer999/AgentStatus/total?label=downloads)](https://github.com/Gameslayer999/AgentStatus/releases)
![Platform](https://img.shields.io/badge/platform-macOS%20·%20Apple%20Silicon-black)

[Install](#install-macos-apple-silicon) · [The lights](#the-lights) · [Customize](#customize-it) · [How it works](#how-it-works) · [VS Code extension](#optional--vs-code-extension)

</div>

![The AgentStatus light bar above the desktop. From left to right: a green light, an orange light, a white light, a green light with a blue subagent badge that shows 2, a red light, and a dim gray light.](docs/lightbar-hero.svg)

<sub>One light for each session. From left to right: running, blocked, done, running with
2 subagents, error, idle. The bar stays above all other windows, also full-screen
applications.</sub>

When you run many agent sessions in different projects and windows, it is difficult to
know their state. AgentStatus shows one colored light for each session. The bar stays
above all other windows, also full-screen applications. The lights change immediately.
Click a light to go directly to that session.

AgentStatus operates with **Claude Code in VS Code** and with the **Cursor agent**. Both
use the same hook. An optional VS Code extension adds a status-bar item to each VS Code
window.

## Install (macOS, Apple Silicon)

The DMG is the fastest method. You do not need build tools. The application installs its
own hooks at the first start.

**Requirements:** macOS on Apple Silicon (M1 or later), and Claude Code or Cursor.

> [!IMPORTANT]
> AgentStatus is **not signed and not notarized**. Thus macOS Gatekeeper stops it at the
> first start. Step 3 removes the download quarantine, and the application starts.

1. Download **`AgentStatus_0.6.4_aarch64.dmg`** from the
   [latest release](https://github.com/Gameslayer999/AgentStatus/releases/latest).
2. Open the DMG. Drag **AgentStatus** into **Applications**.
3. Remove the download quarantine and start the application:

   ```bash
   xattr -dr com.apple.quarantine /Applications/AgentStatus.app
   open /Applications/AgentStatus.app
   ```

   As an alternative, double-click the application and let macOS stop it. Then open
   **System Settings → Privacy & Security**. Find the message "AgentStatus was blocked".
   Click **Open Anyway**. On macOS 15 and later, right-click → Open does not remove this
   block for downloaded applications.

At the first start, the application **installs its own hooks**. It writes
`~/.claude/status/report.sh`. It registers that script for Claude Code
(`~/.claude/settings.json`) and for Cursor (`~/.cursor/hooks.json`). It makes a backup of
the initial files first. **Claude Code sessions that are already open use the hook
immediately. A restart is not necessary.**

AgentStatus is an accessory application and has **no Dock icon**. To start it at login,
add it in **System Settings → General → Login Items**.

**Accessibility permission:** give AgentStatus **Accessibility** permission in System
Settings → Privacy & Security → Accessibility. For VS Code this permission is optional.
With the permission, a click on a light shows a window in the same Space in approximately
0.2 s. Without the permission, the click uses the slower IDE command line and needs
approximately 1 s. For **Cursor**, this permission is necessary. Without it, a click only
brings Cursor to the front.

### Build from source instead

If you have an Intel Mac, or if you want to build the application yourself:

```bash
./install.sh
```

This needs [Rust](https://rustup.rs), Node, and `jq` (`brew install jq`). The script
builds the application and copies it to `/Applications`. The application installs its own
hooks at the first start, as the DMG does. For a new installation, do the Gatekeeper step
above.

## The lights

Each light is one Claude Code or Cursor session.

![The six light states: green for running, orange for blocked with a pulse, white for done, dim gray for idle, red for error with a pulse, and a hollow gray ring for unknown. Each state has a label.](docs/lightbar-states.svg)

![A green light with a blue subagent badge that shows 2, and its tooltip. The tooltip shows the project name, the state, the task, and the subagents that run.](docs/lightbar-hover.svg)

- **A white light is unread.** That session completed a turn, and you did not look at it.
  A click on the light moves you to the session and makes the light gray. The next
  completed turn makes it white again. This applies to Claude Code sessions and to Cursor
  sessions.
- **A hollow ring** is a session that reports no state, such as a Cursor window with no
  open folder. The bar shows "unknown" instead of a color it would only guess. A click
  opens that agent conversation. Set **Unknown → Hide** in the settings to keep these
  rings off the bar.
- **A ring pip** at the end of the bar counts the Cursor agents that wait for you and have
  no light of their own. Click the pip to open the next one in the queue.
- **Put the pointer on a light** to see the project, the session name, the task, and the
  current operation. The session name is the name the host gives the session — for example
  `agentstatus-5b` in Claude Code, or the name of the agent conversation in Cursor. This
  name tells two sessions in the same project folder apart.
- **A blue badge** on a light shows the number of subagents in that session. The tooltip
  gives their types.
- **Click a light** to go to that session. In VS Code, the window comes to the front and
  shows the tab of the session. In Cursor, the application opens that agent conversation.
- **Right-click the bar** to open the settings.
- **Drag the bar** to move it. Hold the bar by the padding, not by a light. The
  application keeps the position.

## Customize it

**Right-click** the bar to open the settings panel. All settings stay after a restart.

![The AgentStatus settings panel. It has Orientation, Sort, and Unknown controls, sliders for Size, Padding, and Opacity, color swatches for Running, Blocked, Done, Idle, and Error, and Reload, Reset to defaults, and Quit links.](docs/lightbar-settings.svg)

- **Mode** — show the lights as the floating bar (default) or in the **macOS menu bar**.
  Refer to [Run it in the menu bar instead](#run-it-in-the-menu-bar-instead).
- **Orientation** — change the bar between a horizontal row and a vertical column. The
  window changes its size to fit the new shape.

![The same light bar as a horizontal row on the left and as a vertical column on the right.](docs/lightbar-orientation.svg)

- **Sort** — put the lights in groups by window, or move the attention states (blocked and
  error) to the front.
- **Unknown** — **Show** (default) or **Hide** the hollow rings.
- **Size, padding, and opacity** — change the size of the lights, the space in the bar,
  and the opacity of the bar. The lights stay fully opaque.
- **Colors** — change the color of each of the five states with the macOS color picker.
- **Audio alerts** — off by default. Set **Audio** to on to get a short tone when a
  session becomes blocked, has an error, or completes a turn. In the sub-panel, select
  which of the three events make a tone, and set the volume. The tone sounds one time at
  the change of state, and only while the application runs.

### Run it in the menu bar instead

For a smaller and permanent presence, set **Mode → Menu bar** in the settings panel. The
same lights become a live item in the macOS menu bar. Click the item to show the full bar
as a popover. Click-to-focus, tooltips, and subagent badges continue to operate.

- **Dots or Single** — show one dot for each session (default), or one dot for the most
  urgent state (error, then blocked, then done, then running, then idle). One dot uses
  less menu-bar width.
- Menu-bar mode is always horizontal. The Orientation control is not available.
- macOS controls the position of menu-bar items. Hold ⌘ and drag the item one time to set
  its position. The system keeps that position.

> [!NOTE]
> The macOS menu bar hides itself in full-screen applications. Thus in menu-bar mode the
> lights are not visible in a full-screen window. Use the floating bar for full-screen
> work, and the menu bar for other work.

## How it works

AgentStatus has two parts:

- **The signal layer** — one hook script (`report.sh`) starts at each session event. It
  writes the state of that session to `~/.claude/status/sessions/<id>.json`. The hook is
  global, thus one installation covers all projects and all Claude Code and Cursor
  windows. The hook does the minimum work and stops immediately. It does not delay a turn.
- **The display layer** — a Tauri application reads that directory and shows the lights,
  as the floating bar or as the menu-bar item.

The status files contain only the data for the lights: the session ID, the state, a short
project name, and a time. AgentStatus does not store prompt text or transcript text.

For the reasons for these decisions, refer to [DECISIONS.md](DECISIONS.md).

## Optional — VS Code extension

The extension adds a status-bar item to each VS Code window, for the workspace of that
window. It has the same click-to-focus and the same tooltip, without the session name. It
reads the same status files, thus it needs the application (or the development hooks) for
the signal.

```bash
code --install-extension extension/claudestatus-0.1.2.vsix
```

## Uninstall

```bash
node hooks/setup.mjs uninstall     # remove the hooks from settings.json
rm -rf /Applications/AgentStatus.app ~/.claude/status
```

The backup of your initial settings is at `~/.claude/settings.json.agentstatus-bak`.

## Develop

```bash
cd app
npm install
node ../hooks/setup.mjs install   # register the repo hooks (development uses hooks/report.sh)
npm run tauri dev
```

In development, the application does **not** install its own hooks. Thus changes to
`hooks/report.sh` are immediately in use, and a rebuild is not necessary. The release
build installs its own hooks. Use `node hooks/setup.mjs status|uninstall` to control the
development hooks.

### Make a release

GitHub Actions builds and publishes the releases. Set the new version in
`app/src-tauri/tauri.conf.json`, `app/package.json`, and `app/src-tauri/Cargo.toml`. Merge
to `main`. Then push a tag with the same version:

```bash
git tag v0.6.4 && git push origin v0.6.4
```

The workflow builds the arm64 DMG on a macOS runner and publishes it with generated
notes. The workflow stops if the tag does not agree with the version in
`tauri.conf.json`. A merge to `main` publishes nothing. The tag starts the workflow.

## Limits

- **macOS only, and the DMG is for Apple Silicon.** For Intel, build from source.
- **Downloaded builds are not signed and not notarized**, thus the Gatekeeper step above
  is necessary.
- **Cursor has no blocked light.** Cursor sends no permission event, so its lights show
  running, done, and idle only.
- **Cursor needs the Accessibility permission** for click-to-focus and to keep its lights
  correct. Without it, a click only brings Cursor to the front, and a long-silent agent
  can go gray while it still works.
