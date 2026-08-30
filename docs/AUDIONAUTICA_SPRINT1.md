# Audionáutica — Sprint 1

Loop Harvester + fundación de la biblioteca de audio.

## Arquitectura

```text
Ableton Live (.als)
        │  read-only
        ▼
Ableton adapter     domain (AudioAsset, Session, Project, …)
        │                    │
        ▼                    ▼
Harvest pipeline  ←→  SQLite (identidad y metadata)
        │
        ▼
Filesystem providers
  LOCAL / DROPBOX_FOLDER / GOOGLE_DRIVE_FOLDER / CUSTOM_FOLDER
        │
        ▼
Loops/<YEAR>/<CATEGORY>/AUD_….wav
```

Boundaries:

| Capa | Dónde | Depende de |
|---|---|---|
| Domain | `crates/audionautica-core/src/domain.rs` | nada de UI, Tauri ni APIs cloud |
| Ableton | `crates/audionautica-core/src/ableton.rs` | filesystem read-only |
| Storage | `crates/audionautica-core/src/storage.rs` | filesystem local |
| Database | `crates/audionautica-core/src/db.rs` | SQLite |
| Harvest | `crates/audionautica-core/src/harvest.rs` | domain + adapters |
| Shell | `apps/desktop/src-tauri` | core + Tauri 2 |
| UI | `apps/desktop/src` | IPC |

El dominio no conoce paths de Windows, Dropbox ni Google Drive. Un `AudioAsset` se identifica por `id` (UUID) y `content_hash` (BLAKE3), no por `C:\…`.

## Flujo de filesystem

1. El usuario elige un `.als`. El project root es el directorio padre.
2. Consolidate se resuelve como `<project>/Samples/Processed/Consolidate`. Si no existe al START, el snapshot queda vacío.
3. START SESSION guarda snapshot (`relative_path`, `size`, `mtime`). No se usa solo el nombre del archivo.
4. END SESSION compara el snapshot con el estado actual. Solo aparecen archivos **nuevos o modificados**.
5. El usuario clasifica. Default: `OTHER` / Otros.
6. ARCHIVE:
   - espera estabilidad (size + mtime + readable)
   - BLAKE3
   - si el hash ya existe → `DUPLICATE`, no se crea otro asset
   - copia verificada a biblioteca local
   - copia el mismo relative path a mirrors habilitados
   - **nunca** borra, mueve, renombra ni modifica el consolidate original

Relative path canónico:

```text
Loops/<YEAR>/<FolderCategoría>/<canonical_filename>
```

## Convención de filename canónico

```text
AUD_{YYYYMMDD}_{BPMTOKEN}_{PROJECT}_{CATEGORY}_{NNN}.{ext}
```

- `BPMTOKEN` = `{n}BPM` si hay `sourceSessionBpm`; si no, `BPMUNK`. Nunca se inventa un BPM.
- `PROJECT` = slug sanitizado del set (máx. 24).
- `CATEGORY` = `HARMONY | RHYTHM | TEXTURE | PERC | BASS | VOICE | FIELDFX | OTHER`
- `NNN` = contador por (año, categoría, project)

Ejemplos:

```text
AUD_20260829_126BPM_HYDRA_TEXTURE_001.wav
AUD_20260829_BPMUNK_HYDRA_OTHER_001.aiff
```

`originalFilename` se conserva siempre en metadata.

Sanitización: se eliminan `<>:"/\|?*`, `:`, controles; NFC unicode; espacios → `_`.

## Taxonomía

| Interno | UI | Carpeta |
|---|---|---|
| HARMONIES | Armonías | Armonias |
| RHYTHMS | Ritmos | Ritmos |
| TEXTURES | Texturas | Texturas |
| PERCUSSION | Percusión | Percusion |
| BASS | Bajos | Bajos |
| VOICES | Voces | Voces |
| FIELD_FX | Field / FX | Field_FX |
| OTHER | Otros | Otros |

SQLite guarda `year` + `category`. La carpeta es una proyección.

## Modelo AudioAsset (Sprint 1)

