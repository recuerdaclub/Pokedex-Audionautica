use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use audionautica_core::db;
use audionautica_core::domain::{new_id, Category, IngestType, StorageKind, StorageLocation};
use audionautica_core::fsutil::stability::StabilityConfig;
use audionautica_core::fsutil::wav::write_pcm_wav;
use audionautica_core::harvest::{
    abandon_active_session, archive_session, end_session, import_historical, scan_historical_consolidates,
    start_session, sync_mirror_from_local, CandidateSelection, LibraryFilter,
};
use audionautica_core::hash::hash_file;
use audionautica_core::harvest::list_library;
use chrono::Utc;
use flate2::write::GzEncoder;
use flate2::Compression;
use rusqlite::Connection;

struct Harness {
    root: PathBuf,
    conn: Connection,
}

impl Harness {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("aud-hist-{}", new_id()));
        fs::create_dir_all(&root).unwrap();
        let conn = db::open_in_memory().unwrap();
        Self { root, conn }
    }

    fn project(&self, name: &str, bpm: &str) -> (PathBuf, PathBuf) {
        let proj = self.root.join(format!("{name} Project"));
        let consolidate = proj.join("Samples").join("Processed").join("Consolidate");
        fs::create_dir_all(&consolidate).unwrap();
        let als = proj.join(format!("{name}.als"));
        write_als(&als, bpm);
        (als, consolidate)
    }

    fn wav(&self, dir: &Path, name: &str, seed: i16) -> PathBuf {
        let path = dir.join(name);
        let samples: Vec<i16> = (0..2205).map(|i| seed.wrapping_add(i as i16)).collect();
        write_pcm_wav(&path, 44100, 1, &samples);
        path
    }

    fn add_local(&mut self) -> StorageLocation {
        let lib = self.root.join("Audionautica");
        fs::create_dir_all(&lib).unwrap();
        self.add_location(StorageKind::Local, "Local Library", &lib)
    }

    fn add_drive(&mut self) -> StorageLocation {
        let lib = self.root.join("Drive");
        fs::create_dir_all(&lib).unwrap();
        self.add_location(StorageKind::GoogleDriveFolder, "Drive", &lib)
    }

    fn add_farm_storage(&mut self) -> StorageLocation {
        let local = self.add_local();
        self.add_drive();
        local
    }

    fn add_location(&mut self, kind: StorageKind, label: &str, path: &Path) -> StorageLocation {
        let loc = StorageLocation {
            id: new_id(),
            kind,
            label: label.into(),
            root_path: path.to_string_lossy().to_string(),
            enabled: true,
            created_at: Utc::now(),
        };
        db::upsert_storage_location(&self.conn, &loc).unwrap();
        loc
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn write_als(path: &Path, bpm: &str) {
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Ableton MajorVersion="5" Creator="Ableton Live 12">
  <LiveSet>
    <Tempo>
      <Manual Value="{bpm}" />
    </Tempo>
  </LiveSet>
</Ableton>
"#
    );
    let mut enc = GzEncoder::new(File::create(path).unwrap(), Compression::default());
    enc.write_all(xml.as_bytes()).unwrap();
    enc.finish().unwrap();
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

/// A. 3 existing unknown consolidates → historical scan finds 3
#[test]
fn a_scan_finds_unknown_consolidates() {
    let h = Harness::new();
    let (als, cons) = h.project("HYDRA", "88");
    h.wav(&cons, "A.wav", 1);
    h.wav(&cons, "B.wav", 2);
    h.wav(&cons, "C.wav", 3);
    let status = scan_historical_consolidates(&h.conn, &als).unwrap();
    assert_eq!(status.pending.len(), 3);
    assert!(!status.synced);
}

/// B. import 3 → 3 AudioAssets
#[test]
fn b_import_creates_three_assets() {
    let mut h = Harness::new();
    let (als, cons) = h.project("HYDRA", "88");
    let a = h.wav(&cons, "A.wav", 1);
    let b = h.wav(&cons, "B.wav", 2);
    let c = h.wav(&cons, "C.wav", 3);
    h.add_farm_storage();
    let report = import_historical(
        &mut h.conn,
        &als,
        None,
        &[sel(&a, Category::Other), sel(&b, Category::Other), sel(&c, Category::Other)],
        &fast(),
    )
    .unwrap();
    assert_eq!(report.new_assets, 3);
    let assets = list_library(&h.conn, &LibraryFilter::default()).unwrap();
    assert_eq!(assets.len(), 3);
    assert!(assets.iter().all(|a| a.ingest_type == IngestType::HistoricalImport));
    assert!(assets.iter().all(|a| a.detected_bpm.is_none()));
    assert_eq!(assets[0].source_session_bpm, Some(88.0));
}

/// C. rescan same project → 0 pending historical
#[test]
fn c_rescan_zero_pending_after_import() {
    let mut h = Harness::new();
    let (als, cons) = h.project("HYDRA", "88");
    let a = h.wav(&cons, "A.wav", 1);
    h.add_farm_storage();
    import_historical(&mut h.conn, &als, None, &[sel(&a, Category::Other)], &fast()).unwrap();
    let status = scan_historical_consolidates(&h.conn, &als).unwrap();
    assert!(status.pending.is_empty());
    assert!(status.synced);
}

/// D. 3 imported + 2 unknown → 2 pending
#[test]
fn d_partial_import_two_pending() {
    let mut h = Harness::new();
    let (als, cons) = h.project("HYDRA", "88");
    h.wav(&cons, "A.wav", 1);
    h.wav(&cons, "B.wav", 2);
    h.wav(&cons, "C.wav", 3);
    h.wav(&cons, "D.wav", 4);
    h.wav(&cons, "E.wav", 5);
    h.add_farm_storage();
    let paths: Vec<_> = ["A.wav", "B.wav", "C.wav"]
        .iter()
        .map(|n| cons.join(n))
        .collect();
    import_historical(
        &mut h.conn,
        &als,
        None,
        &paths
            .iter()
            .map(|p| sel(p, Category::Other))
            .collect::<Vec<_>>(),
        &fast(),
    )
    .unwrap();
    let status = scan_historical_consolidates(&h.conn, &als).unwrap();
    assert_eq!(status.pending.len(), 2);
    assert_eq!(status.pending[0].original_filename, "D.wav");
    assert_eq!(status.pending[1].original_filename, "E.wav");
}

/// E. historical import → START → add 4 → END → exactly 4 session candidates
#[test]
fn e_historical_then_session_delta_four_new() {
    let mut h = Harness::new();
    let (als, cons) = h.project("HYDRA", "88");
    h.wav(&cons, "A.wav", 1);
    h.wav(&cons, "B.wav", 2);
    h.wav(&cons, "C.wav", 3);
    h.add_farm_storage();
    let paths: Vec<_> = ["A.wav", "B.wav", "C.wav"]
        .iter()
        .map(|n| cons.join(n))
        .collect();
    import_historical(
        &mut h.conn,
        &als,
        None,
        &paths
            .iter()
            .map(|p| sel(p, Category::Other))
            .collect::<Vec<_>>(),
        &fast(),
    )
    .unwrap();

    let session = start_session(&h.conn, &als, None).unwrap();
    h.wav(&cons, "D.wav", 4);
    h.wav(&cons, "E.wav", 5);
    h.wav(&cons, "F.wav", 6);
    h.wav(&cons, "G.wav", 7);
    let (_s, cands) = end_session(&h.conn, &session.id).unwrap();
    assert_eq!(cands.len(), 4);
    assert_eq!(list_library(&h.conn, &LibraryFilter::default()).unwrap().len(), 3);
}

/// F. historical duplicate content → no duplicate AudioAsset
#[test]
fn f_cross_project_historical_dedup() {
    let mut h = Harness::new();
    let (als_a, cons_a) = h.project("ALPHA", "100");
    let (als_b, cons_b) = h.project("BETA", "110");
    let a = h.wav(&cons_a, "foo.wav", 42);
    let b = h.wav(&cons_b, "bar.wav", 42);
    assert_eq!(hash_file(&a).unwrap(), hash_file(&b).unwrap());
    h.add_farm_storage();
    let r1 = import_historical(&mut h.conn, &als_a, None, &[sel(&a, Category::Other)], &fast()).unwrap();
    assert_eq!(r1.new_assets, 1);
    let r2 = import_historical(&mut h.conn, &als_b, None, &[sel(&b, Category::Other)], &fast()).unwrap();
    assert_eq!(r2.new_assets, 0);
    assert_eq!(r2.duplicates_skipped, 1);
    assert_eq!(list_library(&h.conn, &LibraryFilter::default()).unwrap().len(), 1);
}

/// G. historical source remains byte-identical
#[test]
fn g_historical_source_safety() {
    let mut h = Harness::new();
    let (als, cons) = h.project("HYDRA", "88");
    let f = h.wav(&cons, "keepme.wav", 11);
    h.add_farm_storage();
    let hash_before = hash_file(&f).unwrap();
    let bytes_before = fs::read(&f).unwrap();
    import_historical(&mut h.conn, &als, None, &[sel(&f, Category::Other)], &fast()).unwrap();
    assert_eq!(hash_file(&f).unwrap(), hash_before);
    assert_eq!(fs::read(&f).unwrap(), bytes_before);
    assert_eq!(f.file_name().unwrap(), "keepme.wav");
}

/// H. mirror failure → canonical historical asset remains safe
#[test]
fn h_historical_mirror_failure_safe_canonical() {
    let mut h = Harness::new();
    let (als, cons) = h.project("HYDRA", "88");
    let local = h.add_local();
    let bad = h.root.join("not-a-folder.txt");
    fs::write(&bad, b"nope").unwrap();
    h.add_location(StorageKind::GoogleDriveFolder, "Drive", &bad);
    let f = h.wav(&cons, "hist.wav", 4);
    let report = import_historical(&mut h.conn, &als, None, &[sel(&f, Category::Bass)], &fast()).unwrap();
    assert_eq!(report.new_assets, 1);
    let drive_summary = report
        .storage
        .iter()
        .find(|s| s.kind == StorageKind::GoogleDriveFolder)
        .unwrap();
    assert!(drive_summary.failed >= 1);
    let assets = list_library(&h.conn, &LibraryFilter::default()).unwrap();
    assert_eq!(assets.len(), 1);
    assert!(Path::new(&assets[0].canonical_path).is_file());
    assert!(Path::new(&local.root_path).join("Loops").exists());
    assert_eq!(assets[0].ingest_type, IngestType::HistoricalImport);
}

/// Session harvest still tagged SESSION_HARVEST after refactor.
#[test]
fn session_harvest_ingest_type_preserved() {
    let mut h = Harness::new();
    let (als, cons) = h.project("HYDRA", "126");
    h.add_farm_storage();
    let session = start_session(&h.conn, &als, None).unwrap();
    let f = h.wav(&cons, "new.wav", 9);
    archive_session(&mut h.conn, &session.id, &[sel(&f, Category::Rhythms)], &fast()).unwrap();
    let assets = list_library(&h.conn, &LibraryFilter::default()).unwrap();
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].ingest_type, IngestType::SessionHarvest);
}

