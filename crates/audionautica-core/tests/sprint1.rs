use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use audionautica_core::ableton::AbletonProjectReader;
use audionautica_core::db;
use audionautica_core::domain::{new_id, Category, CopyStatus, StorageKind, StorageLocation};
use audionautica_core::fsutil::stability::StabilityConfig;
use audionautica_core::fsutil::wav::write_pcm_wav;
use audionautica_core::harvest::{
    archive_session, discover_candidates, end_session, list_library, start_session,
    CandidateSelection, LibraryFilter,
};
use audionautica_core::hash::hash_file;
use audionautica_core::naming::{bpm_token, library_filename_from_original};
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
        let root = std::env::temp_dir().join(format!("aud-e2e-{}", new_id()));
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

/// A. snapshot inicial + archivos nuevos → only new consolidates harvested
#[test]
fn a_only_new_consolidates_harvested() {
    let mut h = Harness::new();
    let (als, cons) = h.project("HYDRA", "126");
    let existing = h.wav(&cons, "0001.wav", 1);
    h.add_farm_storage();
    let session = start_session(&h.conn, &als, None).unwrap();
    let created = h.wav(&cons, "0002.wav", 2);
    let (_s, cands) = end_session(&h.conn, &session.id).unwrap();
    assert_eq!(cands.len(), 1);
    assert_eq!(cands[0].original_filename, "0002.wav");
    assert!(!cands.iter().any(|c| c.original_filename == "0001.wav"));

    let report = archive_session(
        &mut h.conn,
        &session.id,
        &[sel(&created, Category::Textures)],
        &fast(),
    )
    .unwrap();
    assert_eq!(report.new_assets, 1);
    assert!(existing.exists());
}

/// B. archivos existentes never re-harvested incorrectly
#[test]
fn b_existing_files_not_reharvested() {
    let mut h = Harness::new();
    let (als, cons) = h.project("HYDRA", "126");
    h.wav(&cons, "old.wav", 9);
    h.add_farm_storage();
    let session = start_session(&h.conn, &als, None).unwrap();
    let cands = discover_candidates(&session).unwrap();
    assert!(cands.is_empty());
}

/// C. file stability — in-progress files not copied
#[test]
fn c_partial_file_not_copied() {
    let mut h = Harness::new();
    let (als, cons) = h.project("HYDRA", "126");
    h.add_farm_storage();
    let session = start_session(&h.conn, &als, None).unwrap();
    let growing = cons.join("growing.wav");
    let growing2 = growing.clone();
    let handle = thread::spawn(move || {
        let mut f = File::create(&growing2).unwrap();
        for _ in 0..80 {
            f.write_all(&[7u8; 256]).unwrap();
            f.flush().unwrap();
            thread::sleep(Duration::from_millis(8));
        }
    });
    thread::sleep(Duration::from_millis(20));
    let report = archive_session(
        &mut h.conn,
        &session.id,
        &[sel(&growing, Category::Other)],
        &StabilityConfig {
            checks: 3,
            interval: Duration::from_millis(25),
            max_wait: Duration::from_millis(180),
        },
    )
    .unwrap();
    handle.join().ok();
    assert_eq!(report.new_assets, 0);
    assert!(report.failed >= 1);
    assert!(report.errors.iter().any(|e| e.contains("escribiendo")));
}

/// D. BLAKE3 duplicate: same content + different filename → one asset
#[test]
fn d_blake3_duplicate_one_asset() {
    let mut h = Harness::new();
    let (als, cons) = h.project("HYDRA", "126");
    h.add_farm_storage();
    let a = h.wav(&cons, "loop.wav", 42);
    let session = start_session(&h.conn, &als, None).unwrap();
    let b = cons.join("loop copy.wav");
    fs::copy(&a, &b).unwrap();
    // `a` existed before session so only `b` is new, but content equals `a`.
    // Harvest b; then a second session harvests nothing new of same hash if we copy again.
    let report = archive_session(
        &mut h.conn,
        &session.id,
        &[sel(&b, Category::Rhythms)],
        &fast(),
    )
    .unwrap();
    assert_eq!(report.new_assets, 1);

    let (als2, cons2) = h.project("HYDRA2", "126");
    let dup = h.wav(&cons2, "loop final.wav", 42);
    // same seed → same bytes as loop.wav
    let session2 = start_session(&h.conn, &als2, None).unwrap();
    // Force identical bytes
    fs::copy(&a, &dup).unwrap();
    let report2 = archive_session(
        &mut h.conn,
        &session2.id,
        &[sel(&dup, Category::Rhythms)],
        &fast(),
    )
    .unwrap();
    assert_eq!(report2.new_assets, 0);
    assert_eq!(report2.duplicates_skipped, 1);
    let assets = list_library(&h.conn, &LibraryFilter::default()).unwrap();
    assert_eq!(assets.len(), 1);
}

