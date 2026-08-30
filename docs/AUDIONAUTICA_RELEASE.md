# Audionáutica — Release pipeline

## Overview

Releases are built by GitHub Actions (`.github/workflows/release.yml`) for **Windows x64** and **macOS** (universal DMG preferred).

## Triggers

| Trigger | Behavior |
|---|---|
| `workflow_dispatch` | Validate + build + upload CI artifacts (no GitHub Release) |
| Push tag `v*` | Validate + build + **pre-release** on GitHub with installers |

## Recommended sequence

1. Merge release commit to `main`.
2. **Run workflow manually** (`workflow_dispatch`) and confirm Windows + macOS jobs pass.
3. Create and push tag: `git tag v1.0.0 && git push origin v1.0.0`
4. Tag run publishes **Audionautica 1.0.0** as a **pre-release**.

## Local development build

```bash
cd apps/desktop
npm ci
npm run tauri build -- --bundles nsis,msi        # Windows
npm run tauri build -- --target universal-apple-darwin --bundles dmg  # macOS
```

Installers (workspace `target/release/bundle/`):

- `nsis/*-setup.exe`
- `msi/*.msi`
- `dmg/*.dmg`

Copy user-facing names into `release/v1.0.0/windows/` or `macos/` locally if needed.

## CI artifacts

After a workflow run:

- **Actions → workflow run → Artifacts**
  - `audionautica-windows-x64-v1.0.0`
  - `audionautica-macos-v1.0.0`

## GitHub Release assets

Tag `v1.0.0` attaches:

- Windows EXE + MSI
- macOS DMG(s)
- `SHA256SUMS.txt`

Release notes body: `release/v1.0.0/RELEASE_NOTES.md`

## Windows install

1. Download `Audionautica_1.0.0_x64-setup.exe` or MSI.
2. Install (unsigned — SmartScreen may warn).
3. Launch **Audionautica** from Start menu.
4. App data: `%APPDATA%\cl.audionautica.desktop\` (SQLite + logs).

Uninstall via Settings → Apps. User audio library folders are **not** removed automatically.

## macOS install (private test)

1. Download DMG (universal or arch-specific).
2. Open DMG → drag to Applications.
3. If Gatekeeper blocks: **System Settings → Privacy & Security → Open Anyway**.
4. No notarization in v1.0.0.

## Signing / notarization (future)

| Secret | Purpose |
|---|---|
| `WINDOWS_CERTIFICATE` + password | Authenticode (future) |
| Apple Developer ID + `APPLE_ID` / app-specific password | macOS signing + notarization |

**Never commit** certificates or API keys. Add as GitHub Actions secrets when available.

## Releasing v1.0.1 later

1. Bump `1.0.0` → `1.0.1` in `Cargo.toml`, `package.json`, `tauri.conf.json`, workflow `APP_VERSION`.
2. Update `release/v1.0.1/RELEASE_NOTES.md`.
3. Commit, push, `workflow_dispatch`, then tag `v1.0.1`.

## Status matrix (v1.0.0 target)

See `docs/AUDIONAUTICA_LOOP_HARVESTER_V1_FREEZE.md` for full freeze record.
