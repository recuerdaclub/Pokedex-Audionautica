#![allow(linker_messages)]

use std::path::PathBuf;
use std::sync::Mutex;

use audionautica_core::ableton::AbletonProjectReader;
use audionautica_core::db;
use audionautica_core::domain::{Category, Session, StorageKind, StorageLocation};
use audionautica_core::fsutil::stability::StabilityConfig;
use audionautica_core::harvest::{
    self, CandidateSelection, HarvestCandidate, HarvestReport, LibraryFilter,
};
use audionautica_core::domain::IgnoredConsolidateInput;
use audionautica_core::logging;
use rusqlite::Connection;
use serde::Serialize;
use tauri::{Manager, State};

pub struct AppState {
    pub conn: Mutex<Connection>,
    pub data_dir: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct UiAppState {
    pub data_dir: String,
    pub db_path: String,
    pub last_als_path: Option<String>,
    pub active_session: Option<Session>,
    pub storage_locations: Vec<StorageLocation>,
}

fn map_err(err: audionautica_core::AppError) -> String {
    err.to_string()
}

fn snapshot(state: &AppState) -> Result<UiAppState, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let last = db::get_setting(&conn, "last_als_path").map_err(map_err)?;
    Ok(UiAppState {
        data_dir: state.data_dir.to_string_lossy().to_string(),
        db_path: state
            .data_dir
            .join("audionautica.sqlite")
            .to_string_lossy()
            .to_string(),
        last_als_path: last,
        active_session: db::find_active_session(&conn).map_err(map_err)?,
        storage_locations: db::list_storage_locations(&conn).map_err(map_err)?,
    })
}

#[tauri::command]
fn get_app_state(state: State<AppState>) -> Result<UiAppState, String> {
    snapshot(&state)
}

#[tauri::command]
fn inspect_ableton_set(path: String) -> Result<audionautica_core::ableton::AbletonSetInfo, String> {
    AbletonProjectReader::inspect(std::path::Path::new(&path)).map_err(map_err)
}

#[tauri::command]
fn scan_historical_consolidates(
    state: State<AppState>,
    als_path: String,
) -> Result<audionautica_core::domain::ProjectLibraryStatus, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    harvest::scan_historical_consolidates(&conn, std::path::Path::new(&als_path)).map_err(map_err)
}

#[tauri::command]
fn import_historical(
    state: State<AppState>,
    als_path: String,
    bpm_override: Option<f64>,
    selections: Vec<CandidateSelection>,
) -> Result<HarvestReport, String> {
    let mut conn = state.conn.lock().map_err(|e| e.to_string())?;
    harvest::import_historical(
        &mut conn,
        std::path::Path::new(&als_path),
        bpm_override,
        &selections,
        &StabilityConfig::default(),
    )
    .map_err(map_err)
}

#[tauri::command]
fn start_session(
    state: State<AppState>,
    als_path: String,
    bpm_override: Option<f64>,
) -> Result<Session, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let session = harvest::start_session(&conn, std::path::Path::new(&als_path), bpm_override)
        .map_err(map_err)?;
    db::set_setting(&conn, "last_als_path", &als_path).map_err(map_err)?;
    Ok(session)
}

#[derive(Debug, Serialize)]
struct EndSessionResult {
    session: Session,
    candidates: Vec<HarvestCandidate>,
}

#[tauri::command]
fn end_session(state: State<AppState>) -> Result<EndSessionResult, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let active = db::find_active_session(&conn)
        .map_err(map_err)?
        .ok_or_else(|| audionautica_core::AppError::NoActiveSession.to_string())?;
    let (session, candidates) = harvest::end_session(&conn, &active.id).map_err(map_err)?;
    Ok(EndSessionResult {
        session,
        candidates,
    })
}

#[tauri::command]
fn abandon_session(state: State<AppState>) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    harvest::abandon_active_session(&conn).map_err(map_err)
}

#[tauri::command]
fn archive_session(
    state: State<AppState>,
    session_id: String,
    selections: Vec<CandidateSelection>,
) -> Result<HarvestReport, String> {
    let mut conn = state.conn.lock().map_err(|e| e.to_string())?;
    harvest::archive_session(
        &mut conn,
        &session_id,
        &selections,
        &StabilityConfig::default(),
    )
    .map_err(map_err)
}

