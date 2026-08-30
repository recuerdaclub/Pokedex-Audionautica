# Audionáutica V1 — GitHub Release Audit

**Date:** 2026-08-29  
**Auditor:** release automation (Cursor agent)

## Repository state (before release commit)

| Item | Value |
|---|---|
| Branch | `master` (no commits yet at audit start) |
| Remote | **none configured** at audit start |
| Working tree | Untracked source (`apps/`, `crates/`, `docs/`, `Cargo.toml`, `.gitignore`) |
| GitHub account (CLI) | `recuerdaclub` (authenticated) |
| Existing Audionautica repo | **not found** — will create `recuerdaclub/audionautica-lab` |
| GitHub Actions | **none** before this release |
| Protected branch | unknown (new repo) |

## Application versions (updated to 1.0.0)

| Location | Version |
|---|---|
| `Cargo.toml` workspace | `1.0.0` |
| `apps/desktop/package.json` | `1.0.0` |
| `apps/desktop/src-tauri/tauri.conf.json` | `1.0.0` |
| `productName` | `Audionautica` |
| Window title | `Audionautica` |

## Stack

| Component | Version / notes |
|---|---|
| Tauri | 2.x (`tauri-cli 2.11.4` locally) |
| React | 18.3 |
| Node | 20 (CI target) |
| Rust | stable (2021 edition) |
| Package manager | npm + `package-lock.json` |
| Rust lockfile | `Cargo.lock` committed |

## Workspace layout

```text
audionautica-lab/
├── crates/audionautica-core/   # domain, harvest, SQLite, Ableton adapter
├── apps/desktop/               # React + Tauri shell
├── docs/
├── .github/workflows/release.yml
└── release/v1.0.0/             # metadata only (no binaries in git)
```

## Bundle configuration

| Platform | Target | Status |
|---|---|---|
| Windows x64 | NSIS `.exe` | configured (`targets: ["nsis","msi"]`) |
| Windows x64 | MSI | configured |
| macOS | `universal-apple-darwin` DMG | CI with arch fallback |
| Icons | `apps/desktop/src-tauri/icons/*` | present (ico, icns, png) |

## Local regression (2026-08-29)

| Check | Result |
|---|---|
| `cargo test --workspace` | **PASS** (54 tests incl. operator) |
| `cargo clippy --workspace --all-targets -- -D warnings` | **PASS** |
| `npm ci` | **PASS** (after node_modules refresh) |
| `npm run typecheck` | **PASS** |
| `npm run lint` | **PASS** |
| `npm run build` | **PASS** |
| `npm run tauri build -- --bundles nsis,msi` | **in progress** (Windows) |

## Signing

| Platform | Status |
|---|---|
| Windows | `UNSIGNED` — no Authenticode certificate |
| macOS | `AD_HOC` or `UNSIGNED` — no Developer ID / notarization secrets |

## Product scope preserved (V1)

- Historical consolidate import (`HISTORICAL_IMPORT`)
- Optional live session differential harvest (`SESSION_HARVEST`)
- BLAKE3 dedup, local library + filesystem mirrors
- Source safety (read/copy only)
- Session abandon / UI reset for desync recovery

## Spec gaps (documented, not blocking private release)

The following appear in product spec but are **not** fully implemented in current codebase:

- Musical filename without `AUD_` prefix (still uses `AUD_{date}_{BPM}_…` canonical names)
- `UNREVIEWED` / `IMPORTED` / `IGNORED` review states
- Pre-import audio preview in scan UI
- Library actions: REPRODUCIR / MOSTRAR ORIGEN / REVISAR NUEVAMENTE / ELIMINAR DE BIBLIOTECA

These are **not** introduced in this release commit to avoid scope creep; freeze doc records current behavior.

## Release plan

1. Add workflow + docs + version `1.0.0`
2. Initial commit → push to new private GitHub repo `audionautica-lab`
3. Run `workflow_dispatch` on `main` — validate Windows + macOS builds
4. On green CI: push tag `v1.0.0`
5. Tag triggers `publish-release` → pre-release with installers + `SHA256SUMS.txt`
6. Collaborator macOS operator test (human)

## Artifacts naming

- `Audionautica_1.0.0_x64-setup.exe`
- `Audionautica_1.0.0_x64_en-US.msi`
- `Audionautica_1.0.0_universal.dmg` (or arch-specific fallback)
