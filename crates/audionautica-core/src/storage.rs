use std::path::{Path, PathBuf};

use crate::domain::{Category, StorageKind, StorageLocation};
use crate::error::{AppError, AppResult};
use crate::fsutil::copy::copy_verified;
use crate::fsutil::paths::join_library_relative;

/// Filesystem-backed storage provider.
///
/// Sprint 1 only implements local folders. Dropbox / Google Drive are
/// user-selected synced folders — Audionáutica writes files, the desktop
/// clients upload them. Future API providers (DropboxApi, DriveApi, S3, R2)
/// should implement the same `put_relative` contract without changing domain.
pub trait StorageProvider: Send + Sync {
    fn kind(&self) -> StorageKind;
    fn label(&self) -> &str;
    fn location_id(&self) -> &str;
    fn root(&self) -> &Path;
    fn put_relative(&self, relative: &Path, source: &Path) -> AppResult<PathBuf>;
}

pub struct FilesystemProvider {
    pub location: StorageLocation,
}

impl FilesystemProvider {
    pub fn new(location: StorageLocation) -> Self {
        Self { location }
    }
}

impl StorageProvider for FilesystemProvider {
    fn kind(&self) -> StorageKind {
        self.location.kind
    }

    fn label(&self) -> &str {
        &self.location.label
    }

    fn location_id(&self) -> &str {
        &self.location.id
    }

    fn root(&self) -> &Path {
        Path::new(&self.location.root_path)
    }

    fn put_relative(&self, relative: &Path, source: &Path) -> AppResult<PathBuf> {
        let root = self.root();
        if !root.exists() {
            std::fs::create_dir_all(root).map_err(|e| {
                AppError::from_io(e, &format!("crear {}", self.location.kind.label_es()))
            })?;
        }
        if !root.is_dir() {
            return Err(AppError::DestinationUnavailable(format!(
                "{} ({})",
                self.location.kind.label_es(),
                root.display()
            )));
        }
        let dest = root.join(relative);
        copy_verified(source, &dest)?;
        Ok(dest)
    }
}

pub fn library_relative(year: i32, category: Category, filename: &str) -> PathBuf {
    join_library_relative(year, category.folder_name(), filename)
}

pub fn ensure_year_taxonomy(root: &Path, year: i32) -> AppResult<()> {
    let year_dir = root.join("Loops").join(year.to_string());
    for cat in Category::ALL {
        let dir = year_dir.join(cat.folder_name());
        std::fs::create_dir_all(&dir)
            .map_err(|e| AppError::from_io(e, "crear taxonomía de biblioteca"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::new_id;
    use chrono::Utc;

    fn loc(root: &Path) -> StorageLocation {
        StorageLocation {
            id: new_id(),
            kind: StorageKind::Local,
            label: "Local".into(),
            root_path: root.display().to_string(),
            enabled: true,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn creates_year_category_folders() {
        let dir = std::env::temp_dir().join(format!("aud-lib-{}", uuid::Uuid::new_v4()));
        ensure_year_taxonomy(&dir, 2026).unwrap();
        assert!(dir.join("Loops").join("2026").join("Texturas").is_dir());
        assert!(dir.join("Loops").join("2026").join("Ritmos").is_dir());
        assert!(dir.join("Loops").join("2026").join("Armonias").is_dir());
        assert!(dir.join("Loops").join("2026").join("Otros").is_dir());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn provider_copies_under_relative_path() {
        let dir = std::env::temp_dir().join(format!("aud-prov-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src.wav");
        std::fs::write(&src, b"abc").unwrap();
        let lib = dir.join("lib");
        let p = FilesystemProvider::new(loc(&lib));
        let rel = library_relative(2026, Category::Textures, "AUD_X.wav");
        let dest = p.put_relative(&rel, &src).unwrap();
        assert!(dest.ends_with(
            Path::new("Loops")
                .join("2026")
                .join("Texturas")
                .join("AUD_X.wav")
        ));
        assert_eq!(std::fs::read(&dest).unwrap(), b"abc");
        std::fs::remove_dir_all(&dir).ok();
    }
}
