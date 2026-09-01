use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use audionautica_core::db;
use audionautica_core::domain::{new_id, Category, StorageKind, StorageLocation};
use audionautica_core::fsutil::stability::StabilityConfig;
use audionautica_core::fsutil::wav::write_pcm_wav;
use audionautica_core::harvest::{import_historical, list_library, CandidateSelection, LibraryFilter};
use audionautica_core::library::update_library_asset;
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
fn update_library_asset_renames_and_recategorizes() {
    let root = std::env::temp_dir().join(format!("aud-upd-{}", new_id()));
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
    let drive = root.join("Drive");
    fs::create_dir_all(&drive).unwrap();
    db::upsert_storage_location(
        &conn,
        &StorageLocation {
            id: new_id(),
            kind: StorageKind::GoogleDriveFolder,
            label: "Drive".into(),
            root_path: drive.to_string_lossy().to_string(),
            enabled: true,
            created_at: Utc::now(),
        },
    )
    .unwrap();
    let source = cons.join("loop.wav");
    write_pcm_wav(&source, 44100, 1, &(0..1000i16).collect::<Vec<_>>());
    import_historical(
        &mut conn,
        &als,
        Some(120.0),
        &[CandidateSelection {
            original_path: source.to_string_lossy().to_string(),
            selected: true,
            category: Category::Other,
            library_filename_override: None,
        }],
        &StabilityConfig::fast_test(),
    )
    .unwrap();
    let asset = &list_library(&conn, &LibraryFilter::default()).unwrap()[0];
    let old_local = Path::new(&asset.canonical_path);
    assert!(old_local.is_file());

    let report = update_library_asset(
        &conn,
        &asset.id,
        Some(Category::Textures),
        Some("textura renombrada.wav".into()),
    )
    .unwrap();
    assert_eq!(report.new_category, Category::Textures);
    assert_eq!(report.new_filename, "textura renombrada.wav");
    assert!(!old_local.exists());

    let updated = db::get_asset(&conn, &asset.id).unwrap().unwrap();
    assert_eq!(updated.category, Category::Textures);
    assert_eq!(updated.canonical_filename, "textura renombrada.wav");
    assert!(Path::new(&updated.canonical_path).is_file());
    assert!(updated
        .canonical_path
        .replace('\\', "/")
        .contains("Loops/"));
    assert!(updated.canonical_path.contains("Texturas"));

    let drive_copy = drive.join(report.new_relative_path.replace('/', std::path::MAIN_SEPARATOR_STR));
    assert!(drive_copy.is_file());

    let _ = fs::remove_dir_all(&root);
}
