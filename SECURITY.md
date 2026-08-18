# Security Policy

AgentStatus installs a hook into your agent sessions and runs as a permanently open window
on your desktop. Report anything that lets someone else use that position.

## Supported versions

Only the [latest release](https://github.com/Gameslayer999/AgentStatus/releases/latest) gets
fixes. Older versions get none. Update before you report.

## Report a vulnerability

Use GitHub's private reporting:
**[Report a vulnerability](https://github.com/Gameslayer999/AgentStatus/security/advisories/new)**
(the repository's **Security** tab → **Report a vulnerability**). The report stays private
until a fix ships.

Do not open a public issue for a vulnerability.

Include:

- The version, the operating system, and the host (Claude Code in VS Code, in a terminal, in
  Claude Desktop, or Cursor).
- The exact steps that reproduce it.
- What an attacker gains, and what access they need to start.

This is a one-person project. You get an acknowledgement within 7 days and an assessment
within 14. A confirmed vulnerability is fixed in the next release, and the advisory credits
you unless you ask otherwise.

## Scope

**In scope:**

- The application in `app/` — the bar, the settings window, and the Tauri commands behind
  them.
- The hook binary in `hooks/agentstatus-hook/`, which runs inside every one of your agent
  sessions.
- The installers — the first-start install, `hooks/setup.mjs`, and `install.sh` — and what
  they write to `~/.claude/`.
- The VS Code extension in `extension/`.
- The release workflow in `.github/workflows/` and the artifacts it publishes.

**Out of scope:**

- **The unsigned builds.** The macOS DMG is not signed or notarized and the Windows installer
  is not signed. This is stated in the README under *Limits*. It is a known state, not a
  finding.
- **Anything that needs code execution as you already.** The status files sit in your home
  directory under your own file permissions. An attacker who can already write there has your
  account.
- **Claude Code, Cursor, VS Code, and Tauri themselves.** Report those upstream. Do report it
  here if AgentStatus makes one of their weaknesses reachable or worse.

## What the application touches

This is the whole footprint, so you can judge a finding against it.

- **It makes no network connections.** No telemetry, no update check, no outbound traffic.
  Nothing it reads leaves the machine.
- **It writes `~/.claude/status/`** — the hook binary and one JSON file per session. Each file
  holds the session ID, the state, the working directory, the host, the process ID, the
  project folder name, a timestamp, **up to 160 characters of the prompt that started the
  turn**, and one truncated line describing the current tool call (for a `Bash` call, the
  command). At the end of a turn that line holds **up to 160 characters of the assistant's
  last message**. Nothing else from a transcript or a model reply is stored.
- **It edits `~/.claude/settings.json`** to register the hook, after copying that file to
  `~/.claude/settings.json.agentstatus-bak`. **Settings → About → Uninstall AgentStatus**
  removes the registration and the status directory; `node hooks/setup.mjs uninstall`
  removes the registration alone.
- **It reads, and never writes,** Claude Code's session records in `~/.claude/sessions/`,
  Claude Code's session transcripts in `~/.claude/projects/` (only the `ai-title` record is
  taken out of them), and Cursor's `state.vscdb` (opened `-readonly`), for session names and
  state.
- **It runs local commands on a click** — `osascript`, `code`, or `cursor` — to bring the
  session's window forward.
- **On macOS it asks for Accessibility and Automation permissions.** Accessibility raises
  windows and reads Cursor's menu-bar item. Automation selects a terminal tab. Both are used
  only on the click of a light, and the pip's Accessibility read can be turned off in
  **Settings → Lights → Cursor pip**.
