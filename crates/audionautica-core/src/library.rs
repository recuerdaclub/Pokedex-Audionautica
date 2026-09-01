use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use chrono::{DateTime, Utc};
use rusqlite::Connection;
use tracing::info;

use crate::audio::probe_file;
use crate::db;
use crate::domain::{
    AssetStorageLocation, AudioAsset, Category, CopyStatus, DeleteFromLibraryReport, IngestType,
    MirrorImportReport, Project, Session, SessionSnapshot, SessionStatus, SourceType,
    StorageDeleteResult, StorageKind, StorageRelocateResult, UpdateLibraryAssetReport, new_id,
};
use crate::error::{AppError, AppResult};
use crate::fsutil::copy::copy_verified;
use crate::hash::hash_file;
use crate::naming::{
    extension_of, is_supported_audio_filename, normalize_library_filename_input,
    resolve_filename_collision,
};
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

const SHARED_PROJECT_ROOT: &str = "__audionautica_mirror_import__";
const SHARED_PROJECT_NAME: &str = "Biblioteca compartida";

struct MirrorLibraryEntry {
    path: PathBuf,
    relative: PathBuf,
    year: i32,
    category: Category,
    filename: String,
}

/// Import loops from configured Drive/Dropbox mirrors into the local library and SQLite.
/// Skips files already present locally; only hashes files not yet tracked on the mirror.
pub fn sync_mirrors_to_local(conn: &Connection) -> AppResult<MirrorImportReport> {
    let mut report = MirrorImportReport::default();

    let locations = db::list_storage_locations(conn)?;
    let local = match locations
        .iter()
        .find(|l| l.kind == StorageKind::Local && l.enabled)
    {
        Some(local) => local,
        None => return Ok(report),
    };
    let mirrors: Vec<_> = locations
        .iter()
        .filter(|l| l.enabled && l.kind != StorageKind::Local)
        .collect();
    if mirrors.is_empty() {
        return Ok(report);
    }

    let mut import_ctx: Option<(Project, Session)> = None;
    let mut processed_hashes = HashSet::new();

    for mirror in mirrors {
        let entries = match collect_mirror_library_files(Path::new(&mirror.root_path)) {
            Ok(entries) => entries,
            Err(e) => {
                report.errors.push(format!("{}: {e}", mirror.label));
                continue;
            }
        };

        for entry in entries {
            let relative = mirror_relative_key(&entry.relative);

            if let Some(existing) =
                db::find_asset_by_mirror_relative(conn, &mirror.id, &relative)?
            {
                if !processed_hashes.insert(existing.content_hash.clone()) {
                    continue;
                }
                handle_existing_mirror_asset(
                    conn,
                    local,
                    mirror,
                    &existing,
                    &entry,
                    &mut report,
                )?;
                continue;
            }

            let content_hash = match hash_file(&entry.path) {
                Ok(hash) => hash,
                Err(e) => {
                    report.skipped += 1;
                    report.errors.push(format!("{}: {e}", entry.filename));
                    continue;
                }
            };

            if !processed_hashes.insert(content_hash.clone()) {
                continue;
            }

            if let Some(existing) = db::find_asset_by_hash(conn, &content_hash)? {
                handle_existing_mirror_asset(
                    conn,
                    local,
                    mirror,
                    &existing,
                    &entry,
                    &mut report,
                )?;
                continue;
            }

            if import_ctx.is_none() {
                import_ctx = Some(ensure_mirror_import_context(conn)?);
            }
            let (project, session) = import_ctx.as_ref().expect("mirror import context");
            import_new_mirror_asset(
                conn,
                local,
                mirror,
                project,
                session,
                &entry,
                &content_hash,
                &mut report,
            )?;
        }
    }

    if report.imported > 0 || report.local_restored > 0 {
        info!(
            imported = report.imported,
            restored = report.local_restored,
            present = report.already_present,
            "mirror import finished"
        );
    }
    Ok(report)
}

fn mirror_relative_key(relative: &Path) -> String {
    relative.to_string_lossy().replace('\\', "/")
}