#[tauri::command]
fn set_session_bpm(
    state: State<AppState>,
    session_id: String,
    bpm: Option<f64>,
) -> Result<Session, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    harvest::set_session_bpm(&conn, &session_id, bpm).map_err(map_err)
}

#[tauri::command]
fn save_storage_location(
    state: State<AppState>,
    id: Option<String>,
    kind: StorageKind,
    label: String,
    root_path: String,
    enabled: bool,
) -> Result<UiAppState, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let locations = db::list_storage_locations(&conn).map_err(map_err)?;
    let existing = if let Some(want) = id.as_ref() {
        locations.iter().find(|l| &l.id == want).cloned()
    } else {
        locations.iter().find(|l| l.kind == kind).cloned()
    };
    let loc = StorageLocation {
        id: existing
            .as_ref()
            .map(|l| l.id.clone())
            .unwrap_or_else(audionautica_core::domain::new_id),
        kind,
        label,
        root_path,
        enabled,
        created_at: existing
            .map(|l| l.created_at)
            .unwrap_or_else(chrono::Utc::now),
    };
    db::upsert_storage_location(&conn, &loc).map_err(map_err)?;
    if loc.enabled && loc.kind != StorageKind::Local {
        harvest::sync_mirror_from_local(&conn, &loc).map_err(map_err)?;
    }
    drop(conn);
    snapshot(&state)
}

#[tauri::command]
fn delete_storage_location(state: State<AppState>, id: String) -> Result<UiAppState, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    db::delete_storage_location(&conn, &id).map_err(map_err)?;
    drop(conn);
    snapshot(&state)
}

#[tauri::command]
fn ignore_consolidates(
    state: State<AppState>,
    project_id: String,
    items: Vec<IgnoredConsolidateInput>,
) -> Result<u32, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    harvest::ignore_consolidates(&conn, &project_id, &items).map_err(map_err)
}

#[tauri::command]
fn reveal_path(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err(format!("La ruta no existe: {path}"));
    }
    open::that(p).map_err(|e| format!("No se pudo abrir la ruta: {e}"))
}

#[tauri::command]
fn list_library(
    state: State<AppState>,
    year: Option<i32>,
    category: Option<Category>,
    project_id: Option<String>,
) -> Result<Vec<audionautica_core::domain::AudioAsset>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    harvest::list_library(
        &conn,
        &LibraryFilter {
            year,
            category,
            project_id,
        },
    )
    .map_err(map_err)
}

#[tauri::command]
fn list_projects(
    state: State<AppState>,
) -> Result<Vec<audionautica_core::domain::Project>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    db::list_projects(&conn).map_err(map_err)
}

#[tauri::command]
fn delete_from_library(
    state: State<AppState>,
    asset_id: String,
) -> Result<audionautica_core::domain::DeleteFromLibraryReport, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    audionautica_core::delete_from_library(&conn, &asset_id).map_err(map_err)
}

#[tauri::command]
fn update_library_asset(
    state: State<AppState>,
    asset_id: String,
    new_category: Option<Category>,
    new_filename: Option<String>,
) -> Result<audionautica_core::domain::UpdateLibraryAssetReport, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    audionautica_core::update_library_asset(&conn, &asset_id, new_category, new_filename).map_err(map_err)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir().join("audionautica"));
            let _ = std::fs::create_dir_all(&data_dir);
            logging::init_file_logging(&data_dir.join("logs"));
            let db_path = data_dir.join("audionautica.sqlite");
            let conn = db::open(&db_path).expect("open sqlite");
            app.manage(AppState {
                conn: Mutex::new(conn),
                data_dir,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_state,
            inspect_ableton_set,
            scan_historical_consolidates,
            import_historical,
            ignore_consolidates,
            abandon_session,
            start_session,
            end_session,
            archive_session,
            set_session_bpm,
            save_storage_location,
            delete_storage_location,
            list_library,
            list_projects,
            delete_from_library,
            update_library_asset,
            reveal_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Audionáutica");
}
