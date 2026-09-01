use std::path::Path;

use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::domain::{
    AssetStorageLocation, AudioAsset, Category, HarvestEvent, IngestType, Project, Session,
    SessionSnapshot, SessionStatus, SourceType, StorageKind, StorageLocation, CopyStatus,
};
use crate::error::{AppError, AppResult};

const MIGRATIONS: &[&str] = &[r#"
    CREATE TABLE IF NOT EXISTS schema_migrations (
        version INTEGER PRIMARY KEY,
        applied_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS projects (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        ableton_set_path TEXT NOT NULL,
        project_root TEXT NOT NULL,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS sessions (
        id TEXT PRIMARY KEY,
        project_id TEXT NOT NULL REFERENCES projects(id),
        ableton_set_path TEXT NOT NULL,
        project_root TEXT NOT NULL,
        consolidate_dir TEXT NOT NULL,
        start_time TEXT NOT NULL,
        end_time TEXT,
        source_session_bpm REAL,
        snapshot_json TEXT NOT NULL,
        status TEXT NOT NULL,
        created_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS audio_assets (
        id TEXT PRIMARY KEY,
        source_type TEXT NOT NULL,
        original_filename TEXT NOT NULL,
        original_path TEXT NOT NULL,
        canonical_filename TEXT NOT NULL,
        canonical_path TEXT NOT NULL,
        project_id TEXT NOT NULL REFERENCES projects(id),
        session_id TEXT NOT NULL REFERENCES sessions(id),
        category TEXT NOT NULL,
        year INTEGER NOT NULL,
        source_session_bpm REAL,
        detected_bpm REAL,
        created_at TEXT NOT NULL,
        harvested_at TEXT NOT NULL,
        source_modified_at TEXT,
        duration_seconds REAL,
        sample_rate INTEGER,
        channels INTEGER,
        size_bytes INTEGER NOT NULL,
        content_hash TEXT NOT NULL UNIQUE,
        metadata_json TEXT NOT NULL DEFAULT '{}',
        participant TEXT,
        sync_group TEXT,
        timeline_offset_seconds REAL
    );

    CREATE TABLE IF NOT EXISTS storage_locations (
        id TEXT PRIMARY KEY,
        kind TEXT NOT NULL,
        label TEXT NOT NULL,
        root_path TEXT NOT NULL,
        enabled INTEGER NOT NULL DEFAULT 1,
        created_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS asset_storage_locations (
        id TEXT PRIMARY KEY,
        asset_id TEXT NOT NULL REFERENCES audio_assets(id),
        storage_location_id TEXT NOT NULL REFERENCES storage_locations(id),
        relative_path TEXT NOT NULL,
        copy_status TEXT NOT NULL,
        error_message TEXT,
        copied_at TEXT
    );

    CREATE TABLE IF NOT EXISTS harvest_events (
        id TEXT PRIMARY KEY,
        session_id TEXT NOT NULL,
        asset_id TEXT,
        event_type TEXT NOT NULL,
        payload_json TEXT,
        created_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS app_settings (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_assets_hash ON audio_assets(content_hash);
    CREATE INDEX IF NOT EXISTS idx_assets_year ON audio_assets(year);
    CREATE INDEX IF NOT EXISTS idx_assets_category ON audio_assets(category);
    CREATE INDEX IF NOT EXISTS idx_assets_project ON audio_assets(project_id);
    CREATE INDEX IF NOT EXISTS idx_sessions_status ON sessions(status);
    "#,
    r#"
    ALTER TABLE audio_assets ADD COLUMN ingest_type TEXT NOT NULL DEFAULT 'SESSION_HARVEST';
    CREATE INDEX IF NOT EXISTS idx_assets_ingest_type ON audio_assets(ingest_type);
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS ignored_consolidates (
        id TEXT PRIMARY KEY,
        project_id TEXT NOT NULL REFERENCES projects(id),
        content_hash TEXT NOT NULL,
        original_path TEXT NOT NULL,
        original_filename TEXT NOT NULL,
        ignored_at TEXT NOT NULL,
        UNIQUE(project_id, content_hash)
    );
    CREATE INDEX IF NOT EXISTS idx_ignored_project ON ignored_consolidates(project_id);
    "#,
];

pub fn open(path: &Path) -> AppResult<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| AppError::from_io(e, "crear data dir"))?;
    }
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn open_in_memory() -> AppResult<Connection> {
    let conn = Connection::open_in_memory()?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        );",
    )?;
    let current: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    for (idx, sql) in MIGRATIONS.iter().enumerate() {
        let version = (idx + 1) as i64;
        if version > current {
            conn.execute_batch(sql)?;
            conn.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![version, Utc::now().to_rfc3339()],
            )?;
        }
    }
    Ok(())
}

pub fn get_setting(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        params![key],
        |r| r.get(0),
    )
    .optional()
    .map_err(Into::into)
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn upsert_project(conn: &Connection, project: &Project) -> AppResult<()> {
    conn.execute(
        "INSERT INTO projects (id, name, ableton_set_path, project_root, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            ableton_set_path = excluded.ableton_set_path,
            project_root = excluded.project_root,
            updated_at = excluded.updated_at",
        params![
            project.id,
            project.name,
            project.ableton_set_path,
            project.project_root,
            project.created_at.to_rfc3339(),
            project.updated_at.to_rfc3339()
        ],
    )?;
    Ok(())
}

pub fn find_project_by_root(conn: &Connection, project_root: &str) -> AppResult<Option<Project>> {
    conn.query_row(
        "SELECT id, name, ableton_set_path, project_root, created_at, updated_at
         FROM projects WHERE project_root = ?1 LIMIT 1",
        params![project_root],
        row_project,
    )
    .optional()
    .map_err(Into::into)
}

fn row_project(r: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: r.get(0)?,
        name: r.get(1)?,
        ableton_set_path: r.get(2)?,
        project_root: r.get(3)?,
        created_at: parse_dt(&r.get::<_, String>(4)?),
        updated_at: parse_dt(&r.get::<_, String>(5)?),
    })
}

