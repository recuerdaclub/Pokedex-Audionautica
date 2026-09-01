//! Sprint 1.1 operator checks against real Ableton Live 12 artifacts.
//!
//! Reads user Live projects from disk (never writes `.als` or source consolidates).
//! Harvest runs in a sandbox built from *copies* of real Live WAV files.
//! Skips when the Live library is not present (CI / other machines).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use audionautica_core::ableton::{extract_tempo, AbletonProjectReader};
use audionautica_core::db;
use audionautica_core::domain::{new_id, Category, StorageKind, StorageLocation};
use audionautica_core::fsutil::stability::StabilityConfig;
use audionautica_core::harvest::{
    archive_session, discover_candidates, end_session, list_library, start_session,
    CandidateSelection, LibraryFilter,
};
use audionautica_core::hash::hash_file;
use chrono::Utc;
use flate2::read::GzDecoder;
use rusqlite::Connection;
use std::io::Read;

fn live_root() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("AUDIONAUTICA_LIVE_ROOT") {
        let path = PathBuf::from(p);
        if path.is_dir() {
            return Some(path);
        }
    }
    let fallback = PathBuf::from(r"C:\Users\lowen\Music\Live");
    fallback.is_dir().then_some(fallback)
}

fn skip() -> bool {
    live_root().is_none()
}

fn xml_readonly(als: &Path) -> String {
    let mut file = fs::File::open(als).unwrap();
    let mut header = [0u8; 2];
    file.read_exact(&mut header).ok();
    drop(file);
    if header == [0x1f, 0x8b] {
        let file = fs::File::open(als).unwrap();
        let mut s = String::new();
        GzDecoder::new(file).read_to_string(&mut s).unwrap();
        s
    } else {
        fs::read_to_string(als).unwrap()
    }
}

fn sel(path: &Path, category: Category) -> CandidateSelection {
    CandidateSelection {
        original_path: path.to_string_lossy().to_string(),
        selected: true,
        category,
        library_filename_override: None,
    }
}

fn fast() -> StabilityConfig {
    StabilityConfig::fast_test()
}

#[test]
fn operator_real_als_resolution_and_bpm_maintrack() {
    if skip() {
        return;
    }
    let root = live_root().unwrap();

    let cases: &[(&str, f64)] = &[
        (r"2026\dillatastic26 Project\dillatastic26.als", 120.0),
        (
            r"2026\BEATS\RAPIDINCONLAMUSA Project\RAPIDINCONLAMUSA.als",
            69.0,
        ),
        (
            r"2026\ediciones\26-equinox-edit Project\26-equinox-edit.als",
            120.0,
        ),
    ];

    for (rel, expected) in cases {
        let als = root.join(rel);
        assert!(als.is_file(), "missing {}", als.display());
        let before = fs::read(&als).unwrap();
        let info = AbletonProjectReader::inspect(&als).unwrap();
        let after = fs::read(&als).unwrap();
        assert_eq!(before, after, "must not rewrite {}", als.display());
        assert_eq!(info.als_path, als);
        assert_eq!(info.project_root, als.parent().unwrap());
        assert_eq!(
            info.consolidate_dir,
            info.project_root
                .join("Samples")
                .join("Processed")
                .join("Consolidate")
        );
        assert_eq!(
            info.tempo,
            Some(*expected),
            "MainTrack BPM mismatch for {}",
            als.file_name().unwrap().to_string_lossy()
        );
        let xml = xml_readonly(&als);
        assert_eq!(extract_tempo(&xml), Some(*expected));
    }
}

#[test]
fn operator_decoy_first_tempo_is_not_transport() {
    if skip() {
        return;
    }
    let root = live_root().unwrap();
    let als = root.join(r"2026\2dia ao nuevo Project\2dia ao nuevo.als");
    if !als.is_file() {
        return;
    }
    let before = fs::read(&als).unwrap();
    let info = AbletonProjectReader::inspect(&als).unwrap();
    assert_eq!(fs::read(&als).unwrap(), before);
    // Live 12 file has an early Tempo Manual=2; transport is MainTrack=20.
    assert_eq!(info.tempo, Some(20.0));
    assert_ne!(info.tempo, Some(2.0));
}

#[test]
fn operator_unicode_project_path_inspect() {
    if skip() {
        return;
    }
    let root = live_root().unwrap();
    let mut found = None;
    if let Ok(entries) = fs::read_dir(root.join("2026")) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if e.path().is_dir() && !name.is_ascii() {
                found = Some(e.path());
                break;
            }
        }
    }
    let Some(proj) = found else { return };
    assert!(
        !proj.to_string_lossy().is_ascii(),
        "expected a non-ascii project folder"
    );
    let als = fs::read_dir(&proj)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().and_then(|e| e.to_str()) == Some("als"))
        .expect("als in unicode project");
    let before = fs::read(&als).unwrap();
    let info = AbletonProjectReader::inspect(&als).unwrap();
    assert_eq!(fs::read(&als).unwrap(), before);
    assert!(info.tempo.is_some());
    assert_eq!(info.project_root, proj);
}

