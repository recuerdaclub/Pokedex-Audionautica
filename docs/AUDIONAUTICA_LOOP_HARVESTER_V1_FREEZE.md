# Audionáutica Loop Harvester V1 — Freeze

**Status:** `AUDIONAUTICA LOOP HARVESTER V1 — NOT_FROZEN` until operator validation completes on Windows + macOS collaborator test.  
**Release version:** `1.0.0`  
**Sprint 2:** not started

---

## Architecture

```text
Ableton .als (read-only)
        ↓
Consolidate/ scan
        ↓
BLAKE3 + SQLite (AudioAsset)
        ↓
Local Library (canonical) + optional filesystem mirrors
```

- **Core:** `crates/audionautica-core` (Rust, no Tauri)
- **Shell:** `apps/desktop` (Tauri 2 + React)
- **Persistence:** SQLite (`audionautica.sqlite` in app data dir)

## Features (shipped in V1)

### Historical import

- Scan `Samples/Processed/Consolidate/` on project open
- Compare files to library by **BLAKE3** `content_hash`
- Import pipeline: stability → hash → dedup → metadata → category → copy → mirrors
- `ingest_type = HISTORICAL_IMPORT`
- Does **not** affect session snapshot delta

### Session harvest (optional)

- `START SESSION` captures snapshot baseline
- `END SESSION` discovers new/modified files only
- `ingest_type = SESSION_HARVEST`
- `archive_session` copies selected candidates

### Source safety

- **READ + COPY** only on Ableton consolidates
- Never delete, move, rename, or modify source files

### Storage

- **Local Library** (required)
- Optional **Dropbox / Google Drive / custom** folders (filesystem copy, no APIs)
- Taxonomy: `Loops/<year>/<category>/`

### Session recovery

- `abandon_active_session` + UI “Cancelar y volver al inicio” for DB/UI desync

## Data model (summary)

### `AudioAsset`

- Identity: `id` + `content_hash` (UNIQUE)
- `source_type`: `ABLETON_CONSOLIDATE`
- `ingest_type`: `SESSION_HARVEST` | `HISTORICAL_IMPORT`
- `source_session_bpm`: from Ableton set (context, not audio analysis)
- `detected_bpm`: always `null` in V1

### Categories

Armonías, Ritmos, Texturas, Percusión, Bajos, Voces, Field/FX, Otros

## Filename rules (current implementation)

Canonical library files use:

```text
AUD_{YYYYMMDD}_{BPMTOKEN}_{PROJECT}_{CATEGORY}_{NNN}.ext
```

`BPMUNK` when BPM unknown. Original Ableton filename stored in `original_filename`.

> **Note:** Product spec targets musical filenames with timestamp stripping only; that change is **not** in v1.0.0 code — documented as post-V1 enhancement.

## BPM semantics

- Read from `.als` MainTrack tempo when available
- User may override session BPM in UI
- Never invented from audio content

## Windows build

- NSIS + MSI via `npm run tauri build -- --bundles nsis,msi`
- **Signing:** UNSIGNED

## macOS build

- CI: `universal-apple-darwin` DMG, fallback `arm64` + `x86_64`
- **Signing:** AD_HOC / UNSIGNED
- **Notarization:** NOT_CONFIGURED

## Release pipeline

- `.github/workflows/release.yml`
- Tag `v1.0.0` → pre-release with installers + SHA256
- Docs: `docs/AUDIONAUTICA_RELEASE.md`

## Known limitations

- No audio BPM/key detection
- No cloud APIs
- No SonoBus / remote collab
- No project versioning / community / iPad
- No full review-state lifecycle UI (UNREVIEWED/IGNORED/re-review/delete)
- Unsigned Windows / unnotarized macOS installers

## Operator validation pending

| Phase | Status |
|---|---|
| Windows historical + session harvest | pending human re-run on installed EXE |
| macOS collaborator test | pending |

---

**Do not start Sprint 2 from this freeze.**