pub fn insert_session(conn: &Connection, session: &Session) -> AppResult<()> {
    conn.execute(
        "INSERT INTO sessions (
            id, project_id, ableton_set_path, project_root, consolidate_dir,
            start_time, end_time, source_session_bpm, snapshot_json, status, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            session.id,
            session.project_id,
            session.ableton_set_path,
            session.project_root,
            session.consolidate_dir,
            session.start_time.to_rfc3339(),
            session.end_time.map(|t| t.to_rfc3339()),
            session.source_session_bpm,
            serde_json::to_string(&session.snapshot)?,
            session.status.as_str(),
            session.start_time.to_rfc3339()
        ],
    )?;
    Ok(())
}

pub fn update_session(conn: &Connection, session: &Session) -> AppResult<()> {
    conn.execute(
        "UPDATE sessions SET end_time = ?1, source_session_bpm = ?2, status = ?3 WHERE id = ?4",
        params![
            session.end_time.map(|t| t.to_rfc3339()),
            session.source_session_bpm,
            session.status.as_str(),
            session.id
        ],
    )?;
    Ok(())
}

pub fn get_session(conn: &Connection, id: &str) -> AppResult<Option<Session>> {
    conn.query_row(
        "SELECT s.id, s.project_id, p.name, s.ableton_set_path, s.project_root, s.consolidate_dir,
                s.start_time, s.end_time, s.source_session_bpm, s.snapshot_json, s.status
         FROM sessions s JOIN projects p ON p.id = s.project_id
         WHERE s.id = ?1",
        params![id],
        row_session,
    )
    .optional()
    .map_err(Into::into)
}

pub fn find_active_session(conn: &Connection) -> AppResult<Option<Session>> {
    conn.query_row(
        "SELECT s.id, s.project_id, p.name, s.ableton_set_path, s.project_root, s.consolidate_dir,
                s.start_time, s.end_time, s.source_session_bpm, s.snapshot_json, s.status
         FROM sessions s JOIN projects p ON p.id = s.project_id
         WHERE s.status IN ('ACTIVE', 'REVIEW')
         ORDER BY s.start_time DESC LIMIT 1",
        [],
        row_session,
    )
    .optional()
    .map_err(Into::into)
}