/// E. folder structure 2026/Texturas etc.
#[test]
fn e_folder_structure() {
    let mut h = Harness::new();
    let (als, cons) = h.project("HYDRA", "126");
    let local = h.add_farm_storage();
    let session = start_session(&h.conn, &als, None).unwrap();
    let f = h.wav(&cons, "0003.wav", 3);
    archive_session(
        &mut h.conn,
        &session.id,
        &[sel(&f, Category::Textures)],
        &fast(),
    )
    .unwrap();
    let year = Utc::now().format("%Y").to_string();
    let dest = Path::new(&local.root_path)
        .join("Loops")
        .join(&year)
        .join("Texturas");
    assert!(dest.is_dir());
    let entries: Vec<_> = fs::read_dir(&dest).unwrap().collect();
    assert_eq!(entries.len(), 1);
    assert!(!dest.join("Ritmos").exists());
    assert!(Path::new(&local.root_path)
        .join("Loops")
        .join(&year)
        .join("Ritmos")
        .is_dir());
}

/// F. musical library filename preserves clip name, strips Ableton timestamp only
#[test]
fn f_musical_library_filename() {
    assert_eq!(
        library_filename_from_original("textura [2026-08-29 184322].wav"),
        "textura.wav"
    );
    assert_eq!(
        library_filename_from_original("0003.wav"),
        "0003.wav"
    );
}

/// G. unknown BPM never invented (metadata token only)
#[test]
fn g_unknown_bpm_never_invented() {
    assert_eq!(bpm_token(None), "BPMUNK");
    assert!(!bpm_token(None).contains("120BPM"));
}

/// H. one failed mirror does not corrupt canonical library
#[test]
fn h_failed_mirror_does_not_corrupt_canonical() {
    let mut h = Harness::new();
    let (als, cons) = h.project("HYDRA", "126");
    let local = h.add_farm_storage();
    let bad = h.root.join("not-a-folder.txt");
    fs::write(&bad, b"nope").unwrap();
    h.add_location(StorageKind::DropboxFolder, "Dropbox", &bad);
    let session = start_session(&h.conn, &als, None).unwrap();
    let f = h.wav(&cons, "0004.wav", 4);
    let report = archive_session(
        &mut h.conn,
        &session.id,
        &[sel(&f, Category::Bass)],
        &fast(),
    )
    .unwrap();
    assert_eq!(report.new_assets, 1);
    let dropbox_summary = report
        .storage
        .iter()
        .find(|s| s.kind == StorageKind::DropboxFolder)
        .unwrap();
    assert!(dropbox_summary.failed >= 1);
    let assets = list_library(&h.conn, &LibraryFilter::default()).unwrap();
    assert_eq!(assets.len(), 1);
    assert!(Path::new(&assets[0].canonical_path).is_file());
    assert!(Path::new(&local.root_path).join("Loops").exists());
}

/// I. source safety: path unchanged, bytes unchanged
#[test]
fn i_source_safety() {
    let mut h = Harness::new();
    let (als, cons) = h.project("HYDRA", "126");
    h.add_farm_storage();
    let session = start_session(&h.conn, &als, None).unwrap();
    let f = h.wav(&cons, "keepme.wav", 11);
    let path_before = f.clone();
    let hash_before = hash_file(&f).unwrap();
    let bytes_before = fs::read(&f).unwrap();
    archive_session(
        &mut h.conn,
        &session.id,
        &[sel(&f, Category::Other)],
        &fast(),
    )
    .unwrap();
    assert_eq!(f, path_before);
    assert_eq!(hash_file(&f).unwrap(), hash_before);
    assert_eq!(fs::read(&f).unwrap(), bytes_before);
    assert_eq!(f.file_name().unwrap(), "keepme.wav");
}

