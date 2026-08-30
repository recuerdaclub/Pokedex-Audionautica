use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use chrono::{DateTime, Utc};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::ableton::{list_audio_files, AbletonProjectReader, AbletonSetInfo};
use crate::audio::probe_file;
use crate::db;
use crate::domain::{
    new_id, AssetStorageLocation, AudioAsset, Category, ChangeKind, CopyStatus, FileFingerprint,
    HarvestEvent, HistoricalConsolidate, IgnoredConsolidateInput, IngestType, Project,
    ProjectLibraryStatus, Session, SessionSnapshot, SessionStatus, SourceType, StorageKind,
    StorageLocation,
};
pub use crate::domain::{
    DuplicateSkip, HarvestCandidate, HarvestedAssetSummary, StorageCopySummary,
};
use crate::error::{AppError, AppResult};
use crate::fsutil::copy::copy_verified;
use crate::fsutil::stability::{wait_until_stable, StabilityConfig, StabilityError};
use crate::hash::hash_file;
use crate::naming::{
    is_supported_audio_filename, library_filename_from_original, resolve_filename_collision,
};
use crate::storage::{ensure_year_taxonomy, library_relative, FilesystemProvider, StorageProvider};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateSelection {
    pub original_path: String,
    pub selected: bool,
    pub category: Category,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarvestReport {
    pub session_id: String,
    pub new_assets: u32,
    pub duplicates_skipped: u32,
    pub failed: u32,
    pub assets: Vec<HarvestedAssetSummary>,
    pub duplicates: Vec<DuplicateSkip>,
    pub storage: Vec<StorageCopySummary>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LibraryFilter {
    pub year: Option<i32>,
    pub category: Option<Category>,
    pub project_id: Option<String>,
}

struct IngestContext {
    session_id: String,
    project_id: String,
    source_session_bpm: Option<f64>,
    ingest_type: IngestType,
}

pub fn inspect_set(als_path: &Path) -> AppResult<AbletonSetInfo> {
    AbletonProjectReader::inspect(als_path)
}

pub fn scan_historical_consolidates(
    conn: &Connection,
    als_path: &Path,
) -> AppResult<ProjectLibraryStatus> {
    let info = AbletonProjectReader::inspect(als_path)?;
    let project = upsert_project_from_info(conn, &info)?;
    let snapshot = scan_snapshot(&info.consolidate_dir)?;
    let known_hashes = db::all_content_hashes(conn)?;
    let ignored_hashes = db::ignored_hashes_for_project(conn, &project.id)?;
    let mut pending = Vec::new();
    let mut archived_count = 0u32;

    for file in snapshot.files {
        let original_filename = Path::new(&file.relative_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&file.relative_path)
            .to_string();
        if !is_supported_audio_filename(&original_filename) {
            continue;
        }
        let original_path = info.consolidate_dir.join(&file.relative_path);
        let content_hash = match hash_file(&original_path) {
            Ok(h) => h,
            Err(_) => continue,
        };
        if ignored_hashes.contains(&content_hash) {
            continue;
        }
        if known_hashes.contains(&content_hash) {
            archived_count += 1;
            continue;
        }
        pending.push(HistoricalConsolidate {
            original_path: original_path.to_string_lossy().to_string(),
            library_filename: library_filename_from_original(&original_filename),
            original_filename,
            relative_path: file.relative_path,
            size_bytes: file.size_bytes,
            modified_at: DateTime::from_timestamp_millis(file.modified_unix_ms)
                .unwrap_or_else(Utc::now),
            content_hash,
        });
    }
    pending.sort_by(|a, b| a.original_filename.cmp(&b.original_filename));
    let synced = pending.is_empty();

    Ok(ProjectLibraryStatus {
        project_id: project.id,
        project_name: project.name,
        consolidate_dir: info.consolidate_dir.to_string_lossy().to_string(),
        pending,
        archived_count,
        synced,
    })
}

pub fn import_historical(
    conn: &mut Connection,
    als_path: &Path,
    bpm_override: Option<f64>,
    selections: &[CandidateSelection],
    stability: &StabilityConfig,
) -> AppResult<HarvestReport> {
    let info = AbletonProjectReader::inspect(als_path)?;
    let project = upsert_project_from_info(conn, &info)?;
    let bpm = bpm_override.or(info.tempo).and_then(valid_bpm);
    let now = Utc::now();

    let session = Session {
        id: new_id(),
        project_id: project.id.clone(),
        project_name: project.name.clone(),
        ableton_set_path: info.als_path.to_string_lossy().to_string(),
        project_root: info.project_root.to_string_lossy().to_string(),
        consolidate_dir: info.consolidate_dir.to_string_lossy().to_string(),
        start_time: now,
        end_time: Some(now),
        source_session_bpm: bpm,
        snapshot: SessionSnapshot::default(),
        status: SessionStatus::Archived,
    };
    db::insert_session(conn, &session)?;
    emit(
        conn,
        &session.id,
        None,
        "historical_import_started",
        serde_json::json!({
            "project": session.project_name,
            "selected": selections.iter().filter(|s| s.selected).count(),
            "bpm": bpm,
        }),
    )?;

    let ctx = IngestContext {
        session_id: session.id.clone(),
        project_id: project.id,
        source_session_bpm: bpm,
        ingest_type: IngestType::HistoricalImport,
    };
    let mut report = ingest_selected(conn, &ctx, selections, stability)?;
    report.session_id = session.id.clone();

    emit(
        conn,
        &session.id,
        None,
        "historical_import_completed",
        serde_json::json!({
            "new_assets": report.new_assets,
            "duplicates": report.duplicates_skipped,
        }),
    )?;
    info!(
        session_id = %session.id,
        new_assets = report.new_assets,
        duplicates = report.duplicates_skipped,
        "historical import completed"
    );
    Ok(report)
}

pub fn start_session(
    conn: &Connection,
    als_path: &Path,
    bpm_override: Option<f64>,
) -> AppResult<Session> {
    if db::find_active_session(conn)?.is_some() {
        return Err(AppError::SessionAlreadyActive);
    }
    let info = AbletonProjectReader::inspect(als_path)?;
    let bpm = bpm_override.or(info.tempo).and_then(valid_bpm);
    let project = upsert_project_from_info(conn, &info)?;

    let snapshot = scan_snapshot(&info.consolidate_dir)?;
    let now = Utc::now();
    let session = Session {
        id: new_id(),
        project_id: project.id,
        project_name: project.name.clone(),
        ableton_set_path: info.als_path.to_string_lossy().to_string(),
        project_root: info.project_root.to_string_lossy().to_string(),
        consolidate_dir: info.consolidate_dir.to_string_lossy().to_string(),
        start_time: now,
        end_time: None,
        source_session_bpm: bpm,
        snapshot,
        status: SessionStatus::Active,
    };
    db::insert_session(conn, &session)?;
    emit(
        conn,
        &session.id,
        None,
        "session_started",
        serde_json::json!({
            "project": session.project_name,
            "files_in_snapshot": session.snapshot.files.len(),
            "bpm": bpm,
        }),
    )?;
    info!(
        session_id = %session.id,
        project = %session.project_name,
        snapshot_files = session.snapshot.files.len(),
        "session started"
    );
    Ok(session)
}

pub fn end_session(
    conn: &Connection,
    session_id: &str,
) -> AppResult<(Session, Vec<HarvestCandidate>)> {
    let mut session = db::get_session(conn, session_id)?
        .ok_or_else(|| AppError::SessionNotFound(session_id.to_string()))?;
    let ignored = db::ignored_hashes_for_project(conn, &session.project_id)?;
    let candidates: Vec<HarvestCandidate> = discover_candidates(&session)?
        .into_iter()
        .filter(|c| !ignored.contains(&c.content_hash))
        .collect();
    session.end_time = Some(Utc::now());
    session.status = if candidates.is_empty() {
        SessionStatus::Archived
    } else {
        SessionStatus::Review
    };
    db::update_session(conn, &session)?;
    emit(
        conn,
        &session.id,
        None,
        if candidates.is_empty() {
            "session_completed"
        } else {
            "session_review"
        },
        serde_json::json!({ "candidates": candidates.len() }),
    )?;
    info!(
        session_id = %session.id,
        candidates = candidates.len(),
        "session ended, harvest review"
    );
    Ok((session, candidates))
}

pub fn discover_candidates(session: &Session) -> AppResult<Vec<HarvestCandidate>> {
    let dir = PathBuf::from(&session.consolidate_dir);
    let current = scan_snapshot(&dir)?;
    let mut out = Vec::new();
    for file in current.files {
        let prev = session.snapshot.get(&file.relative_path);
        let kind = match prev {
            None => Some(ChangeKind::New),
            Some(old)
                if old.size_bytes != file.size_bytes
                    || old.modified_unix_ms != file.modified_unix_ms =>
            {
                Some(ChangeKind::Modified)
            }
            Some(_) => None,
        };
        if let Some(change_kind) = kind {
            let original_path = dir.join(&file.relative_path);
            let original_filename = Path::new(&file.relative_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&file.relative_path)
                .to_string();
            if !is_supported_audio_filename(&original_filename) {
                continue;
            }
            let content_hash = hash_file(&original_path).unwrap_or_default();
            if content_hash.is_empty() {
                continue;
            }
            out.push(HarvestCandidate {
                original_path: original_path.to_string_lossy().to_string(),
                library_filename: library_filename_from_original(&original_filename),
                original_filename,
                relative_path: file.relative_path,
                size_bytes: file.size_bytes,
                modified_at: DateTime::from_timestamp_millis(file.modified_unix_ms)
                    .unwrap_or_else(Utc::now),
                change_kind,
                content_hash,
            });
        }
    }
    out.sort_by(|a, b| a.original_filename.cmp(&b.original_filename));
    Ok(out)
}

pub fn set_session_bpm(
    conn: &Connection,
    session_id: &str,
    bpm: Option<f64>,
) -> AppResult<Session> {
    let mut session = db::get_session(conn, session_id)?
        .ok_or_else(|| AppError::SessionNotFound(session_id.to_string()))?;
    session.source_session_bpm = bpm.and_then(valid_bpm);
    db::update_session(conn, &session)?;
    Ok(session)
}

/// Cancel the current ACTIVE/REVIEW session so the operator can restart from zero.
/// Idempotent when no session is open (`Ok(false)`).
pub fn abandon_active_session(conn: &Connection) -> AppResult<bool> {
    let Some(mut session) = db::find_active_session(conn)? else {
        return Ok(false);
    };
    session.status = SessionStatus::Cancelled;
    session.end_time = Some(Utc::now());
    db::update_session(conn, &session)?;
    emit(
        conn,
        &session.id,
        None,
        "session_cancelled",
        serde_json::json!({ "project": session.project_name }),
    )?;
    info!(session_id = %session.id, "session cancelled by operator");
    Ok(true)
}

pub fn archive_session(
    conn: &mut Connection,
    session_id: &str,
    selections: &[CandidateSelection],
    stability: &StabilityConfig,
) -> AppResult<HarvestReport> {
    let session = db::get_session(conn, session_id)?
        .ok_or_else(|| AppError::SessionNotFound(session_id.to_string()))?;
    let ctx = IngestContext {
        session_id: session.id.clone(),
        project_id: session.project_id.clone(),
        source_session_bpm: session.source_session_bpm,
        ingest_type: IngestType::SessionHarvest,
    };
    let mut report = ingest_selected(conn, &ctx, selections, stability)?;

    let mut session = session;
    session.status = SessionStatus::Archived;
    session.end_time = Some(Utc::now());
    db::update_session(conn, &session)?;
    emit(
        conn,
        &session.id,
        None,
        "session_completed",
        serde_json::json!({
            "new_assets": report.new_assets,
            "duplicates": report.duplicates_skipped,
        }),
    )?;
    info!(
        session_id = %session.id,
        new_assets = report.new_assets,
        duplicates = report.duplicates_skipped,
        "session completed"
    );
    report.session_id = session.id;
    Ok(report)
}

fn ingest_selected(
    conn: &mut Connection,
    ctx: &IngestContext,
    selections: &[CandidateSelection],
    stability: &StabilityConfig,
) -> AppResult<HarvestReport> {
    let locations: Vec<StorageLocation> = db::list_storage_locations(conn)?
        .into_iter()
        .filter(|l| l.enabled)
        .collect();
    let local = locations
        .iter()
        .find(|l| l.kind == StorageKind::Local)
        .cloned()
        .ok_or(AppError::LocalLibraryMissing)?;
    locations
        .iter()
        .find(|l| l.kind == StorageKind::GoogleDriveFolder && l.enabled)
        .ok_or(AppError::DriveLibraryMissing)?;

    let selected: Vec<&CandidateSelection> = selections.iter().filter(|s| s.selected).collect();
    let mut report = HarvestReport {
        session_id: ctx.session_id.clone(),
        new_assets: 0,
        duplicates_skipped: 0,
        failed: 0,
        assets: Vec::new(),
        duplicates: Vec::new(),
        storage: locations
            .iter()
            .map(|l| StorageCopySummary {
                storage_location_id: l.id.clone(),
                kind: l.kind,
                label: l.label.clone(),
                copied: 0,
                failed: 0,
                total: selected.len() as u32,
            })
            .collect(),
        errors: Vec::new(),
    };

    let now = Utc::now();
    let year = now.year_for_library();
    ensure_year_taxonomy(Path::new(&local.root_path), year)?;
    let mut allocated_filenames: HashSet<String> = HashSet::new();

    for sel in &selected {
        let source = PathBuf::from(&sel.original_path);
        let filename = source
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("loop")
            .to_string();

        emit(
            conn,
            &ctx.session_id,
            None,
            "asset_discovered",
            serde_json::json!({ "file": filename, "ingest_type": ctx.ingest_type.as_str() }),
        )?;
        info!(file = %filename, ingest = %ctx.ingest_type.as_str(), "asset discovered");

        match wait_until_stable(&source, stability) {
            Ok(()) => {
                emit(
                    conn,
                    &ctx.session_id,
                    None,
                    "asset_stabilized",
                    serde_json::json!({ "file": filename }),
                )?;
                info!(file = %filename, "asset stabilized");
            }
            Err(StabilityError::StillWriting) | Err(StabilityError::Empty) => {
                report.failed += 1;
                report.errors.push(format!(
                    "El archivo todavía se está escribiendo: {filename}"
                ));
                continue;
            }
            Err(other) => {
                report.failed += 1;
                report.errors.push(format!("{filename}: {other}"));
                continue;
            }
        }

        let content_hash = match hash_file(&source) {
            Ok(h) => h,
            Err(e) => {
                report.failed += 1;
                report.errors.push(format!("{filename}: {e}"));
                continue;
            }
        };
        emit(
            conn,
            &ctx.session_id,
            None,
            "hash_calculated",
            serde_json::json!({ "file": filename, "hash_prefix": &content_hash[..12.min(content_hash.len())] }),
        )?;
        info!(file = %filename, "hash calculated");

        if let Some(existing) = db::find_asset_by_hash(conn, &content_hash)? {
            emit(
                conn,
                &ctx.session_id,
                Some(&existing.id),
                "duplicate_detected",
                serde_json::json!({ "file": filename, "existing_asset_id": existing.id }),
            )?;
            info!(file = %filename, existing = %existing.id, "duplicate detected");
            report.duplicates_skipped += 1;
            report.duplicates.push(DuplicateSkip {
                original_filename: filename,
                existing_asset_id: existing.id,
                content_hash,
            });
            continue;
        }

        let probe = probe_file(&source);
        let category = sel.category;
        let base = library_filename_from_original(&filename);
        let mut taken = db::list_library_filenames_in_category(conn, year, category)?;
        taken.extend(allocated_filenames.iter().cloned());
        let canonical = resolve_filename_collision(&base, &taken);
        allocated_filenames.insert(canonical.clone());
        let relative = library_relative(year, category, &canonical);
        let canonical_path = Path::new(&local.root_path).join(&relative);

        let source_before = content_hash.clone();
        let source_meta = fs::metadata(&source).ok();
        let size_bytes = source_meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let source_modified_at = source_meta.and_then(|m| {
            m.modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .and_then(|d| DateTime::from_timestamp_millis(d.as_millis() as i64))
        });

        info!(file = %filename, dest_kind = "LOCAL", "copy started");
        emit(
            conn,
            &ctx.session_id,
            None,
            "copy_started",
            serde_json::json!({ "file": filename, "kind": "LOCAL" }),
        )?;

        match copy_verified(&source, &canonical_path) {
            Ok(()) => {
                emit(
                    conn,
                    &ctx.session_id,
                    None,
                    "copy_completed",
                    serde_json::json!({ "file": canonical, "kind": "LOCAL" }),
                )?;
                info!(file = %canonical, "copy completed");
            }
            Err(e) => {
                emit(
                    conn,
                    &ctx.session_id,
                    None,
                    "copy_failed",
                    serde_json::json!({ "file": filename, "kind": "LOCAL" }),
                )?;
                report.failed += 1;
                report.errors.push(format!("Copia fallida (local): {e}"));
                bump_storage(&mut report, &local.id, false);
                continue;
            }
        }

        if hash_file(&source).ok().as_deref() != Some(source_before.as_str()) {
            report.errors.push(format!(
                "El origen cambió durante la copia (no se modificó desde Audionáutica): {filename}"
            ));
        }

        let asset = AudioAsset {
            id: new_id(),
            source_type: SourceType::AbletonConsolidate,
            ingest_type: ctx.ingest_type,
            original_filename: filename.clone(),
            original_path: source.to_string_lossy().to_string(),
            canonical_filename: canonical.clone(),
            canonical_path: canonical_path.to_string_lossy().to_string(),
            project_id: ctx.project_id.clone(),
            session_id: ctx.session_id.clone(),
            category,
            year,
            source_session_bpm: ctx.source_session_bpm,
            detected_bpm: None,
            created_at: source_modified_at.unwrap_or(now),
            harvested_at: now,
            source_modified_at,
            duration_seconds: probe.duration_seconds,
            sample_rate: probe.sample_rate,
            channels: probe.channels,
            size_bytes,
            content_hash: content_hash.clone(),
            metadata: serde_json::json!({
                "relative_source": sel.original_path,
                "ingest_type": ctx.ingest_type.as_str(),
            }),
            participant: None,
            sync_group: None,
            timeline_offset_seconds: None,
        };

        db::insert_asset(conn, &asset)?;
        db::insert_asset_storage(
            conn,
            &AssetStorageLocation {
                id: new_id(),
                asset_id: asset.id.clone(),
                storage_location_id: local.id.clone(),
                relative_path: relative.to_string_lossy().to_string(),
                copy_status: CopyStatus::Copied,
                error_message: None,
                copied_at: Some(now),
            },
        )?;
        bump_storage(&mut report, &local.id, true);

        for loc in locations.iter().filter(|l| l.id != local.id) {
            let provider = FilesystemProvider::new(loc.clone());
            emit(
                conn,
                &ctx.session_id,
                Some(&asset.id),
                "copy_started",
                serde_json::json!({ "file": canonical, "kind": loc.kind.as_str() }),
            )?;
            info!(file = %canonical, dest_kind = loc.kind.as_str(), "copy started");
            match provider.put_relative(&relative, &canonical_path) {
                Ok(_) => {
                    db::insert_asset_storage(
                        conn,
                        &AssetStorageLocation {
                            id: new_id(),
                            asset_id: asset.id.clone(),
                            storage_location_id: loc.id.clone(),
                            relative_path: relative.to_string_lossy().to_string(),
                            copy_status: CopyStatus::Copied,
                            error_message: None,
                            copied_at: Some(Utc::now()),
                        },
                    )?;
                    emit(
                        conn,
                        &ctx.session_id,
                        Some(&asset.id),
                        "copy_completed",
                        serde_json::json!({ "kind": loc.kind.as_str() }),
                    )?;
                    info!(file = %canonical, dest_kind = loc.kind.as_str(), "copy completed");
                    bump_storage(&mut report, &loc.id, true);
                }
                Err(e) => {
                    db::insert_asset_storage(
                        conn,
                        &AssetStorageLocation {
                            id: new_id(),
                            asset_id: asset.id.clone(),
                            storage_location_id: loc.id.clone(),
                            relative_path: relative.to_string_lossy().to_string(),
                            copy_status: CopyStatus::Failed,
                            error_message: Some(e.to_string()),
                            copied_at: None,
                        },
                    )?;
                    emit(
                        conn,
                        &ctx.session_id,
                        Some(&asset.id),
                        "copy_failed",
                        serde_json::json!({ "kind": loc.kind.as_str() }),
                    )?;
                    report
                        .errors
                        .push(format!("Copia fallida hacia {}: {e}", loc.kind.label_es()));
                    bump_storage(&mut report, &loc.id, false);
                }
            }
        }

        report.new_assets += 1;
        report.assets.push(HarvestedAssetSummary {
            asset_id: asset.id,
            canonical_filename: canonical,
            category,
            original_filename: filename,
            duplicate: false,
        });
    }

    for summary in &mut report.storage {
        summary.total = report.new_assets;
    }

    Ok(report)
}

pub fn ignore_consolidates(
    conn: &Connection,
    project_id: &str,
    items: &[IgnoredConsolidateInput],
) -> AppResult<u32> {
    let mut count = 0u32;
    for item in items {
        db::insert_ignored_consolidate(
            conn,
            project_id,
            &item.content_hash,
            &item.original_path,
            &item.original_filename,
        )?;
        count += 1;
    }
    Ok(count)
}

/// Copy every canonical library file into a newly configured mirror (Drive/Dropbox).
pub fn sync_mirror_from_local(conn: &Connection, location: &StorageLocation) -> AppResult<u32> {
    if !location.enabled || location.kind == StorageKind::Local {
        return Ok(0);
    }
    if db::list_storage_locations(conn)?
        .iter()
        .all(|l| l.kind != StorageKind::Local || !l.enabled)
    {
        return Err(AppError::LocalLibraryMissing);
    }
    let assets = db::list_assets(conn, None, None, None)?;
    let provider = FilesystemProvider::new(location.clone());
    let mut copied = 0u32;
    let mut years: HashSet<i32> = HashSet::new();
    for asset in &assets {
        years.insert(asset.year);
    }
    for year in years {
        ensure_year_taxonomy(Path::new(&location.root_path), year)?;
    }
    for asset in assets {
        let relative = library_relative(asset.year, asset.category, &asset.canonical_filename);
        let source = PathBuf::from(&asset.canonical_path);
        if !source.is_file() {
            continue;
        }
        let now = Utc::now();
        match provider.put_relative(&relative, &source) {
            Ok(_) => {
                db::upsert_asset_storage(
                    conn,
                    &AssetStorageLocation {
                        id: new_id(),
                        asset_id: asset.id.clone(),
                        storage_location_id: location.id.clone(),
                        relative_path: relative.to_string_lossy().to_string(),
                        copy_status: CopyStatus::Copied,
                        error_message: None,
                        copied_at: Some(now),
                    },
                )?;
                copied += 1;
            }
            Err(e) => {
                db::upsert_asset_storage(
                    conn,
                    &AssetStorageLocation {
                        id: new_id(),
                        asset_id: asset.id.clone(),
                        storage_location_id: location.id.clone(),
                        relative_path: relative.to_string_lossy().to_string(),
                        copy_status: CopyStatus::Failed,
                        error_message: Some(e.to_string()),
                        copied_at: None,
                    },
                )?;
            }
        }
    }
    Ok(copied)
}

pub fn list_library(conn: &Connection, filter: &LibraryFilter) -> AppResult<Vec<AudioAsset>> {
    db::list_assets(
        conn,
        filter.year,
        filter.category,
        filter.project_id.as_deref(),
    )
}

fn upsert_project_from_info(conn: &Connection, info: &AbletonSetInfo) -> AppResult<Project> {
    let now = Utc::now();
    let project = match db::find_project_by_root(conn, &info.project_root.to_string_lossy())? {
        Some(mut existing) => {
            existing.name = info.project_name.clone();
            existing.ableton_set_path = info.als_path.to_string_lossy().to_string();
            existing.updated_at = now;
            db::upsert_project(conn, &existing)?;
            existing
        }
        None => {
            let project = Project {
                id: new_id(),
                name: info.project_name.clone(),
                ableton_set_path: info.als_path.to_string_lossy().to_string(),
                project_root: info.project_root.to_string_lossy().to_string(),
                created_at: now,
                updated_at: now,
            };
            db::upsert_project(conn, &project)?;
            project
        }
    };
    Ok(project)
}

fn scan_snapshot(dir: &Path) -> AppResult<SessionSnapshot> {
    if !dir.exists() {
        return Ok(SessionSnapshot::default());
    }
    if !dir.is_dir() {
        return Err(AppError::ConsolidateUnavailable(dir.display().to_string()));
    }
    let files = list_audio_files(dir)?;
    let mut fps = Vec::new();
    for path in files {
        let md = match fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let relative = path
            .strip_prefix(dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let modified_unix_ms = md
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        fps.push(FileFingerprint {
            relative_path: relative,
            size_bytes: md.len(),
            modified_unix_ms,
        });
    }
    Ok(SessionSnapshot { files: fps })
}

fn emit(
    conn: &Connection,
    session_id: &str,
    asset_id: Option<&str>,
    event_type: &str,
    payload: serde_json::Value,
) -> AppResult<()> {
    db::insert_event(
        conn,
        &HarvestEvent {
            id: new_id(),
            session_id: session_id.to_string(),
            asset_id: asset_id.map(|s| s.to_string()),
            event_type: event_type.to_string(),
            payload,
            created_at: Utc::now(),
        },
    )
}

fn bump_storage(report: &mut HarvestReport, location_id: &str, ok: bool) {
    if let Some(s) = report
        .storage
        .iter_mut()
        .find(|s| s.storage_location_id == location_id)
    {
        if ok {
            s.copied += 1;
        } else {
            s.failed += 1;
        }
    }
}

fn valid_bpm(value: f64) -> Option<f64> {
    if value.is_finite() && value > 0.0 && value < 999.0 {
        Some(value)
    } else {
        None
    }
}

trait YearExt {
    fn year_for_library(&self) -> i32;
}

impl YearExt for DateTime<Utc> {
    fn year_for_library(&self) -> i32 {
        use chrono::Datelike;
        self.year()
    }
}
