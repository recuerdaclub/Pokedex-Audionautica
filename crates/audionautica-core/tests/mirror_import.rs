use std::fs;
use std::path::{Path, PathBuf};

use audionautica_core::db;
use audionautica_core::domain::{Category, IngestType, StorageKind, StorageLocation};
use audionautica_core::fsutil::wav::write_pcm_wav;
use audionautica_core::harvest::{list_library, LibraryFilter};
use audionautica_core::library::sync_mirrors_to_local;
use audionautica_core::storage::library_relative;
use audionautica_core::domain::new_id;
use chrono::Utc;
use rusqlite::Connection;

struct Harness {
    root: PathBuf,
    conn: Connection,
}

impl Harness {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("aud-mirror-{}", new_id()));
        fs::create_dir_all(&root).unwrap();
        let conn = db::open_in_memory().unwrap();
        Self { root, conn }
    }

    fn add_local(&mut self) -> StorageLocation {
        let lib = self.root.join("Local");
        fs::create_dir_all(&lib).unwrap();
        self.add_location(StorageKind::Local, "Local", &lib)
    }

    fn add_drive(&mut self) -> StorageLocation {
        let lib = self.root.join("Drive");
        fs::create_dir_all(&lib).unwrap();
        self.add_location(StorageKind::GoogleDriveFolder, "Drive", &lib)
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

    fn wav_in_drive(&self, drive: &StorageLocation, name: &str, seed: i16) -> PathBuf {
        let year = 2026;
        let category = Category::Textures;
        let relative = library_relative(year, category, name);
        let path = Path::new(&drive.root_path).join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let samples: Vec<i16> = (0..2205).map(|i| seed.wrapping_add(i as i16)).collect();
        write_pcm_wav(&path, 44100, 1, &samples);
        path
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn imports_new_loops_from_drive_mirror_to_local() {
    let mut h = Harness::new();
    let local = h.add_local();
    let drive = h.add_drive();
    h.wav_in_drive(&drive, "loop amigo.wav", 42);

    let report = sync_mirrors_to_local(&h.conn).unwrap();
    assert_eq!(report.imported, 1);
    assert_eq!(report.local_restored, 0);

    let assets = list_library(&h.conn, &LibraryFilter::default()).unwrap();
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].ingest_type, IngestType::MirrorImport);
    assert_eq!(assets[0].canonical_filename, "loop amigo.wav");
    assert!(Path::new(&assets[0].canonical_path).is_file());
    assert!(Path::new(&local.root_path)
        .join(library_relative(2026, Category::Textures, "loop amigo.wav"))
        .is_file());

    let second = sync_mirrors_to_local(&h.conn).unwrap();
    assert_eq!(second.imported, 0);
    assert_eq!(second.local_restored, 0);
    assert_eq!(second.already_present, 1);
}

#[test]
fn restores_to_current_local_root_when_canonical_path_is_stale() {
    let mut h = Harness::new();
    let local_v1 = h.add_local();
    let drive = h.add_drive();
    let wav = h.wav_in_drive(&drive, "loop propio.wav", 77);

    let first = sync_mirrors_to_local(&h.conn).unwrap();
    assert_eq!(first.imported, 1);

    let assets = list_library(&h.conn, &LibraryFilter::default()).unwrap();
    assert_eq!(assets.len(), 1);
    let stale_path = PathBuf::from(&assets[0].canonical_path);
    assert!(stale_path.is_file());

    let local_v2_root = h.root.join("LocalV2");
    fs::create_dir_all(&local_v2_root).unwrap();
    let mut local_v2 = local_v1.clone();
    local_v2.id = local_v1.id.clone();
    local_v2.root_path = local_v2_root.to_string_lossy().to_string();
    db::upsert_storage_location(&h.conn, &local_v2).unwrap();
    fs::remove_file(&stale_path).unwrap();

    let report = sync_mirrors_to_local(&h.conn).unwrap();
    assert_eq!(report.imported, 0);
    assert_eq!(report.already_present, 0);
    assert_eq!(report.local_restored, 1);

    let expected = local_v2_root.join(library_relative(2026, Category::Textures, "loop propio.wav"));
    assert!(expected.is_file());

    let assets = list_library(&h.conn, &LibraryFilter::default()).unwrap();
    assert_eq!(assets[0].canonical_path, expected.to_string_lossy());
    assert!(Path::new(&wav).is_file());
}
