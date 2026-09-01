use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use tracing::info;

use crate::db;
use crate::domain::{
    Category, CopyStatus, DeleteFromLibraryReport, StorageDeleteResult, StorageKind,
    StorageRelocateResult, UpdateLibraryAssetReport,
};
use crate::error::{AppError, AppResult};
use crate::naming::{extension_of, normalize_library_filename_input, resolve_filename_collision};
use crate::storage::{ensure_year_taxonomy, library_relative};

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

pub fn update_library_asset(
    conn: &Connection,
    asset_id: &str,
    new_category: Option<Category>,
    new_filename: Option<String>,
) -> AppResult<UpdateLibraryAssetReport> {
    if new_category.is_none() && new_filename.is_none() {
        return Err(AppError::Other(
            "Indica un nombre nuevo, una categoría nueva, o ambos.".into(),
        ));
    }

    let asset = db::get_asset(conn, asset_id)?
        .ok_or_else(|| AppError::AssetNotFound(asset_id.to_string()))?;
    let original = PathBuf::from(&asset.original_path);

    let target_category = new_category.unwrap_or(asset.category);
    let target_filename = if let Some(raw) = new_filename {
        normalize_library_filename_input(&raw, &extension_of(&asset.canonical_filename))?
    } else {
        asset.canonical_filename.clone()
    };

    let old_relative = library_relative(asset.year, asset.category, &asset.canonical_filename);
    let new_relative = library_relative(asset.year, target_category, &target_filename);

    if asset.category == target_category && asset.canonical_filename == target_filename {
        return Ok(UpdateLibraryAssetReport {
            asset_id: asset.id,
            old_filename: asset.canonical_filename,
            new_filename: target_filename,
            old_category: asset.category,
            new_category: target_category,
            old_relative_path: old_relative.to_string_lossy().to_string(),
            new_relative_path: new_relative.to_string_lossy().to_string(),
            locations: Vec::new(),
            errors: Vec::new(),
        });
    }

    let mut taken = db::list_library_filenames_in_category(conn, asset.year, target_category)?;
    if asset.category == target_category {
        taken.remove(&asset.canonical_filename);
    }
    let final_filename = if taken.contains(&target_filename) {
        resolve_filename_collision(&target_filename, &taken)
    } else {
        target_filename
    };
    let new_relative = library_relative(asset.year, target_category, &final_filename);

    let storage_rows = db::list_asset_storage_for_asset(conn, asset_id)?;
    let mut locations = Vec::new();
    let mut errors = Vec::new();
    let mut local_moved = false;
    let mut planned_moves: Vec<(String, PathBuf, PathBuf, StorageKind, String)> = Vec::new();

    for (rec, loc) in &storage_rows {
        if rec.copy_status != CopyStatus::Copied {
            locations.push(StorageRelocateResult {
                storage_location_id: loc.id.clone(),
                kind: loc.kind,
                label: loc.label.clone(),
                old_path: String::new(),
                new_path: String::new(),
                moved: false,
                error: Some("No había copia en este destino".into()),
            });
            continue;
        }

        let root = PathBuf::from(&loc.root_path);
        ensure_year_taxonomy(&root, asset.year)?;
        let old_full = root.join(&rec.relative_path);
        let new_full = root.join(&new_relative);
        planned_moves.push((
            rec.id.clone(),
            old_full.clone(),
            new_full.clone(),
            loc.kind,
            loc.label.clone(),
        ));
    }

    let canonical = PathBuf::from(&asset.canonical_path);
    let canonical_in_planned = planned_moves
        .iter()
        .any(|(_, old, _, _, _)| is_same_path(old, &canonical));
    if canonical.exists() && !canonical_in_planned {
        if let Some((_, local_loc)) = storage_rows
            .iter()
            .find(|(_, l)| l.kind == StorageKind::Local)
        {
            ensure_year_taxonomy(Path::new(&local_loc.root_path), asset.year)?;
            let new_full = PathBuf::from(&local_loc.root_path).join(&new_relative);
            planned_moves.push((
                "canonical".into(),
                canonical.clone(),
                new_full,
                StorageKind::Local,
                "Biblioteca local (canonical)".into(),
            ));
        }
    }

    for (storage_id, old_full, new_full, kind, label) in &planned_moves {
        match try_relocate_managed_file(old_full, new_full, &original) {
            Ok(()) => {
                info!(
                    old = %old_full.display(),
                    new = %new_full.display(),
                    kind = %kind.as_str(),
                    "library copy relocated"
                );
                if *kind == StorageKind::Local {
                    local_moved = true;
                }
                locations.push(StorageRelocateResult {
                    storage_location_id: storage_id.clone(),
                    kind: *kind,
                    label: label.clone(),
                    old_path: old_full.to_string_lossy().to_string(),
                    new_path: new_full.to_string_lossy().to_string(),
                    moved: true,
                    error: None,
                });
            }
            Err(e) => {
                let msg = e.to_string();
                errors.push(format!("{label}: {msg}"));
                locations.push(StorageRelocateResult {
                    storage_location_id: storage_id.clone(),
                    kind: *kind,
                    label: label.clone(),
                    old_path: old_full.to_string_lossy().to_string(),
                    new_path: new_full.to_string_lossy().to_string(),
                    moved: false,
                    error: Some(msg),
                });
            }
        }
    }

    let had_local_copy = storage_rows.iter().any(|(r, l)| {
        l.kind == StorageKind::Local && r.copy_status == CopyStatus::Copied
    }) || canonical.exists();

    if had_local_copy && !local_moved {
        return Err(AppError::Other(
            "No se pudo mover la copia local. El asset permanece sin cambios.".into(),
        ));
    }

    let new_canonical_path = if local_moved {
        locations
            .iter()
            .find(|l| l.kind == StorageKind::Local && l.moved)
            .map(|l| l.new_path.clone())
            .unwrap_or_else(|| {
                storage_rows
                    .iter()
                    .find(|(_, l)| l.kind == StorageKind::Local)
                    .map(|(_, l)| {
                        PathBuf::from(&l.root_path)
                            .join(&new_relative)
                            .to_string_lossy()
                            .to_string()
                    })
                    .unwrap_or_else(|| asset.canonical_path.clone())
            })
    } else {
        asset.canonical_path.clone()
    };

    let tx = conn.unchecked_transaction()?;
    db::update_asset_metadata(
        &tx,
        asset_id,
        target_category,
        &final_filename,
        &new_canonical_path,
    )?;
    for (rec, _) in &storage_rows {
        if rec.copy_status != CopyStatus::Copied {
            continue;
        }
        if locations
            .iter()
            .any(|l| l.storage_location_id == rec.id && l.moved)
        {
            db::update_asset_storage_relative_path(
                &tx,
                &rec.id,
                &new_relative.to_string_lossy(),
            )?;
        }
    }
    tx.commit()?;

    Ok(UpdateLibraryAssetReport {
        asset_id: asset.id,
        old_filename: asset.canonical_filename,
        new_filename: final_filename,
        old_category: asset.category,
        new_category: target_category,
        old_relative_path: old_relative.to_string_lossy().to_string(),
        new_relative_path: new_relative.to_string_lossy().to_string(),
        locations,
        errors,
    })
}

fn try_relocate_managed_file(old: &Path, new: &Path, original: &Path) -> AppResult<()> {
    if is_same_path(old, original) {
        return Err(AppError::Other(
            "Refusing to relocate Ableton source file".into(),
        ));
    }
    if is_same_path(old, new) {
        return Ok(());
    }
    if !old.exists() {
        if new.exists() {
            return Ok(());
        }
        return Err(AppError::InvalidPath(format!(
            "No se encontró el archivo a mover: {}",
            old.display()
        )));
    }
    if let Some(parent) = new.parent() {
        fs::create_dir_all(parent).map_err(|e| AppError::from_io(e, &parent.display().to_string()))?;
    }
    if new.exists() && !is_same_path(old, new) {
        return Err(AppError::Other(format!(
            "Ya existe un archivo en el destino: {}",
            new.display()
        )));
    }
    match fs::rename(old, new) {
        Ok(()) => Ok(()),
        Err(_) => {
            fs::copy(old, new).map_err(|e| AppError::from_io(e, &new.display().to_string()))?;
            fs::remove_file(old).map_err(|e| AppError::from_io(e, &old.display().to_string()))?;
            Ok(())
        }
    }
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
