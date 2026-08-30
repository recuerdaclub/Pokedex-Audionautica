use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use tracing::info;

use crate::db;
use crate::domain::{CopyStatus, DeleteFromLibraryReport, StorageDeleteResult, StorageKind};
use crate::error::{AppError, AppResult};

pub fn delete_from_library(conn: &Connection, asset_id: &str) -> AppResult<DeleteFromLibraryReport> {
    let asset = db::get_asset(conn, asset_id)?
        .ok_or_else(|| AppError::AssetNotFound(asset_id.to_string()))?;
    let original = PathBuf::from(&asset.original_path);
    let storage_rows = db::list_asset_storage_for_asset(conn, asset_id)?;
    let mut locations = Vec::new();
    let mut errors = Vec::new();
    let mut local_deleted = false;

    for (rec, loc) in &storage_rows {
        if rec.copy_status != CopyStatus::Copied {
            locations.push(StorageDeleteResult {
                storage_location_id: loc.id.clone(),
                kind: loc.kind,
                label: loc.label.clone(),
                path: String::new(),
                deleted: false,
                error: Some("No había copia en este destino".into()),
            });
            continue;
        }
        let full = PathBuf::from(&loc.root_path).join(&rec.relative_path);
        match try_delete_managed_file(&full, &original) {
            Ok(()) => {
                info!(path = %full.display(), kind = %loc.kind.as_str(), "library copy removed");
                if loc.kind == StorageKind::Local {
                    local_deleted = true;
                }
                locations.push(StorageDeleteResult {
                    storage_location_id: loc.id.clone(),
                    kind: loc.kind,
                    label: loc.label.clone(),
                    path: full.to_string_lossy().to_string(),
                    deleted: true,
                    error: None,
                });
            }
            Err(e) => {
                let msg = e.to_string();
                errors.push(format!("{}: {msg}", loc.label));
                locations.push(StorageDeleteResult {
                    storage_location_id: loc.id.clone(),
                    kind: loc.kind,
                    label: loc.label.clone(),
                    path: full.to_string_lossy().to_string(),
                    deleted: false,
                    error: Some(msg),
                });
            }
        }
    }

    if !local_deleted {
        let canonical = PathBuf::from(&asset.canonical_path);
        if canonical.exists() {
            match try_delete_managed_file(&canonical, &original) {
                Ok(()) => {
                    local_deleted = true;
                    locations.push(StorageDeleteResult {
                        storage_location_id: "canonical".into(),
                        kind: StorageKind::Local,
                        label: "Biblioteca local (canonical)".into(),
                        path: canonical.to_string_lossy().to_string(),
                        deleted: true,
                        error: None,
                    });
                }
                Err(e) => {
                    errors.push(format!("Biblioteca local: {e}"));
                }
            }
        }
    }

    if !local_deleted && storage_rows.iter().any(|(r, l)| {
        l.kind == StorageKind::Local && r.copy_status == CopyStatus::Copied
    }) {
        return Err(AppError::Other(
            "No se pudo eliminar la copia local. El asset permanece en la biblioteca.".into(),
        ));
    }

    db::delete_asset(conn, asset_id)?;

    Ok(DeleteFromLibraryReport {
        asset_id: asset.id,
        canonical_filename: asset.canonical_filename,
        removed_from_db: true,
        source_preserved: true,
        locations,
        errors,
    })
}

fn try_delete_managed_file(path: &Path, original: &Path) -> AppResult<()> {
    if is_same_path(path, original) {
        return Err(AppError::Other(
            "Refusing to delete Ableton source file".into(),
        ));
    }
    if !path.exists() {
        return Ok(());
    }
    if path.is_dir() {
        return Err(AppError::InvalidPath(format!(
            "Se esperaba un archivo, no una carpeta: {}",
            path.display()
        )));
    }
    fs::remove_file(path).map_err(|e| AppError::from_io(e, &path.display().to_string()))
}

fn is_same_path(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    if let (Ok(ca), Ok(cb)) = (a.canonicalize(), b.canonicalize()) {
        return ca == cb;
    }
    normalize_path(a) == normalize_path(b)
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_lowercase()
}
