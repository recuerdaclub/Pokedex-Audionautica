use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use audionautica_core::db;
use audionautica_core::domain::{new_id, Category, StorageKind, StorageLocation};
use audionautica_core::fsutil::stability::StabilityConfig;
use audionautica_core::fsutil::wav::write_pcm_wav;
use audionautica_core::harvest::{import_historical, list_library, CandidateSelection, LibraryFilter};
use audionautica_core::hash::hash_file;
use audionautica_core::library::delete_from_library;
use chrono::Utc;
use flate2::write::GzEncoder;
use flate2::Compression;

fn write_als(path: &Path, bpm: &str) {
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Ableton MajorVersion="5" Creator="Ableton Live 12">
  <LiveSet><Tempo><Manual Value="{bpm}" /></Tempo></LiveSet>
</Ableton>"#
    );
    let mut enc = GzEncoder::new(File::create(path).unwrap(), Compression::default());
    enc.write_all(xml.as_bytes()).unwrap();
    enc.finish().unwrap();
}

#[test]
fn delete_from_library_removes_canonical_keeps_ableton_source() {
    let root = std::env::temp_dir().join(format!("aud-del-{}", new_id()));
    let proj = root.join("P Project");
    let cons = proj.join("Samples").join("Processed").join("Consolidate");
    fs::create_dir_all(&cons).unwrap();
    let als = proj.join("P.als");
    write_als(&als, "120");
    let mut conn = db::open_in_memory().unwrap();
    let lib = root.join("Lib");
    fs::create_dir_all(&lib).unwrap();
    db::upsert_storage_location(
        &conn,
        &StorageLocation {
            id: new_id(),
            kind: StorageKind::Local,
            label: "Local".into(),
            root_path: lib.to_string_lossy().to_string(),
            enabled: true,
            created_at: Utc::now(),
        },
    )
    .unwrap();
    let source = cons.join("loop.wav");
    write_pcm_wav(&source, 44100, 1, &(0..1000i16).collect::<Vec<_>>());
    let hash_before = hash_file(&source).unwrap();
    import_historical(
        &mut conn,
        &als,
        Some(120.0),
        &[CandidateSelection {
            original_path: source.to_string_lossy().to_string(),
            selected: true,
            category: Category::Other,
        }],
        &StabilityConfig::fast_test(),
    )
    .unwrap();
    let asset = &list_library(&conn, &LibraryFilter::default()).unwrap()[0];
    assert!(Path::new(&asset.canonical_path).is_file());
    let report = delete_from_library(&conn, &asset.id).unwrap();
    assert!(report.removed_from_db);
    assert!(report.source_preserved);
    assert!(!Path::new(&asset.canonical_path).exists());
    assert_eq!(hash_file(&source).unwrap(), hash_before);
    assert!(list_library(&conn, &LibraryFilter::default()).unwrap().is_empty());
    let _ = fs::remove_dir_all(&root);
}