fn ensure_mirror_import_context(conn: &Connection) -> AppResult<(Project, Session)> {
    let now = Utc::now();
    let project = match db::find_project_by_root(conn, SHARED_PROJECT_ROOT)? {
        Some(existing) => existing,
        None => {
            let project = Project {
                id: new_id(),
                name: SHARED_PROJECT_NAME.into(),
                ableton_set_path: String::new(),
                project_root: SHARED_PROJECT_ROOT.into(),
                created_at: now,
                updated_at: now,
            };
            db::upsert_project(conn, &project)?;
            project
        }
    };

    let session = Session {
        id: new_id(),
        project_id: project.id.clone(),
        project_name: project.name.clone(),
        ableton_set_path: String::new(),
        project_root: project.project_root.clone(),
        consolidate_dir: String::new(),
        start_time: now,
        end_time: Some(now),
        source_session_bpm: None,
        snapshot: SessionSnapshot::default(),
        status: SessionStatus::Archived,
    };
    db::insert_session(conn, &session)?;
    Ok((project, session))
}

fn collect_mirror_library_files(mirror_root: &Path) -> AppResult<Vec<MirrorLibraryEntry>> {
    let loops = mirror_root.join("Loops");
    if !loops.exists() {
        return Ok(Vec::new());
    }
    if !loops.is_dir() {
        return Err(AppError::InvalidPath(format!(
            "Se esperaba una carpeta Loops en {}",
            mirror_root.display()
        )));
    }

    let mut out = Vec::new();
    for year_entry in fs::read_dir(&loops)
        .map_err(|e| AppError::from_io(e, &loops.display().to_string()))?
    {
        let year_entry = year_entry.map_err(|e| AppError::from_io(e, &loops.display().to_string()))?;
        if !year_entry.file_type().map_err(|e| AppError::from_io(e, "Loops"))?.is_dir() {
            continue;
        }
        let year = match year_entry.file_name().to_string_lossy().parse::<i32>() {
            Ok(year) if (1900..3000).contains(&year) => year,
            _ => continue,
        };
        let year_path = year_entry.path();
        for category_entry in fs::read_dir(&year_path)
            .map_err(|e| AppError::from_io(e, &year_path.display().to_string()))?
        {
            let category_entry = category_entry
                .map_err(|e| AppError::from_io(e, &year_path.display().to_string()))?;
            if !category_entry
                .file_type()
                .map_err(|e| AppError::from_io(e, &year_path.display().to_string()))?
                .is_dir()
            {
                continue;
            }
            let category = match Category::from_folder_name(&category_entry.file_name().to_string_lossy()) {
                Some(category) => category,
                None => continue,
            };
            let category_path = category_entry.path();
            for file_entry in fs::read_dir(&category_path)
                .map_err(|e| AppError::from_io(e, &category_path.display().to_string()))?
            {
                let file_entry = file_entry
                    .map_err(|e| AppError::from_io(e, &category_path.display().to_string()))?;
                if !file_entry
                    .file_type()
                    .map_err(|e| AppError::from_io(e, &category_path.display().to_string()))?
                    .is_file()
                {
                    continue;
                }
                let filename = file_entry.file_name().to_string_lossy().to_string();
                if !is_supported_audio_filename(&filename) {
                    continue;
                }
                let relative = library_relative(year, category, &filename);
                out.push(MirrorLibraryEntry {
                    path: file_entry.path(),
                    relative,
                    year,
                    category,
                    filename,
                });
            }
        }
    }
    Ok(out)
}

fn handle_existing_mirror_asset(
    conn: &Connection,
    local: &crate::domain::StorageLocation,
    mirror: &crate::domain::StorageLocation,
    existing: &AudioAsset,
    entry: &MirrorLibraryEntry,
    report: &mut MirrorImportReport,
) -> AppResult<()> {
    let local_dest = PathBuf::from(&existing.canonical_path);
    let relative = mirror_relative_key(&entry.relative);
    let now = Utc::now();

    if local_dest.is_file() {
        report.already_present += 1;
    } else {
        if let Some(parent) = local_dest.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| AppError::from_io(e, &parent.display().to_string()))?;
        }
        match copy_verified(&entry.path, &local_dest) {
            Ok(()) => {
                report.local_restored += 1;
                info!(asset = %existing.id, path = %local_dest.display(), "restored local copy from mirror");
            }
            Err(e) => {
                report.errors.push(format!(
                    "No se pudo restaurar {} en local: {e}",
                    existing.canonical_filename
                ));
            }
        }
    }

    if local_dest.is_file() {
        db::upsert_asset_storage(
            conn,
            &AssetStorageLocation {
                id: new_id(),
                asset_id: existing.id.clone(),
                storage_location_id: local.id.clone(),
                relative_path: library_relative(existing.year, existing.category, &existing.canonical_filename)
                    .to_string_lossy()
                    .to_string(),
                copy_status: CopyStatus::Copied,
                error_message: None,
                copied_at: Some(now),
            },
        )?;
    }

    db::upsert_asset_storage(
        conn,
        &AssetStorageLocation {
            id: new_id(),
            asset_id: existing.id.clone(),
            storage_location_id: mirror.id.clone(),
            relative_path: relative,
            copy_status: CopyStatus::Copied,
            error_message: None,
            copied_at: Some(now),
        },
    )?;
    Ok(())
}

