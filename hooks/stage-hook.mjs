#!/usr/bin/env node
// Build the native status hook and stage it where tauri.conf.json expects to find it as a
// bundle resource (decision 068).
//
// Why a staging step rather than referencing target/release directly: Tauri resolves
// `bundle.resources` for `tauri dev` as well as `tauri build`, and errors if a listed path
// is missing. Pointing at `target/release/…` would therefore break `tauri dev` on any tree
// that has never done a release build. Staging gives the config one stable path that always
// exists, and keeps the bundled hook in lockstep with the source (Agent Guideline #8: the
// step is a script, not a thing you remember to do).
//
// Wired to `beforeBuildCommand` and `beforeDevCommand`, so it runs on every build of either
// kind. Re-running is cheap: cargo no-ops when nothing changed, and the copy is skipped when
// the bytes already match.
//
//   node hooks/stage-hook.mjs

import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync, readFileSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = dirname(dirname(fileURLToPath(import.meta.url)));
const CRATE = join(REPO, 'hooks', 'agentstatus-hook');
const TAURI = join(REPO, 'app', 'src-tauri');
const BIN = process.platform === 'win32' ? 'agentstatus-hook.exe' : 'agentstatus-hook';

// Always the release build: this is the binary users get, and it is what makes the hook
// fast enough to satisfy Agent Guideline #3. A debug hook would ship a slow one.
execFileSync('cargo', ['build', '--release'], { cwd: CRATE, stdio: 'inherit' });

const src = join(CRATE, 'target', 'release', BIN);
if (!existsSync(src)) {
  console.error(`stage-hook: cargo did not produce ${src}`);
  process.exit(1);
}

// The hook runs on *every tool call*. A console-subsystem binary makes Windows allocate a
// console for each one, which flashes a black window on and off the user's screen all day —
// shipped once, and it is not something a test would catch. `#![windows_subsystem =
// "windows"]` in main.rs prevents it; this refuses to stage a binary where that went missing.
if (process.platform === 'win32') {
  const pe = readFileSync(src);
  const peOffset = pe.readUInt32LE(0x3c);
  // Optional header starts at peOffset+24; Subsystem sits 68 bytes into it in both PE32 and
  // PE32+, because every field before it is the same size in the two layouts.
  const subsystem = pe.readUInt16LE(peOffset + 24 + 68);
  const WINDOWS_GUI = 2;
  if (subsystem !== WINDOWS_GUI) {
    console.error(
      `stage-hook: ${BIN} is subsystem ${subsystem}, expected ${WINDOWS_GUI} (windows).\n` +
        'It would flash a console window on every hook event. Check that\n' +
        '`#![windows_subsystem = "windows"]` is still in hooks/agentstatus-hook/src/main.rs.',
    );
    process.exit(1);
  }
}

const dstDir = join(TAURI, 'resources');
const dst = join(dstDir, BIN);
mkdirSync(dstDir, { recursive: true });

// Skip an identical copy so a rebuild never rewrites the file the running app may be
// reading, and so `tauri dev` restarts stay quiet.
if (existsSync(dst) && readFileSync(dst).equals(readFileSync(src))) {
  console.log(`stage-hook: ${BIN} already current`);
} else {
  copyFileSync(src, dst);
  console.log(`stage-hook: staged ${BIN}`);
}