fn copy_real_wavs(from: &Path, to: &Path, n: usize) -> Vec<PathBuf> {
    fs::create_dir_all(to).unwrap();
    let mut wavs: Vec<PathBuf> = fs::read_dir(from)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            matches!(
                p.extension().and_then(|e| e.to_str()).map(|s| s.to_ascii_lowercase()),
                Some(ref e) if e == "wav" || e == "aif" || e == "aiff" || e == "flac"
            )
        })
        .collect();
    wavs.sort();
    wavs.truncate(n);
    let mut out = Vec::new();
    for (i, src) in wavs.iter().enumerate() {
        let dest = to.join(format!(
            "op_{i:02}_{}",
            src.file_name().unwrap().to_string_lossy()
        ));
        fs::copy(src, &dest).unwrap();
        out.push(dest);
    }
    out
}

#[test]
fn operator_harvest_real_ableton_wavs_differential_categories_storage() {
    if skip() {
        return;
    }
    let root = live_root().unwrap();
    let als_src = root.join(r"2026\ediciones\26-equinox-edit Project\26-equinox-edit.als");
    let cons_src =
        root.join(r"2026\ediciones\26-equinox-edit Project\Samples\Processed\Consolidate");
    if !als_src.is_file() || !cons_src.is_dir() {
        return;
    }

    let sandbox = std::env::temp_dir().join(format!("aud-op11-{}", new_id()));
    let unicode_lib = sandbox.join("Audionáutica Sesión Ñ");
    let proj = sandbox.join("Operator Project");
    let cons = proj.join("Samples").join("Processed").join("Consolidate");
    fs::create_dir_all(&cons).unwrap();
    let als = proj.join("Operator.als");
    fs::copy(&als_src, &als).unwrap();
    // Original set must stay byte-identical.
    assert_eq!(fs::read(&als_src).unwrap(), fs::read(&als).unwrap());

    let copied = copy_real_wavs(&cons_src, &cons, 7);
    assert_eq!(copied.len(), 7);
    let existing = &copied[0..3];
    let news = &copied[3..7];

    // Source integrity of the *user* consolidates (never touched).
    let user_src = cons_src.join(
        fs::read_dir(&cons_src)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().and_then(|e| e.to_str()) == Some("wav"))
            .unwrap()
            .file_name()
            .unwrap(),
    );
    let user_hash_before = hash_file(&user_src).unwrap();

    let mut conn = db::open_in_memory().unwrap();
    let drive_root = PathBuf::from(r"G:\Mi unidad\AudionauticaOperatorTest");
    let drive_available = Path::new(r"G:\Mi unidad").is_dir();
    let bad = sandbox.join("not-a-dir.txt");
    fs::write(&bad, b"x").unwrap();

    // Operator START with 3 existing consolidates.
    for p in news {
        fs::remove_file(p).unwrap();
    }
    let local = upsert_loc(&conn, StorageKind::Local, "Local", &unicode_lib);
    let drive_tmp = sandbox.join("drive-mirror");
    fs::create_dir_all(&drive_tmp).unwrap();
    upsert_loc(&conn, StorageKind::GoogleDriveFolder, "Drive", &drive_tmp);
    if drive_available {
        let _ = fs::create_dir_all(&drive_root);
        upsert_loc(&conn, StorageKind::GoogleDriveFolder, "DriveG", &drive_root);
    }
    upsert_loc(&conn, StorageKind::DropboxFolder, "Dropbox", &bad);

    let als_before = fs::read(&als).unwrap();
    let session = start_session(&conn, &als, None).unwrap();
    assert_eq!(fs::read(&als).unwrap(), als_before);
    assert_eq!(session.snapshot.files.len(), 3, "existing count");

    // Immediate appearance of 4 complete Ableton WAV files.
    let t0 = Instant::now();
    for p in news {
        let name = p.file_name().unwrap();
        let orig_name = name.to_string_lossy();
        let orig_name = orig_name.splitn(3, '_').nth(2).unwrap_or(&orig_name);
        fs::copy(cons_src.join(orig_name), p).unwrap();
    }
    let appeared_ms = t0.elapsed().as_millis();

    let (_s, cands) = end_session(&conn, &session.id).unwrap();
    assert_eq!(cands.len(), 4, "new candidates; copy_ms={appeared_ms}");
    for e in existing {
        let name = e.file_name().unwrap().to_string_lossy();
        assert!(
            !cands.iter().any(|c| c.original_filename == name),
            "preexisting {} must not appear",
            name
        );
    }

    let cats = [
        Category::Harmonies,
        Category::Rhythms,
        Category::Textures,
        Category::Percussion,
    ];
    let selections: Vec<_> = news.iter().zip(cats).map(|(p, c)| sel(p, c)).collect();

    let hashes_before: Vec<_> = news.iter().map(|p| hash_file(p).unwrap()).collect();
    let report = archive_session(&mut conn, &session.id, &selections, &fast()).unwrap();
    assert_eq!(report.new_assets, 4);
    for (p, h) in news.iter().zip(&hashes_before) {
        assert_eq!(&hash_file(p).unwrap(), h);
        assert_eq!(p.file_name().unwrap(), p.file_name().unwrap());
        assert!(p.exists());
    }

    let year = Utc::now().format("%Y").to_string();
    for (folder, cat) in [
        ("Armonias", Category::Harmonies),
        ("Ritmos", Category::Rhythms),
        ("Texturas", Category::Textures),
        ("Percusion", Category::Percussion),
    ] {
        let dir = unicode_lib.join("Loops").join(&year).join(folder);
        assert!(dir.is_dir(), "missing {folder}");
        let n = fs::read_dir(&dir).unwrap().count();
        assert_eq!(n, 1, "{folder}");
        let assets = list_library(
            &conn,
            &LibraryFilter {
                category: Some(cat),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].source_session_bpm, session.source_session_bpm);
        let dest = PathBuf::from(&assets[0].canonical_path);
        let source = news.iter().zip(cats).find(|(_, c)| *c == cat).unwrap().0;
        assert_eq!(hash_file(&dest).unwrap(), hash_file(source).unwrap());
        assert!(dest.is_file());
    }

    let dropbox = report
        .storage
        .iter()
        .find(|s| s.kind == StorageKind::DropboxFolder)
        .unwrap();
    assert!(dropbox.failed >= 1);
    assert_eq!(report.new_assets, 4);

    if drive_available {
        let drive = report
            .storage
            .iter()
            .find(|s| s.kind == StorageKind::GoogleDriveFolder)
            .unwrap();
        assert_eq!(drive.copied, 4, "Drive filesystem copies");
        assert_eq!(drive.failed, 0);
    }

    let local_summary = report
        .storage
        .iter()
        .find(|s| s.storage_location_id == local.id)
        .unwrap();
    assert_eq!(local_summary.copied, 4);
    assert_eq!(local_summary.failed, 0);

    // Dedup: same bytes, new name, new session.
    let dup = cons.join("loop-copy.wav");
    fs::copy(&news[0], &dup).unwrap();
    let session2 = start_session(&conn, &als, None).unwrap();
    let report2 = archive_session(
        &mut conn,
        &session2.id,
        &[sel(&dup, Category::Other)],
        &fast(),
    )
    .unwrap();
    assert_eq!(report2.new_assets, 0);
    assert_eq!(report2.duplicates_skipped, 1);

    // Restart: no new consolidates.
    let session3 = start_session(&conn, &als, None).unwrap();
    let cands3 = discover_candidates(&session3).unwrap();
    assert!(cands3.is_empty());
    let _ = end_session(&conn, &session3.id).unwrap();
    archive_session(&mut conn, &session3.id, &[], &fast()).unwrap();

    // One genuinely new real Live wav after a fresh start.
    let session4 = start_session(&conn, &als, None).unwrap();
    let known: std::collections::HashSet<String> = list_library(&conn, &LibraryFilter::default())
        .unwrap()
        .into_iter()
        .map(|a| a.content_hash)
        .collect();
    let brand_src = fs::read_dir(&cons_src)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|s| s.eq_ignore_ascii_case("wav"))
                .unwrap_or(false)
                && hash_file(p)
                    .ok()
                    .map(|h| !known.contains(&h))
                    .unwrap_or(false)
        })
        .expect("another distinct Live wav");
    let brand_new = cons.join("brand_new_operator.wav");
    fs::copy(&brand_src, &brand_new).unwrap();
    let (_s4, cands4) = end_session(&conn, &session4.id).unwrap();
    assert_eq!(cands4.len(), 1);
    let report4 = archive_session(
        &mut conn,
        &session4.id,
        &[sel(&brand_new, Category::Other)],
        &fast(),
    )
    .unwrap();
    assert_eq!(report4.new_assets, 1);

    assert_eq!(hash_file(&user_src).unwrap(), user_hash_before);
    assert!(user_src.exists());
    fs::remove_dir_all(&sandbox).ok();
}

fn upsert_loc(conn: &Connection, kind: StorageKind, label: &str, path: &Path) -> StorageLocation {
    let loc = StorageLocation {
        id: new_id(),
        kind,
        label: label.into(),
        root_path: path.to_string_lossy().to_string(),
        enabled: true,
        created_at: Utc::now(),
    };
    db::upsert_storage_location(conn, &loc).unwrap();
    loc
}