fn import_new_mirror_asset(
    conn: &Connection,
    local: &crate::domain::StorageLocation,
    mirror: &crate::domain::StorageLocation,
    project: &Project,
    session: &Session,
    entry: &MirrorLibraryEntry,
    content_hash: &str,
    report: &mut MirrorImportReport,
) -> AppResult<()> {
    let taken = db::list_library_filenames_in_category(conn, entry.year, entry.category)?;
    let canonical = resolve_filename_collision(&entry.filename, &taken);
    let relative = library_relative(entry.year, entry.category, &canonical);
    let local_path = Path::new(&local.root_path).join(&relative);
    let now = Utc::now();

    ensure_year_taxonomy(Path::new(&local.root_path), entry.year)?;
    if let Some(parent) = local_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AppError::from_io(e, &parent.display().to_string()))?;
    }

    if let Err(e) = copy_verified(&entry.path, &local_path) {
        report.errors.push(format!(
            "No se pudo copiar {} a la biblioteca local: {e}",
            entry.filename
        ));
        return Ok(());
    }

    let probe = probe_file(&entry.path);
    let source_meta = fs::metadata(&entry.path).ok();
    let size_bytes = source_meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let source_modified_at = source_meta.and_then(|m| {
        m.modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .and_then(|d| DateTime::from_timestamp_millis(d.as_millis() as i64))
    });

    let asset = AudioAsset {
        id: new_id(),
        source_type: SourceType::CommunityUpload,
        ingest_type: IngestType::MirrorImport,
        original_filename: entry.filename.clone(),
        original_path: entry.path.to_string_lossy().to_string(),
        canonical_filename: canonical.clone(),
        canonical_path: local_path.to_string_lossy().to_string(),
        project_id: project.id.clone(),
        session_id: session.id.clone(),
        category: entry.category,
        year: entry.year,
        source_session_bpm: None,
        detected_bpm: None,
        created_at: source_modified_at.unwrap_or(now),
        harvested_at: now,
        source_modified_at,
        duration_seconds: probe.duration_seconds,
        sample_rate: probe.sample_rate,
        channels: probe.channels,
        size_bytes,
        content_hash: content_hash.to_string(),
        metadata: serde_json::json!({
            "mirror_source": mirror.label,
            "ingest_type": IngestType::MirrorImport.as_str(),
        }),
        participant: None,
        sync_group: None,
        timeline_offset_seconds: None,
    };

    db::insert_asset(conn, &asset)?;
    let relative_str = mirror_relative_key(&relative);
    db::insert_asset_storage(
        conn,
        &AssetStorageLocation {
            id: new_id(),
            asset_id: asset.id.clone(),
            storage_location_id: local.id.clone(),
            relative_path: relative_str.clone(),
            copy_status: CopyStatus::Copied,
            error_message: None,
            copied_at: Some(now),
        },
    )?;
    db::upsert_asset_storage(
        conn,
        &AssetStorageLocation {
            id: new_id(),
            asset_id: asset.id.clone(),
            storage_location_id: mirror.id.clone(),
            relative_path: relative_str,
            copy_status: CopyStatus::Copied,
            error_message: None,
            copied_at: Some(now),
        },
    )?;
    report.imported += 1;
    info!(asset = %asset.id, file = %canonical, "imported loop from mirror");
    Ok(())
}
