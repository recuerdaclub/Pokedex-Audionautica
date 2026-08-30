# Audionautica 1.0.0

Private pre-release of **Audionautica Loop Harvester V1**.

## Windows

- Download `Audionautica_1.0.0_x64-setup.exe` (NSIS) or `Audionautica_1.0.0_x64_en-US.msi`.
- Run the installer and launch **Audionautica** from the Start menu.
- **Signing:** unsigned (`WINDOWS_SIGNING = UNSIGNED`). SmartScreen may warn — choose *More info → Run anyway* for private testing.

## macOS

- Download the DMG for your Mac:
  - **Universal** `Audionautica_1.0.0_universal.dmg` when available
  - **Apple Silicon** `Audionautica_1.0.0_arm64.dmg`
  - **Intel** `Audionautica_1.0.0_x86_64.dmg`
- Open the DMG → drag **Audionautica.app** to **Applications**.
- **Gatekeeper:** private test build is **ad-hoc or unsigned** (`MACOS_SIGNING = AD_HOC or UNSIGNED`). If blocked: **System Settings → Privacy & Security → Open Anyway**.
- **Notarization:** not configured for v1.0.0.

## Installation (first run)

1. Configure **Local Library** folder.
2. Optionally add **Google Drive** / **Dropbox** sync folders (filesystem copy only).
3. Choose an Ableton `.als`.
4. Review **Library status** for existing Consolidates (historical import).
5. Optionally **START SESSION** for live differential harvest.

## V1 features

- Ableton `Samples/Processed/Consolidate/` discovery
- Post-session / historical scan by BLAKE3 content hash
- Pre-import categorization
- Session harvest (START → END → archive delta only)
- Local library + filesystem mirrors (Dropbox/Drive folders)
- Source safety: read, hash, copy — never delete/move/rename Ableton sources
- Historical import (`ingest_type = HISTORICAL_IMPORT`) vs session harvest

## Known limitations

- No BPM detection from audio (uses Ableton set BPM as context only)
- No key detection
- No Dropbox / Google Drive APIs (folder copy only)
- No cloud upload confirmation
- No SonoBus / remote jam
- No project versioning
- No community / iPad app
- Windows & macOS installers unsigned / unnotarized for private collaborator testing

## MAC TEST INSTRUCTIONS (collaborator)

1. Download the correct DMG (universal or arch-specific).
2. Install to Applications.
3. Bypass Gatekeeper if required (Privacy & Security → Open Anyway).
4. Launch → select `.als` → scan consolidates → import → close/reopen → verify persistence.
5. No terminal or Cursor required.

## Checksums

See `SHA256SUMS.txt` attached to this release.
