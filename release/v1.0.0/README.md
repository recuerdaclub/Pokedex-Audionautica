# Audionautica v1.0.0 — release artifacts

Installers are **not** committed to Git. They are produced by:

- local `npm run tauri build` (Windows), or
- GitHub Actions workflow `.github/workflows/release.yml`

## Layout

```text
release/v1.0.0/
├── README.md
├── RELEASE_NOTES.md
├── windows/          (local copies after Windows build)
├── macos/            (local copies after macOS build)
└── SHA256SUMS.txt    (checksums for final files)
```

## Expected filenames

| Platform | File |
|---|---|
| Windows | `Audionautica_1.0.0_x64-setup.exe` |
| Windows | `Audionautica_1.0.0_x64_en-US.msi` |
| macOS | `Audionautica_1.0.0_universal.dmg` (or `_arm64` / `_x86_64` fallback) |

Official distribution: **GitHub Release** assets for tag `v1.0.0`.