fn row_session(r: &rusqlite::Row<'_>) -> rusqlite::Result<Session> {
    let snapshot_json: String = r.get(9)?;
    let snapshot: SessionSnapshot = serde_json::from_str(&snapshot_json).unwrap_or_default();
    Ok(Session {
        id: r.get(0)?,
        project_id: r.get(1)?,
        project_name: r.get(2)?,
        ableton_set_path: r.get(3)?,
        project_root: r.get(4)?,
        consolidate_dir: r.get(5)?,
        start_time: parse_dt(&r.get::<_, String>(6)?),
        end_time: r.get::<_, Option<String>>(7)?.as_deref().map(parse_dt),
        source_session_bpm: r.get(8)?,
        snapshot,
        status: SessionStatus::parse(&r.get::<_, String>(10)?),
    })
}

pub fn insert_asset(conn: &Connection, asset: &AudioAsset) -> AppResult<()> {
    conn.execute(
        "INSERT INTO audio_assets (
            id, source_type, original_filename, original_path, canonical_filename, canonical_path,
            project_id, session_id, category, year, source_session_bpm, detected_bpm,
            created_at, harvested_at, source_modified_at, duration_seconds, sample_rate, channels,
            size_bytes, content_hash, metadata_json, participant, sync_group, timeline_offset_seconds,
            ingest_type
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25
        )",
        params![
            asset.id,
            asset.source_type.as_str(),
            asset.original_filename,
            asset.original_path,
            asset.canonical_filename,
            asset.canonical_path,
            asset.project_id,
            asset.session_id,
            asset.category.as_str(),
            asset.year,
            asset.source_session_bpm,
            asset.detected_bpm,
            asset.created_at.to_rfc3339(),
            asset.harvested_at.to_rfc3339(),
            asset.source_modified_at.map(|t| t.to_rfc3339()),
            asset.duration_seconds,
            asset.sample_rate,
            asset.channels,
            asset.size_bytes as i64,
            asset.content_hash,
            asset.metadata.to_string(),
            asset.participant,
            asset.sync_group,
            asset.timeline_offset_seconds,
            asset.ingest_type.as_str()
        ],
    )?;
    Ok(())
}

pub fn get_asset(conn: &Connection, id: &str) -> AppResult<Option<AudioAsset>> {
    conn.query_row(
        "SELECT id, source_type, original_filename, original_path, canonical_filename, canonical_path,
                project_id, session_id, category, year, source_session_bpm, detected_bpm,
                created_at, harvested_at, source_modified_at, duration_seconds, sample_rate, channels,
                size_bytes, content_hash, metadata_json, participant, sync_group, timeline_offset_seconds,
                ingest_type
         FROM audio_assets WHERE id = ?1",
        params![id],
        row_asset,
    )
    .optional()
    .map_err(Into::into)
}

