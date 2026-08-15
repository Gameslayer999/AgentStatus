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
// **Universal builds (decision 076).** The macOS DMG ships arm64 + x86_64 so Intel Macs are
// covered, and a *binary* hook is arch-specific where `report.sh` was not — an arm64-only
// hook inside a universal app would leave every Intel user with a silently dead app. Set
// `AGENTSTATUS_HOOK_UNIVERSAL=1` (the release workflow does) to build both slices and `lipo`
// them; the result is verified to carry both before it is staged. Unset — every local build —
// stages the host arch only, so no developer needs a second rustup target installed.
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
const UNIVERSAL = process.env.AGENTSTATUS_HOOK_UNIVERSAL === '1' && process.platform === 'darwin';
const SLICES = ['aarch64-apple-darwin', 'x86_64-apple-darwin'];

// Always the release build: this is the binary users get, and it is what makes the hook
// fast enough to satisfy Agent Guideline #3. A debug hook would ship a slow one.
const cargo = (args) => execFileSync('cargo', args, { cwd: CRATE, stdio: 'inherit' });

let src;
if (UNIVERSAL) {
  // Idempotent and a no-op when the targets are already there (Agent Guideline #8) — this is
  // what keeps "build a universal hook" one command rather than a documented prerequisite.
  execFileSync('rustup', ['target', 'add', ...SLICES], { stdio: 'inherit' });
  for (const slice of SLICES) cargo(['build', '--release', '--target', slice]);
  src = join(CRATE, 'target', 'universal', 'release', BIN);
  mkdirSync(dirname(src), { recursive: true });
  execFileSync('lipo', [
    '-create', '-output', src,
    ...SLICES.map((s) => join(CRATE, 'target', s, 'release', BIN)),
  ], { stdio: 'inherit' });

  // A hook that runs on only one of the two architectures the DMG claims to support is
  // exactly the silent failure this change exists to remove, so prove it before staging.
  const archs = execFileSync('lipo', ['-archs', src], { encoding: 'utf8' }).trim().split(/\s+/);
  for (const need of ['arm64', 'x86_64']) {
    if (!archs.includes(need)) {
      console.error(`stage-hook: ${BIN} is missing the ${need} slice (has: ${archs.join(', ')})`);
      process.exit(1);
    }
  }
} else {
  cargo(['build', '--release']);
  src = join(CRATE, 'target', 'release', BIN);
}

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