/// Session abandon allows restart after UI/DB desync.
#[test]
fn abandon_active_session_allows_new_start() {
    let mut h = Harness::new();
    let (als, cons) = h.project("HYDRA", "88");
    h.add_farm_storage();
    let session = start_session(&h.conn, &als, None).unwrap();
    assert!(abandon_active_session(&h.conn).unwrap());
    assert!(db::find_active_session(&h.conn).unwrap().is_none());
    let session2 = start_session(&h.conn, &als, None).unwrap();
    assert_ne!(session.id, session2.id);
    let _ = h.wav(&cons, "new.wav", 1);
    let (_s, cands) = end_session(&h.conn, &session2.id).unwrap();
    assert_eq!(cands.len(), 1);
}

#[test]
fn dropbox_added_later_backfills_library() {
    let mut h = Harness::new();
    let (als, cons) = h.project("HYDRA", "88");
    h.add_farm_storage();
    let f = h.wav(&cons, "later.wav", 3);
    import_historical(&mut h.conn, &als, None, &[sel(&f, Category::Other)], &fast()).unwrap();
    let drop_root = h.root.join("Dropbox");
    fs::create_dir_all(&drop_root).unwrap();
    let drop = h.add_location(StorageKind::DropboxFolder, "Dropbox", &drop_root);
    let copied = sync_mirror_from_local(&h.conn, &drop).unwrap();
    assert_eq!(copied, 1);
    let year = Utc::now().format("%Y").to_string();
    assert!(drop_root
        .join("Loops")
        .join(&year)
        .join("Otros")
        .join("later.wav")
        .is_file());
}