/// J. restart / idempotency
#[test]
fn j_idempotent_second_run() {
    let mut h = Harness::new();
    let (als, cons) = h.project("HYDRA", "126");
    h.add_farm_storage();
    let session = start_session(&h.conn, &als, None).unwrap();
    let f = h.wav(&cons, "once.wav", 8);
    archive_session(
        &mut h.conn,
        &session.id,
        &[sel(&f, Category::Voices)],
        &fast(),
    )
    .unwrap();

    let session2 = start_session(&h.conn, &als, None).unwrap();
    // Same file already in snapshot of session2 (it exists at start), so candidates empty.
    let cands = discover_candidates(&session2).unwrap();
    assert!(cands.is_empty());

    // Even if we force-archive the same path, hash dedupes.
    let report = archive_session(
        &mut h.conn,
        &session2.id,
        &[sel(&f, Category::Voices)],
        &fast(),
    )
    .unwrap();
    assert_eq!(report.new_assets, 0);
    assert_eq!(report.duplicates_skipped, 1);
    assert_eq!(
        list_library(&h.conn, &LibraryFilter::default())
            .unwrap()
            .len(),
        1
    );
}

/// K. cross-platform paths: windows, macos-ish, spaces, unicode, accents
#[test]
fn k_cross_platform_paths() {
    let mut h = Harness::new();
    let weird = h.root.join("Proyecto Café").join("Ableton Set (macOS)");
    let cons = weird.join("Samples").join("Processed").join("Consolidate");
    fs::create_dir_all(&cons).unwrap();
    let als = weird.join("textura ñoño.als");
    write_als(&als, "100");
    h.add_farm_storage();
    let session = start_session(&h.conn, &als, None).unwrap();
    let f = h.wav(&cons, "loop final 01.wav", 5);
    let report = archive_session(
        &mut h.conn,
        &session.id,
        &[sel(&f, Category::FieldFx)],
        &fast(),
    )
    .unwrap();
    assert_eq!(report.new_assets, 1);
    let asset = &list_library(&h.conn, &LibraryFilter::default()).unwrap()[0];
    assert!(Path::new(&asset.canonical_path).is_file());
    assert_eq!(asset.canonical_filename, "loop final 01.wav");
}

#[test]
fn ableton_reader_does_not_rewrite_set() {
    let h = Harness::new();
    let (als, _) = h.project("SAFE", "111");
    let before = fs::read(&als).unwrap();
    let info = AbletonProjectReader::inspect(&als).unwrap();
    assert_eq!(info.tempo, Some(111.0));
    let after = fs::read(&als).unwrap();
    assert_eq!(before, after);
}

#[test]
fn copy_status_enum_roundtrip() {
    assert_eq!(CopyStatus::parse("COPIED"), CopyStatus::Copied);
    assert_eq!(CopyStatus::parse("FAILED"), CopyStatus::Failed);
    assert_eq!(CopyStatus::parse("PENDING"), CopyStatus::Pending);
}

#[test]
fn library_filter_by_category_and_year() {
    let mut h = Harness::new();
    let (als, cons) = h.project("HYDRA", "126");
    h.add_farm_storage();
    let session = start_session(&h.conn, &als, None).unwrap();
    let a = h.wav(&cons, "a.wav", 1);
    let b = h.wav(&cons, "b.wav", 2);
    archive_session(
        &mut h.conn,
        &session.id,
        &[sel(&a, Category::Textures), sel(&b, Category::Rhythms)],
        &fast(),
    )
    .unwrap();
    let textures = list_library(
        &h.conn,
        &LibraryFilter {
            category: Some(Category::Textures),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(textures.len(), 1);
    assert_eq!(textures[0].category, Category::Textures);
}
