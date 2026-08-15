#!/usr/bin/env node
// AgentStatus — install/uninstall the real status hooks (agentstatus-hook).
//
// Merges the status hooks into the user's ~/.claude/settings.json WITHOUT
// clobbering existing settings or hooks (Agent Guideline #3). Idempotent
// (re-running install never duplicates), reversible (one-time backup + a clean
// uninstall that removes exactly our entries).
//
//   node hooks/setup.mjs install
//   node hooks/setup.mjs uninstall
//   node hooks/setup.mjs status

import { readFileSync, writeFileSync, copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const CLAUDE_SETTINGS = join(homedir(), '.claude', 'settings.json');
const CLAUDE_BACKUP = CLAUDE_SETTINGS + '.agentstatus-bak';
// Hosts a past version registered hooks into (decision 040). No longer installed;
// still cleaned up so old installs don't leave orphaned entries behind.
const LEGACY_CODEX_HOOKS = join(homedir(), '.codex', 'hooks.json');
const LEGACY_ANTIGRAVITY_HOOKS = join(homedir(), '.gemini', 'config', 'hooks.json');
const HOOKS_DIR = dirname(fileURLToPath(import.meta.url));
const HOOK_BIN = join(
  HOOKS_DIR, 'agentstatus-hook', 'target', 'release',
  process.platform === 'win32' ? 'agentstatus-hook.exe' : 'agentstatus-hook',
);

// Both platforms register the native binary, mirroring install.rs (decisions 068 and 076):
// `report.sh` needs a `jq` that Windows does not ship at all and macOS ships only from 15,
// and it costs ~8x more per event. The command is handed to a shell (Git Bash on Windows),
// so it is quoted for spaces and a Windows path is written with forward slashes (a backslash
// is an escape there).
function hookCommand() {
  if (!existsSync(HOOK_BIN)) {
    console.error(`Missing ${HOOK_BIN}\nBuild it first:  npm --prefix app run stage-hook`);
    process.exit(1);
  }
  return `"${HOOK_BIN.replace(/\\/g, '/')}"`;
}

// The exact events the signal layer consumes (verified contract, DECISIONS.md #006).
// Tool-scoped events take a "*" matcher (match all tools); lifecycle events take none.
const SIMPLE = [
  'SessionStart', 'UserPromptSubmit', 'Stop', 'SessionEnd', 'StopFailure',
  'SubagentStart', 'SubagentStop',
];
const TOOL = ['PreToolUse', 'PostToolUse', 'PostToolUseFailure', 'PermissionRequest'];

// Entries this project owns, in any version it has shipped — matching both is what makes a
// re-install replace its own registration instead of stacking a second hook on every event.
const MARKERS = ['report.sh', 'agentstatus-hook'];
const marker = (entry) => MARKERS.some((m) => JSON.stringify(entry).includes(m));
const load = (path) => (existsSync(path) ? JSON.parse(readFileSync(path, 'utf8')) : {});
const save = (path, s) => {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, JSON.stringify(s, null, 2) + '\n');
};

function installHooks(path, backup, simpleEvents = SIMPLE, toolEvents = TOOL) {
  if (existsSync(path) && !existsSync(backup)) copyFileSync(path, backup);
  const command = hookCommand();
  const s = load(path);
  s.hooks ??= {};
  const add = (event, withMatcher) => {
    const kept = (s.hooks[event] ?? []).filter((e) => !marker(e)); // drop stale entries
    const hook = { type: 'command', command: `${command} ${event}` };
    kept.push(withMatcher ? { matcher: '*', hooks: [hook] } : { hooks: [hook] });
    s.hooks[event] = kept;
  };
  simpleEvents.forEach((e) => add(e, false));
  toolEvents.forEach((e) => add(e, true));
  save(path, s);
}

function uninstallHooks(path) {
  if (!existsSync(path)) return;
  const s = load(path);
  if (s.hooks) {
    for (const event of Object.keys(s.hooks)) {
      s.hooks[event] = (s.hooks[event] ?? []).filter((e) => !marker(e));
      if (s.hooks[event].length === 0) delete s.hooks[event];
    }
    if (Object.keys(s.hooks).length === 0) delete s.hooks;
  }
  save(path, s);
}

// Remove hooks a past version registered into Codex and Antigravity (decision 040).
// Runs on install and uninstall alike, so upgrading clears the orphans that would
// otherwise keep firing report.sh. Never creates a file that isn't already there.
function cleanupLegacyHosts() {
  uninstallHooks(LEGACY_CODEX_HOOKS);
  if (existsSync(LEGACY_ANTIGRAVITY_HOOKS)) {
    const s = load(LEGACY_ANTIGRAVITY_HOOKS);
    if (s.agentstatus) {
      delete s.agentstatus;
      save(LEGACY_ANTIGRAVITY_HOOKS, s);
    }
  }
}

function hookEvents(path) {
  const s = load(path);
  return s.hooks
    ? Object.entries(s.hooks).filter(([, arr]) => (arr ?? []).some(marker)).map(([e]) => e)
    : [];
}

const cmd = process.argv[2] || 'status';

if (cmd === 'install') {
  installHooks(CLAUDE_SETTINGS, CLAUDE_BACKUP);
  cleanupLegacyHosts();
  console.log(`Installed AgentStatus hooks for ${SIMPLE.length + TOOL.length} events into ${CLAUDE_SETTINGS}`);
  console.log(`Backup: ${CLAUDE_BACKUP}`);
} else if (cmd === 'uninstall') {
  uninstallHooks(CLAUDE_SETTINGS);
  cleanupLegacyHosts();
  console.log(`Removed AgentStatus hooks from ${CLAUDE_SETTINGS}`);
} else {
  const claudeEvents = hookEvents(CLAUDE_SETTINGS);
  console.log(claudeEvents.length ? `Claude hooks active on: ${claudeEvents.join(', ')}` : 'Claude hooks not installed');
}