Campos persistidos: `id`, `sourceType`, `originalFilename/Path`, `canonicalFilename/Path`, `projectId`, `sessionId`, `category`, `year`, `sourceSessionBpm`, `detectedBpm` (siempre `null` en Sprint 1), timestamps, probe técnico opcional, `contentHash`, `metadata`.

Reservados para el futuro (null en Sprint 1): `participant`, `sync_group`, `timeline_offset_seconds`.

`sourceType` admite `ABLETON_CONSOLIDATE` y otros; Sprint 1 solo **produce** consolidates.

## BPM

- `sourceSessionBpm`: tempo de la sesión. Se intenta leer del `.als` (gzip XML, primer bloque `<Tempo><Manual Value>`). Read-only: el parser nunca reescribe el set.
- Si no hay confianza → el campo queda vacío y la UI muestra input manual.
- El usuario siempre puede sobrescribir.
- Harvest no se bloquea por BPM desconocido.
- `detectedBpm`: no implementado (detección desde audio).

## Deduplicación

BLAKE3 del contenido. Mismo hash ⇒ un solo `AudioAsset`. El origen no se toca. Re-ejecutar el flujo es idempotente.

## Storage providers

Sprint 1: solo filesystem.

- `LOCAL` — biblioteca canónica (obligatoria para archivar)
- `DROPBOX_FOLDER` — carpeta local de Dropbox Desktop
- `GOOGLE_DRIVE_FOLDER` — carpeta local de Google Drive for Desktop
- `CUSTOM_FOLDER` — cualquier otra carpeta

La UI dice «carpeta Dropbox / Google Drive», **no** «subido a la nube».

Estados por destino: `PENDING | COPIED | FAILED`. Un mirror fallido no revierte la copia canónica.

El trait `StorageProvider` (`put_relative`) es el contrato para futuros `DropboxApiProvider`, `GoogleDriveApiProvider`, `S3Provider`, etc.

## SQLite

Archivo: `%APPDATA%/cl.audionautica.desktop/audionautica.sqlite` (Windows) / Application Support equivalente en macOS.

Tablas: `projects`, `sessions`, `audio_assets`, `storage_locations`, `asset_storage_locations`, `harvest_events`, `app_settings`, `schema_migrations`.

WAL + foreign keys. Migrations incrementales en `db.rs`.

## Source safety

Garantías de Sprint 1 sobre `Samples/Processed/Consolidate`:

- NEVER DELETE
- NEVER MOVE
- NEVER RENAME
- NEVER MODIFY

Solo READ + COPY. Tests comprueban path y bytes idénticos después del harvest.

## Windows / macOS

- Paths vía `PathBuf`, nunca concatenación de `\` / `/`.
- Nombres de carpeta de categoría sin tildes (compatibilidad Windows).
- Caracteres prohibidos de ambos OS sanitizados en filenames canónicos.
- SQLite bundled (no depende de sqlite del sistema).
- Tauri 2 + WebView2 (Windows) / WKWebView (macOS).

## Cómo correr la app

Requisitos: Node 24+, Rust stable, MSVC Build Tools (Windows) o Xcode CLT (macOS).

```bash
cd apps/desktop
npm install
npm run tauri dev
```

Build de producción:

```bash
cd apps/desktop
npm run tauri build
```

## Cómo testear

```bash
# tests de dominio / harvest (A–K) + unitarios
cargo test --workspace

# typecheck frontend
cd apps/desktop
npm run typecheck

# lint frontend
npm run lint

# build frontend (sin empaquetar Tauri)
npm run build
```

## Limitaciones conocidas

- No hay detección de BPM/key desde audio.
- No hay OAuth ni APIs de Dropbox/Drive.
- No hay clasificador semántico; la categoría es manual (default Otros).
- Preview de audio usa el asset protocol de Tauri; depende de que el archivo canónico exista en disco.
- El parser `.als` es best-effort: el XML de Live no es una API estable.
- Un solo harvest activo a la vez.
- No hay reintento automático de mirrors fallidos.
- No hay code signing / notarization / auto-update.
