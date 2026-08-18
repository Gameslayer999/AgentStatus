# Developing AgentStatus

## Development setup

```bash
cd app
npm install
node ../hooks/setup.mjs install   # register the repo hooks for development
npm run tauri dev
```

In development, the application does **not** install its own hooks, thus the hooks that the
repository registers stay in use. They point at the compiled hook. An edit to
`hooks/agentstatus-hook/` therefore needs `npm --prefix app run stage-hook` before it
operates. If the binary is not present, `node hooks/setup.mjs install` reports what to run.
The release build installs its own hooks. Use `node hooks/setup.mjs status|uninstall` to
control the development hooks.

## Windows diagnostics

`hooks/windows-diagnostics.ps1` answers three questions that the code does not: whether the
bar flashes a console window, whether it takes a taskbar slot (it must not) and whether its
tray icon is present, and which windows the application owns. Each check found a real defect.

```powershell
powershell -ExecutionPolicy Bypass -File hooks\windows-diagnostics.ps1
```

## Make a release

GitHub Actions builds and publishes the releases.

1. Set the new version in `app/src-tauri/tauri.conf.json`, `app/package.json`,
   `app/src-tauri/Cargo.toml`, and `hooks/agentstatus-hook/Cargo.toml`.
2. Build one time, so that both `Cargo.lock` files and `app/package-lock.json` get the new
   number.
3. Write the changes in `docs/release-notes/v<version>.md`. The workflow puts that text above
   the generated commit list. If the file is not present, it publishes the generated notes
   alone.
4. Merge to `main`.
5. Push a tag with the same version:

   ```bash
   git tag v0.9.0 && git push origin v0.9.0
   ```

The workflow builds the universal (arm64 and x86_64) DMG on a macOS runner, and the MSI and
NSIS installers on a Windows runner. It publishes all of them in one release. It stops before
the build if the tag does not agree with the version in `tauri.conf.json`, and it publishes
nothing unless each platform built. A merge to `main` publishes nothing. Only the tag starts
the workflow.
