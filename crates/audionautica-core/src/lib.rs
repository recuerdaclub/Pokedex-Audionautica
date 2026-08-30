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
pub mod logging;
pub mod naming;
pub mod storage;

pub use error::{AppError, AppResult};
pub use harvest::{
    abandon_active_session, archive_session, discover_candidates, end_session, import_historical,
    scan_historical_consolidates, start_session, CandidateSelection, HarvestReport, LibraryFilter,
};
