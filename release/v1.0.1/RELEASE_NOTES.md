# Audionautica 1.0.1

Patch release: new app icon + **Eliminar de biblioteca**.

## Windows

- Download `Audionautica_1.0.1_x64-setup.exe` (NSIS) or `Audionautica_1.0.1_x64_en-US.msi`.
- **Signing:** unsigned (`WINDOWS_SIGNING = UNSIGNED`).

## macOS

- `Audionautica_1.0.1_universal.dmg` (or arch-specific fallback from CI).
- **Gatekeeper:** ad-hoc/unsigned private test build.

## Changes

- New Audionautica brand icon (Windows `.exe` / `.ico`, macOS `.icns` / DMG).
- **Biblioteca → Eliminar**: removes managed copies (Local/Drive/Dropbox) + SQLite record; **never** deletes Ableton Consolidate sources.
- Mirror delete failures reported per destination.

## Known limitations

Same as 1.0.0 — no BPM detection, no cloud APIs, unsigned installers.

See `SHA256SUMS.txt` attached to this release.
