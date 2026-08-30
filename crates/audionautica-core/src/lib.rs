//! Audionáutica core: domain, Ableton adapter, filesystem storage, harvest pipeline.
//!
//! This crate has **no** dependency on Tauri, React, Dropbox APIs or Google Drive APIs.

pub mod ableton;
pub mod audio;
pub mod db;
pub mod domain;
pub mod error;
pub mod fsutil;
pub mod harvest;
pub mod hash;
pub mod library;
pub mod logging;
pub mod naming;
pub mod storage;

pub use error::{AppError, AppResult};
pub use harvest::{
    abandon_active_session, archive_session, discover_candidates, end_session, ignore_consolidates,
    import_historical, scan_historical_consolidates, start_session, sync_mirror_from_local,
    CandidateSelection,
    HarvestReport, LibraryFilter,
};
pub use library::delete_from_library;
pub use domain::DeleteFromLibraryReport;
