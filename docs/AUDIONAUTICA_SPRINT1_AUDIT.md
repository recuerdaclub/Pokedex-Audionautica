# Audionáutica — Sprint 1 Audit

Fecha: 2026-08-29  
Repositorio: `audionautica-lab`  
Estado al auditar: git inicializado en `master`, **sin commits y sin código fuente**.

---

## 1. Arquitectura encontrada

El repositorio está esencialmente vacío:

- no hay `package.json`, `Cargo.toml`, `src/`, ni `apps/`
- no hay tests, base de datos, UI ni abstracciones de filesystem
- no hay README ni documentación previa
- el único contenido es el esqueleto de `.git/`

Conclusión: **greenfield**. No hay infraestructura que reutilizar ni duplicar.

Entorno de la máquina de desarrollo (Windows 10/11):

| Herramienta | Estado |
|---|---|
| Node.js | v24.18.0 |
| npm | 11.16.0 |
| Python | 3.14.6 |
| Rust / cargo / rustup | **no instalados** |
| Visual Studio Build Tools 2026 | instalado pero **incompleto** (`isComplete: false`, instalación cancelada). Workload C++ no presente. |
| Windows SDK | presente (`10.0.26100.0`) |
| Chocolatey / winget | disponibles |

El Sprint 1 requiere toolchain nativo (Tauri 2 + Rust). La implementación incluye instalar `rustup` y completar MSVC C++ Build Tools. Eso no es un blocker de producto: es setup de entorno.

---

## 2. Stack elegido

Según la guía del sprint para repo vacío:

```text
Tauri 2
React
TypeScript
Rust
SQLite
```

Plataformas objetivo: **Windows** y **macOS**.  
Fuera de alcance: code signing, notarization, auto-updater, App Store.

### Capas

| Capa | Tecnología | Responsabilidad |
|---|---|---|
| Presentación | React + TypeScript + Vite | UI de sesión, harvest y biblioteca |
| Shell nativo | Tauri 2 | diálogos, ventana, IPC, paths de app data |
| Dominio | crate Rust `audionautica-core` | AudioAsset, Session, categorías, naming, harvest |
| Ableton adapter | módulo Rust read-only | localizar Project Root, leer BPM del `.als` |
| Storage adapters | filesystem providers | LOCAL / DROPBOX_FOLDER / GOOGLE_DRIVE_FOLDER / CUSTOM_FOLDER |
| Persistencia | SQLite (`rusqlite` bundled) + migrations | projects, sessions, assets, locations, events |
| Hash | BLAKE3 | identidad de contenido / deduplicación |
| Audio probe | Symphonia (WAV / AIFF / FLAC) | duration, sample rate, channels — best-effort |
| Logs | `tracing` a archivo local | eventos de harvest, sin PII innecesaria |

El dominio **no** depende de Tauri, React, Dropbox API ni Google Drive API.

---

## 3. Estructura propuesta

```text
audionautica-lab/
├── Cargo.toml                          # workspace Rust
├── docs/
│   ├── AUDIONAUTICA_SPRINT1_AUDIT.md
│   ├── AUDIONAUTICA_SPRINT1.md
│   └── AUDIONAUTICA_FUTURE_ARCHITECTURE.md
├── crates/
│   └── audionautica-core/              # dominio + adapters + tests
│       ├── Cargo.toml
│       ├── src/
│       │   ├── domain/
│       │   ├── ableton/
│       │   ├── storage/
│       │   ├── harvest/
│       │   ├── db/
│       │   ├── audio/
│       │   ├── hash.rs
│       │   ├── naming.rs
│       │   └── fsutil/
│       └── tests/                      # tests de integración A–K
└── apps/
    └── desktop/                        # Tauri 2 + React
        ├── package.json
        ├── src/                        # UI
        └── src-tauri/                  # shell: commands IPC
```

Boundaries:

```text
DOMAIN          crates/audionautica-core/src/domain
ABLETON         crates/audionautica-core/src/ableton     (read-only)
STORAGE         crates/audionautica-core/src/storage     (filesystem only)
DATABASE        crates/audionautica-core/src/db
HARVEST         crates/audionautica-core/src/harvest
TAURI           apps/desktop/src-tauri                   (thin IPC)
REACT           apps/desktop/src                         (presentation)
```

No se fuerza un monorepo JS (no hay packages npm extra). Un crate de dominio + una app desktop es suficiente para Sprint 1.

---

## 4. Riesgos cross-platform

| Riesgo | Mitigación |
|---|---|
| Separadores de path (`\` vs `/`) | `std::path::PathBuf` en todo el core; nunca concatenar strings de OS |
| Caracteres ilegales en filenames (`<>:"/\|?*`, `:`) | sanitizer único, tests con unicode, acentos y espacios |
| Long paths en Windows | paths normalizados; nombres canónicos cortos |
| Carpetas Dropbox/Drive con espacios | file dialog nativo; no asumir ASCII |
| `.als` gzip vs XML plano | detector por magic bytes; nunca reescribir el archivo |
| Consolidate folder ausente al START | snapshot vacío; no fallar |
| Archivo a medio escribir por Ableton | stability check (size + mtime + readable) |
| SQLite locking | una conexión por flujo de harvest; WAL |
| Asset protocol de Tauri para preview | scope configurable; preview best-effort |
| macOS APFS vs Windows NTFS | tests de path lógicos; CI/dev Windows ahora, macOS compatible por API std |

---

## 5. Plan de implementación

1. Instalar `rustup` (stable) y completar workload **Desktop development with C++** en Build Tools.
2. Scaffold Tauri 2 + React + TypeScript en `apps/desktop`.
3. Crear `audionautica-core` con modelo de dominio, SQLite, harvest pipeline, Ableton reader, storage providers.
4. Cubrir tests A–K en el crate (sin UI).
5. Exponer commands Tauri y construir UI oscura de sesión / harvest / library.
6. Documentar Sprint 1 y notas de arquitectura futura.
7. Correr typecheck, tests, lint y build.

Decisiones de producto ya cerradas (no requieren confirmación):

- Dropbox / Drive = carpetas locales elegidas por el usuario (sin OAuth).
- Categorías físicas en español sin tildes en el filesystem (`Armonias`, no `Armonías`).
- BPM de sesión leído del `.als` si es posible; si no, input manual; `detectedBpm = null`.
- Fuente Ableton: **solo READ + COPY**. Nunca delete/move/rename/modify.
- `sourceType` genérico desde el día 1; Sprint 1 solo produce `ABLETON_CONSOLIDATE`.
- Campos futuros opcionales en `AudioAsset` (`participant`, `sync_group`, `timeline_offset`) para no bloquear SonoBus.
