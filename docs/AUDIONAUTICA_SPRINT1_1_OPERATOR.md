# Audionáutica Sprint 1.1 — operator notes

Paths below are redacted. Original Live sets were never rewritten.

## Project resolution (real Live 12)

| Item | Result |
|---|---|
| Ableton | Live 12 Suite (sets saved with 12.2.5 and 12.3.7) |
| ALS (tempo match) | `Live/2026/dillatastic26 Project/*.als` |
| Project root | parent of the `.als` |
| Consolidate | `<root>/Samples/Processed/Consolidate` |
| Parsed BPM | **120** (MainTrack). First XML `<Tempo>` was **0** (ignored). |
| ALS (fixed tempo) | `Live/2026/BEATS/RAPIDINCONLAMUSA Project/*.als` → **69 BPM** |
| ALS (changed/saved tempo) | `Live/2026/2dia ao nuevo Project/*.als` → **20 BPM** (decoy first Tempo was **2**) |
| Unicode project | `Live/2026/` folder with non-ASCII name; inspect OK, `.als` bytes unchanged |
| Harvest WAV source | copies of real consolidates from `Live/2026/ediciones/26-equinox-edit Project` (35+ files). User Consolidate folder not modified. |

## Parser change

Live 12 uses `<MainTrack><Tempo><Manual Value>`, not the first `<Tempo>` in the file.

## Historical consolidate import (V1)

**Status:** `AUDIONAUTICA LOOP HARVESTER V1 — NOT_FROZEN` until both historical import and real session harvest pass operator validation.

### Automated regression (`tests/historical_import.rs`)

| Case | Expectation |
|---|---|
| A | 3 unknown consolidates → scan finds 3 |
| B | import 3 → 3 `AudioAssets` with `ingest_type = HISTORICAL_IMPORT` |
| C | rescan → 0 pending |
| D | 3 imported + 2 unknown → 2 pending |
| E | historical → START → +4 → END → exactly 4 session candidates; library total 7 after archive |
| F | identical BLAKE3 across projects → one asset |
| G | source bytes unchanged |
| H | mirror failure → canonical local asset safe |

### Operator flow (manual — redo after implementation)

**PHASE A — historical**

1. Open real Ableton project in Audionáutica (choose `.als`).
2. Confirm **Library status** shows pending consolidates (not session loops).
3. **REVISAR E IMPORTAR** → categorize at least several files.
4. **IMPORTAR** → verify Local / Drive copies.
5. Preview one asset in Library.

**PHASE B — session**

1. **START SESSION** (allowed even if historical pending was skipped).
2. Create exactly **4** new Consolidates in Ableton.
3. **END SESSION** → exactly **4 NEW LOOPS**.
4. Categorize and archive.

**Expected:** historical material preserved + new session material harvested + no duplicates.

