//! Regression tests for musical filename preservation and collision handling.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use audionautica_core::db;
use audionautica_core::domain::{new_id, Category, StorageKind, StorageLocation};
use audionautica_core::fsutil::stability::StabilityConfig;
use audionautica_core::fsutil::wav::write_pcm_wav;
use audionautica_core::harvest::{import_historical, scan_historical_consolidates, CandidateSelection};
use audionautica_core::naming::{
    library_filename_from_original, resolve_filename_collision, strip_ableton_consolidate_timestamp,
};
use chrono::Utc;
use flate2::write::GzEncoder;
use flate2::Compression;
use rusqlite::Connection;
use std::collections::HashSet;

struct Harness {
    root: PathBuf,
    conn: Connection,
}

impl Harness {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("aud-fname-{}", new_id()));
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
        let loc = StorageLocation {
            id: new_id(),
            kind: StorageKind::Local,
            label: "Local Library".into(),
            root_path: lib.to_string_lossy().to_string(),
            enabled: true,
            created_at: Utc::now(),
        };
        db::upsert_storage_location(&self.conn, &loc).unwrap();
        let drive_root = self.root.join("Drive");
        fs::create_dir_all(&drive_root).unwrap();
        db::upsert_storage_location(
            &self.conn,
            &StorageLocation {
                id: new_id(),
                kind: StorageKind::GoogleDriveFolder,
                label: "Drive".into(),
                root_path: drive_root.to_string_lossy().to_string(),
                enabled: true,
                created_at: Utc::now(),
            },
        )
        .unwrap();
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
    }
}

fn fast() -> StabilityConfig {
    StabilityConfig::fast_test()
}

#[test]
fn same_cleaned_name_same_hash_dedups() {
    let mut h = Harness::new();
    let (als, cons) = h.project("HYDRA", "88");
    h.add_local();
    let wav = h.wav(&cons, "textura [2026-08-29 180000].wav", 1);
    let r1 = import_historical(&mut h.conn, &als, None, &[sel(&wav, Category::Textures)], &fast()).unwrap();
    assert_eq!(r1.new_assets, 1);
    assert_eq!(r1.assets[0].canonical_filename, "textura.wav");

    let r2 = import_historical(&mut h.conn, &als, None, &[sel(&wav, Category::Textures)], &fast()).unwrap();
    assert_eq!(r2.new_assets, 0);
    assert_eq!(r2.duplicates_skipped, 1);
}

#[test]
fn same_cleaned_name_different_hash_gets_suffix() {
    let mut h = Harness::new();
    let (als, cons) = h.project("HYDRA", "88");
    h.add_local();
    let a = h.wav(&cons, "textura [2026-08-29 180000].wav", 1);
    let b = h.wav(&cons, "textura [2026-08-29 190000].wav", 2);
    import_historical(&mut h.conn, &als, None, &[sel(&a, Category::Textures)], &fast()).unwrap();
    let r2 = import_historical(&mut h.conn, &als, None, &[sel(&b, Category::Textures)], &fast()).unwrap();
    assert_eq!(r2.new_assets, 1);
    assert_eq!(r2.assets[0].canonical_filename, "textura (2).wav");
}

#[test]
fn scan_exposes_library_filename() {
    let h = Harness::new();
    let (als, cons) = h.project("HYDRA", "88");
    h.wav(&cons, "ritmo granular [2026-08-29 191502].wav", 3);
    let status = scan_historical_consolidates(&h.conn, &als).unwrap();
    assert_eq!(status.pending.len(), 1);
    assert_eq!(status.pending[0].library_filename, "ritmo granular.wav");
    assert_eq!(
        status.pending[0].original_filename,
        "ritmo granular [2026-08-29 191502].wav"
    );
}

#[test]
fn collision_unit_cases() {
    let mut taken = HashSet::new();
    assert_eq!(
        library_filename_from_original("loop [2026-08-29 184322].wav"),
        "loop.wav"
    );
    assert_eq!(
        strip_ableton_consolidate_timestamp("grabacion 2026-08-29 final.wav"),
        "grabacion 2026-08-29 final.wav"
    );
    taken.insert("textura.wav".to_string());
    assert_eq!(resolve_filename_collision("textura.wav", &taken), "textura (2).wav");
}