pub fn list_asset_storage_for_asset(
    conn: &Connection,
    asset_id: &str,
) -> AppResult<Vec<(AssetStorageLocation, StorageLocation)>> {
    let mut stmt = conn.prepare(
        "SELECT asl.id, asl.asset_id, asl.storage_location_id, asl.relative_path, asl.copy_status,
                asl.error_message, asl.copied_at,
                sl.id, sl.kind, sl.label, sl.root_path, sl.enabled, sl.created_at
         FROM asset_storage_locations asl
         JOIN storage_locations sl ON sl.id = asl.storage_location_id
         WHERE asl.asset_id = ?1",
    )?;
    let rows = stmt.query_map(params![asset_id], |r| {
        let copied_at: Option<String> = r.get(6)?;
        Ok((
            AssetStorageLocation {
                id: r.get(0)?,
                asset_id: r.get(1)?,
                storage_location_id: r.get(2)?,
                relative_path: r.get(3)?,
                copy_status: CopyStatus::parse(&r.get::<_, String>(4)?),
                error_message: r.get(5)?,
                copied_at: copied_at.as_deref().map(parse_dt),
            },
            StorageLocation {
                id: r.get(7)?,
                kind: StorageKind::parse(&r.get::<_, String>(8)?),
                label: r.get(9)?,
                root_path: r.get(10)?,
                enabled: r.get::<_, i64>(11)? != 0,
                created_at: parse_dt(&r.get::<_, String>(12)?),
            },
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn delete_asset(conn: &Connection, asset_id: &str) -> AppResult<()> {
    conn.execute(
        "DELETE FROM asset_storage_locations WHERE asset_id = ?1",
        params![asset_id],
    )?;
    let n = conn.execute("DELETE FROM audio_assets WHERE id = ?1", params![asset_id])?;
    if n == 0 {
        return Err(AppError::AssetNotFound(asset_id.to_string()));
    }
    Ok(())
}

pub fn update_asset_metadata(
    conn: &Connection,
    asset_id: &str,
    category: Category,
    canonical_filename: &str,
    canonical_path: &str,
) -> AppResult<()> {
    let n = conn.execute(
        "UPDATE audio_assets SET category = ?1, canonical_filename = ?2, canonical_path = ?3 WHERE id = ?4",
        params![category.as_str(), canonical_filename, canonical_path, asset_id],
    )?;
    if n == 0 {
        return Err(AppError::AssetNotFound(asset_id.to_string()));
    }
    Ok(())
}

pub fn update_asset_storage_relative_path(
    conn: &Connection,
    asset_storage_id: &str,
    relative_path: &str,
) -> AppResult<()> {
    conn.execute(
        "UPDATE asset_storage_locations SET relative_path = ?1 WHERE id = ?2",
        params![relative_path, asset_storage_id],
    )?;
    Ok(())
}

pub fn find_asset_by_hash(conn: &Connection, hash: &str) -> AppResult<Option<AudioAsset>> {
    conn.query_row(
        "SELECT id, source_type, original_filename, original_path, canonical_filename, canonical_path,
                project_id, session_id, category, year, source_session_bpm, detected_bpm,
                created_at, harvested_at, source_modified_at, duration_seconds, sample_rate, channels,
                size_bytes, content_hash, metadata_json, participant, sync_group, timeline_offset_seconds,
                ingest_type
         FROM audio_assets WHERE content_hash = ?1",
        params![hash],
        row_asset,
    )
    .optional()
    .map_err(Into::into)
}

fn row_asset(r: &rusqlite::Row<'_>) -> rusqlite::Result<AudioAsset> {
    let meta: String = r.get(20)?;
    Ok(AudioAsset {
        id: r.get(0)?,
        source_type: SourceType::parse(&r.get::<_, String>(1)?),
        original_filename: r.get(2)?,
        original_path: r.get(3)?,
        canonical_filename: r.get(4)?,
        canonical_path: r.get(5)?,
        project_id: r.get(6)?,
        session_id: r.get(7)?,
        category: Category::parse(&r.get::<_, String>(8)?),
        year: r.get(9)?,
        source_session_bpm: r.get(10)?,
        detected_bpm: r.get(11)?,
        created_at: parse_dt(&r.get::<_, String>(12)?),
        harvested_at: parse_dt(&r.get::<_, String>(13)?),
        source_modified_at: r.get::<_, Option<String>>(14)?.as_deref().map(parse_dt),
        duration_seconds: r.get(15)?,
        sample_rate: r.get::<_, Option<i64>>(16)?.map(|v| v as u32),
        channels: r.get::<_, Option<i64>>(17)?.map(|v| v as u16),
        size_bytes: r.get::<_, i64>(18)? as u64,
        content_hash: r.get(19)?,
        metadata: serde_json::from_str(&meta).unwrap_or(serde_json::json!({})),
        participant: r.get(21)?,
        sync_group: r.get(22)?,
        timeline_offset_seconds: r.get(23)?,
        ingest_type: IngestType::parse(&r.get::<_, String>(24)?),
    })
}

pub fn all_content_hashes(conn: &Connection) -> AppResult<std::collections::HashSet<String>> {
    let mut stmt = conn.prepare("SELECT content_hash FROM audio_assets")?;
    let rows = stmt.query_map([], |r| r.get(0))?;
    let mut out = std::collections::HashSet::new();
    for row in rows {
        out.insert(row?);
    }
    Ok(out)
}

pub fn next_sequence(
    conn: &Connection,
    year: i32,
    category: Category,
    project_id: &str,
) -> AppResult<u32> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM audio_assets WHERE year = ?1 AND category = ?2 AND project_id = ?3",
        params![year, category.as_str(), project_id],
        |r| r.get(0),
    )?;
    Ok((count as u32) + 1)
}

pub fn list_library_filenames_in_category(
    conn: &Connection,
    year: i32,
    category: Category,
) -> AppResult<std::collections::HashSet<String>> {
    let mut stmt = conn.prepare(
        "SELECT canonical_filename FROM audio_assets WHERE year = ?1 AND category = ?2",
    )?;
    let rows = stmt.query_map(params![year, category.as_str()], |r| r.get(0))?;
    let mut out = std::collections::HashSet::new();
    for row in rows {
        out.insert(row?);
    }
    Ok(out)
}

pub fn ignored_hashes_for_project(
    conn: &Connection,
    project_id: &str,
) -> AppResult<std::collections::HashSet<String>> {
    let mut stmt =
        conn.prepare("SELECT content_hash FROM ignored_consolidates WHERE project_id = ?1")?;
    let rows = stmt.query_map(params![project_id], |r| r.get(0))?;
    let mut out = std::collections::HashSet::new();
    for row in rows {
        out.insert(row?);
    }
    Ok(out)
}

pub fn insert_ignored_consolidate(
    conn: &Connection,
    project_id: &str,
    content_hash: &str,
    original_path: &str,
    original_filename: &str,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO ignored_consolidates (id, project_id, content_hash, original_path, original_filename, ignored_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(project_id, content_hash) DO UPDATE SET
            original_path = excluded.original_path,
            original_filename = excluded.original_filename,
            ignored_at = excluded.ignored_at",
        params![
            crate::domain::new_id(),
            project_id,
            content_hash,
            original_path,
            original_filename,
            Utc::now().to_rfc3339()
        ],
    )?;
    Ok(())
}

pub fn remove_ignored_for_hash(conn: &Connection, project_id: &str, content_hash: &str) -> AppResult<()> {
    conn.execute(
        "DELETE FROM ignored_consolidates WHERE project_id = ?1 AND content_hash = ?2",
        params![project_id, content_hash],
    )?;
    Ok(())
}

pub fn list_assets(
    conn: &Connection,
    year: Option<i32>,
    category: Option<Category>,
    project_id: Option<&str>,
) -> AppResult<Vec<AudioAsset>> {
    let mut sql = String::from(
        "SELECT id, source_type, original_filename, original_path, canonical_filename, canonical_path,
                project_id, session_id, category, year, source_session_bpm, detected_bpm,
                created_at, harvested_at, source_modified_at, duration_seconds, sample_rate, channels,
                size_bytes, content_hash, metadata_json, participant, sync_group, timeline_offset_seconds,
                ingest_type
         FROM audio_assets WHERE 1=1",
    );
    if year.is_some() {
        sql.push_str(" AND year = ?");
    }
    if category.is_some() {
        sql.push_str(" AND category = ?");
    }
    if project_id.is_some() {
        sql.push_str(" AND project_id = ?");
    }
    sql.push_str(" ORDER BY harvested_at DESC");
    let mut stmt = conn.prepare(&sql)?;
    let mut idx = 1;
    let mut bindings: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    if let Some(y) = year {
        bindings.push(Box::new(y));
        idx += 1;
        let _ = idx;
    }
    if let Some(c) = category {
        bindings.push(Box::new(c.as_str().to_string()));
    }
    if let Some(p) = project_id {
        bindings.push(Box::new(p.to_string()));
    }
    let refs: Vec<&dyn rusqlite::types::ToSql> = bindings.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(refs.as_slice(), row_asset)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn list_projects(conn: &Connection) -> AppResult<Vec<Project>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, ableton_set_path, project_root, created_at, updated_at
         FROM projects ORDER BY name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], row_project)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn list_storage_locations(conn: &Connection) -> AppResult<Vec<StorageLocation>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, label, root_path, enabled, created_at FROM storage_locations ORDER BY created_at",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(StorageLocation {
            id: r.get(0)?,
            kind: StorageKind::parse(&r.get::<_, String>(1)?),
            label: r.get(2)?,
            root_path: r.get(3)?,
            enabled: r.get::<_, i64>(4)? != 0,
            created_at: parse_dt(&r.get::<_, String>(5)?),
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn upsert_storage_location(conn: &Connection, loc: &StorageLocation) -> AppResult<()> {
    conn.execute(
        "INSERT INTO storage_locations (id, kind, label, root_path, enabled, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET
            kind = excluded.kind,
            label = excluded.label,
            root_path = excluded.root_path,
            enabled = excluded.enabled",
        params![
            loc.id,
            loc.kind.as_str(),
            loc.label,
            loc.root_path,
            if loc.enabled { 1 } else { 0 },
            loc.created_at.to_rfc3339()
        ],
    )?;
    Ok(())
}

pub fn delete_storage_location(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM storage_locations WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn insert_asset_storage(conn: &Connection, rec: &AssetStorageLocation) -> AppResult<()> {
    conn.execute(
        "INSERT INTO asset_storage_locations (
            id, asset_id, storage_location_id, relative_path, copy_status, error_message, copied_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            rec.id,
            rec.asset_id,
            rec.storage_location_id,
            rec.relative_path,
            rec.copy_status.as_str(),
            rec.error_message,
            rec.copied_at.map(|t| t.to_rfc3339())
        ],
    )?;
    Ok(())
}

pub fn find_asset_storage(
    conn: &Connection,
    asset_id: &str,
    storage_location_id: &str,
) -> AppResult<Option<AssetStorageLocation>> {
    conn.query_row(
        "SELECT id, asset_id, storage_location_id, relative_path, copy_status, error_message, copied_at
         FROM asset_storage_locations
         WHERE asset_id = ?1 AND storage_location_id = ?2
         LIMIT 1",
        params![asset_id, storage_location_id],
        |r| {
            let copied_at: Option<String> = r.get(6)?;
            Ok(AssetStorageLocation {
                id: r.get(0)?,
                asset_id: r.get(1)?,
                storage_location_id: r.get(2)?,
                relative_path: r.get(3)?,
                copy_status: CopyStatus::parse(&r.get::<_, String>(4)?),
                error_message: r.get(5)?,
                copied_at: copied_at.as_deref().map(parse_dt),
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

pub fn upsert_asset_storage(conn: &Connection, rec: &AssetStorageLocation) -> AppResult<()> {
    if find_asset_storage(conn, &rec.asset_id, &rec.storage_location_id)?.is_some() {
        conn.execute(
            "UPDATE asset_storage_locations
             SET relative_path = ?1, copy_status = ?2, error_message = ?3, copied_at = ?4
             WHERE asset_id = ?5 AND storage_location_id = ?6",
            params![
                rec.relative_path,
                rec.copy_status.as_str(),
                rec.error_message,
                rec.copied_at.map(|t| t.to_rfc3339()),
                rec.asset_id,
                rec.storage_location_id
            ],
        )?;
        return Ok(());
    }
    insert_asset_storage(conn, rec)
}

pub fn insert_event(conn: &Connection, event: &HarvestEvent) -> AppResult<()> {
    conn.execute(
        "INSERT INTO harvest_events (id, session_id, asset_id, event_type, payload_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            event.id,
            event.session_id,
            event.asset_id,
            event.event_type,
            event.payload.to_string(),
            event.created_at.to_rfc3339()
        ],
    )?;
    Ok(())
}

fn parse_dt(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc.timestamp_opt(0, 0).unwrap())
}
