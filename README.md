<div align="center">

<img src="docs/logo.png" width="128" alt="AgentStatus icon" />

# AgentStatus

**A small bar of lights that stays above all other windows. It shows the live state of each
open Claude Code or Cursor session.**

[![Latest release](https://img.shields.io/github/v/release/Gameslayer999/AgentStatus?sort=semver&label=release)](https://github.com/Gameslayer999/AgentStatus/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/Gameslayer999/AgentStatus/total?label=downloads)](https://github.com/Gameslayer999/AgentStatus/releases)
![Platform](https://img.shields.io/badge/platform-macOS%20·%20Windows-black)
[![License](https://img.shields.io/badge/license-MIT-black)](LICENSE)

[Install](#install) · [The lights](#the-lights) · [Settings](#settings) · [How it works](#how-it-works) · [Limits](#limits)

</div>

![The AgentStatus light bar above the desktop. From left to right: a green light, an orange light, a white light, a green light with a blue subagent badge that shows 2, a red light, and a dim gray light.](docs/lightbar-hero.svg)

One light shows one session. The lights change immediately. Click a light to go to that
session. The bar stays above all other windows, also full-screen applications.

AgentStatus operates with **Claude Code** in VS Code, in a terminal, and in Claude Desktop,
and with the **Cursor agent**. One hook covers all of them.

## Install

Use the installer from the latest release. Build tools are not necessary. The application
installs its own hook at the first start.

**Requirements:** Claude Code or Cursor, and **macOS 13 or later** (Apple Silicon or Intel)
or **Windows 11 on x64**.

### macOS

> [!IMPORTANT]
> The application is not signed and not notarized. Gatekeeper stops it at the first start.
> Step 3 removes the download quarantine.

1. Download the universal `.dmg` from the
   [latest release](https://github.com/Gameslayer999/AgentStatus/releases/latest).
2. Open the DMG. Drag **AgentStatus** into **Applications**.
3. Remove the quarantine and start the application:

   ```bash
   xattr -dr com.apple.quarantine /Applications/AgentStatus.app
   open /Applications/AgentStatus.app
   ```

   As an alternative, start the application, then open **System Settings → Privacy &
   Security** and click **Open Anyway**.

The application has no Dock icon. To start it at login, add it in **System Settings →
General → Login Items**.

**Give the Accessibility permission** in **System Settings → Privacy & Security →
Accessibility**. With this permission, a click on a light shows the window in the same Space
in approximately 0.2 s. Without it, a click on a VS Code light uses the slower IDE command
line and needs approximately 1 s, and a click on a Cursor light only brings Cursor to the
front.

**Approve the Automation prompt.** The first click on a light for a terminal session asks
whether AgentStatus may control that terminal application. Click **OK**. This selects the
tab. macOS asks one time for each terminal application.

### Windows

> [!IMPORTANT]
> The installer is not signed. SmartScreen shows "Windows protected your PC" at the first
> start. Click **More info → Run anyway**.

1. Download the `x64-setup.exe` from the
   [latest release](https://github.com/Gameslayer999/AgentStatus/releases/latest). An `.msi`
   is also available.
2. Run the installer.
3. Start **AgentStatus** from the Start menu.

To start it at login, press <kbd>Win</kbd>+<kbd>R</kbd>, enter `shell:startup`, and put a
shortcut to AgentStatus in the folder. Windows needs no permission grants.

### What the first start does

The application writes the hook to `~/.claude/status/agentstatus-hook` and registers it in
`~/.claude/settings.json`. It makes a backup of that file first. Open sessions use the hook
immediately. A restart is not necessary. Claude Code reads this one file in VS Code, in a
terminal, and in Claude Desktop. Cursor reads it through its Claude-compatible bridge.

## The lights

![The six light states: green for running, orange for blocked with a pulse, white for done, dim gray for idle, red for error with a pulse, and a hollow gray ring for unknown. Each state has a label.](docs/lightbar-states.svg)

- **Green means that the session runs. Red means that a turn or a tool failed.** The
  orange and the red lights pulse, thus you see them immediately.
- **Orange means one thing: that session waits for your decision.** It is a permission
  prompt or a question. Click the light to answer it.
- **White means unread.** The session completed a turn, and you did not look at it. A click
  makes the light gray. The next completed turn makes it white again.
- **Gray means idle.** A session that you stop with Ctrl+C or Esc goes gray in approximately
  one second.
- **A hollow ring** is a session that reports no state, for example a Cursor window with no
  open folder. A click opens that conversation.
- **A blue badge** shows the number of subagents in that session.
- **A ring pip** at the end of the bar counts the Cursor agents that wait for you and have no
  light. A click opens the next agent in the queue and clears all Cursor notifications,
  because Cursor gives no way to clear one.

![A green light with a blue subagent badge that shows 2, and its tooltip. The tooltip shows the project name, the session name, the application, the state, the task, and the subagents that run.](docs/lightbar-hover.svg)

- **Point at a light** to see the project, the session name, the application, the task, and
  the current operation. The application is `VS Code`, `Cursor`, `Claude Desktop`, the
  terminal (`Ghostty`, `Terminal`, `iTerm2`), or `background agent`.
- **Click a light** to go to that session. VS Code and Cursor show the tab or the
  conversation. Terminal.app and Ghostty select the tab. A background agent opens in a
  Ghostty tab, and each subsequent click uses that same tab.
- **A light goes away with its session.** A background agent keeps its light for five more
  minutes. A light that waits for you (orange or red) stays until you answer it.
- **Drag the bar** to move it. Hold the bar by the padding, not by a light. The application
  keeps the position.
- **Right-click the bar** to show a red × below each light and the settings gear. Click a ×
  to remove that light now. Right-click again to hide the controls.

## Settings

Click the gear to open the settings window. It has **General**, **Lights**, **Colors**,
**Audio**, and **About** sections. All settings stay after a restart.

![The AgentStatus settings window: a sidebar listing General, Lights, Colors, Audio, and About, with the General pane showing Mode and Orientation toggles, sliders for light size, padding, and opacity, and Reload, Reset to defaults, and Quit AgentStatus buttons along the bottom.](docs/lightbar-settings.svg)

- **Mode** — the floating bar (default), or the macOS menu bar / the Windows notification
  area. Refer to [Menu-bar mode](#menu-bar-mode).
- **Orientation** — a horizontal row or a vertical column.
- **Order** — **Stable** (default) keeps each light in its slot. **Urgency** moves error,
  then blocked, then done to the front.
- **Unknown state** — show (default) or hide the hollow rings.
- **Cursor pip** — show (default) or hide the pip. Hidden, it also stops the Accessibility
  read that can close Cursor's own menu.
- **Light size, padding, and opacity** — the lights stay fully opaque.
- **Colors** — set the color of each of the five states.
- **Audio alerts** — off by default. Set **Alerts** to on for a tone when a session becomes
  blocked, has an error, or completes a turn. Select the events and the volume.

### Menu-bar mode

Set **Mode → Menu bar** (macOS) or **Mode → Tray** (Windows). Click the item to show the
full bar as a popover. Click-to-focus, tooltips, and badges continue to operate. Click
anywhere outside the popover to close it.

In the menu bar, the two states that need you also have their own shape: **blocked is a
triangle** and **error is a square**. The other states stay round. Thus you can identify them
without color, at the small size of a menu-bar icon.

- **Dots or Single** (macOS) — one dot for each session (default), or one dot for the most
  urgent state. On Windows the item is always one dot, because the icon is square.
- Windows 11 puts a new icon in the **hidden icons** flyout (`^`). Drag it onto the taskbar.
- This mode is always horizontal.
- macOS controls the position. Hold ⌘ and drag the item one time to set it.

> [!NOTE]
> The macOS menu bar hides itself in full-screen applications. Use the floating bar for
> full-screen work.

## How it works

- **The signal layer** — a hook starts at each session event and writes the state to
  `~/.claude/status/sessions/<id>.json`. The hook is a compiled binary with no dependencies.
  It does the minimum work and stops immediately. It does not delay a turn.
- **The display layer** — a Tauri application reads that directory and shows the lights.

The status files contain only the session ID, the state, a short project name, and a time.
AgentStatus does not store prompt text or transcript text. For the reasons for these
decisions, refer to [DECISIONS.md](DECISIONS.md).

## Optional — VS Code extension

The extension adds a status-bar item to each VS Code window, for the workspace of that
window. It shows **Claude Code sessions in VS Code only**. It reads the same status files,
thus it needs the application for the signal.

```bash
code --install-extension extension/agentstatus-0.1.3.vsix
```

## Uninstall

**macOS:**

```bash
node hooks/setup.mjs uninstall
rm -rf /Applications/AgentStatus.app ~/.claude/status
```

**Windows:** remove **AgentStatus** in Settings → Apps → Installed apps. Then:

```bash
node hooks/setup.mjs uninstall
rm -rf ~/.claude/status
```

The backup of your initial settings is at `~/.claude/settings.json.agentstatus-bak`.

## Limits

- **Downloaded builds are not signed.** The Gatekeeper step (macOS) or the SmartScreen step
  (Windows) above is necessary.
- **Cursor has no blocked light.** Cursor sends no permission event. Its lights show
  running, done, and idle only.
- **Cursor needs the Accessibility permission** on macOS for click-to-focus and correct
  lights.
- **Only Terminal.app and Ghostty 1.3 or later get tab-precise focus.** Another terminal
  application comes to the front, and you select the tab. A session without a title from
  Claude Code also falls back to this.
- **On Windows, a click brings the window forward, not the tab or the exact Cursor agent.**
  Windows Terminal gives no way to select a tab. AgentStatus tells its windows apart by the
  session title, thus an untitled session cannot be placed.
- **Windows Subsystem for Linux is not supported.** A session in WSL writes its status in
  WSL, where the Windows application cannot read it.
- **Claude Desktop chat threads have no light.** Claude Desktop gives no hook. Claude Code
  inside Claude Desktop is fully supported.

## Build from source

**macOS** — `./install.sh` builds the application for your architecture and copies it to
`/Applications`. It needs [Rust](https://rustup.rs) and Node. Do the Gatekeeper step above.

**Windows** — this needs [Rust](https://rustup.rs) (MSVC), the Visual Studio Build Tools with
the C++ workload, and Node. Run the installer from
`app/src-tauri/target/release/bundle/`:

```bash
npm --prefix app ci
npm --prefix app run tauri build
```

For the development setup and the release procedure, refer to
[docs/DEVELOPING.md](docs/DEVELOPING.md).

## License

[MIT](LICENSE). Copyright (c) 2026 Gameslayer999.
