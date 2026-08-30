use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Stable identity independent of any filesystem path.
pub type AssetId = String;
pub type ProjectId = String;
pub type SessionId = String;
pub type StorageLocationId = String;
pub type HarvestEventId = String;

pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

/// Origin of an audio asset. Sprint 1 only *produces* Ableton consolidates,
/// but the domain is not coupled exclusively to that source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SourceType {
    AbletonConsolidate,
    SonobusStem,
    RemoteCollab,
    FieldRecording,
    IpadInstrument,
    CommunityUpload,
    GeneratedAudio,
}

/// How an asset entered the library. Distinct from [`SourceType`] (where the audio came from).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IngestType {
    #[default]
    SessionHarvest,
    HistoricalImport,
}

impl IngestType {
    pub fn as_str(self) -> &'static str {
        match self {
            IngestType::SessionHarvest => "SESSION_HARVEST",
            IngestType::HistoricalImport => "HISTORICAL_IMPORT",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "HISTORICAL_IMPORT" => IngestType::HistoricalImport,
            _ => IngestType::SessionHarvest,
        }
    }
}

impl SourceType {
    pub fn as_str(self) -> &'static str {
        match self {
            SourceType::AbletonConsolidate => "ABLETON_CONSOLIDATE",
            SourceType::SonobusStem => "SONOBUS_STEM",
            SourceType::RemoteCollab => "REMOTE_COLLAB",
            SourceType::FieldRecording => "FIELD_RECORDING",
            SourceType::IpadInstrument => "IPAD_INSTRUMENT",
            SourceType::CommunityUpload => "COMMUNITY_UPLOAD",
            SourceType::GeneratedAudio => "GENERATED_AUDIO",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "SONOBUS_STEM" => SourceType::SonobusStem,
            "REMOTE_COLLAB" => SourceType::RemoteCollab,
            "FIELD_RECORDING" => SourceType::FieldRecording,
            "IPAD_INSTRUMENT" => SourceType::IpadInstrument,
            "COMMUNITY_UPLOAD" => SourceType::CommunityUpload,
            "GENERATED_AUDIO" => SourceType::GeneratedAudio,
            _ => SourceType::AbletonConsolidate,
        }
    }
}

/// Musical taxonomy. Physical folders are a projection, not the source of truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Category {
    Harmonies,
    Rhythms,
    Textures,
    Percussion,
    Bass,
    Voices,
    FieldFx,
    #[default]
    Other,
}

impl Category {
    pub const ALL: [Category; 8] = [
        Category::Harmonies,
        Category::Rhythms,
        Category::Textures,
        Category::Percussion,
        Category::Bass,
        Category::Voices,
        Category::FieldFx,
        Category::Other,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Category::Harmonies => "HARMONIES",
            Category::Rhythms => "RHYTHMS",
            Category::Textures => "TEXTURES",
            Category::Percussion => "PERCUSSION",
            Category::Bass => "BASS",
            Category::Voices => "VOICES",
            Category::FieldFx => "FIELD_FX",
            Category::Other => "OTHER",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "HARMONIES" => Category::Harmonies,
            "RHYTHMS" => Category::Rhythms,
            "TEXTURES" => Category::Textures,
            "PERCUSSION" => Category::Percussion,
            "BASS" => Category::Bass,
            "VOICES" => Category::Voices,
            "FIELD_FX" => Category::FieldFx,
            _ => Category::Other,
        }
    }

    /// Label shown in the UI (Spanish).
    pub fn label_es(self) -> &'static str {
        match self {
            Category::Harmonies => "Armonías",
            Category::Rhythms => "Ritmos",
            Category::Textures => "Texturas",
            Category::Percussion => "Percusión",
            Category::Bass => "Bajos",
            Category::Voices => "Voces",
            Category::FieldFx => "Field / FX",
            Category::Other => "Otros",
        }
    }

    /// Filesystem folder name (no accents — safer on Windows).
    pub fn folder_name(self) -> &'static str {
        match self {
            Category::Harmonies => "Armonias",
            Category::Rhythms => "Ritmos",
            Category::Textures => "Texturas",
            Category::Percussion => "Percusion",
            Category::Bass => "Bajos",
            Category::Voices => "Voces",
            Category::FieldFx => "Field_FX",
            Category::Other => "Otros",
        }
    }

    /// Token used in canonical filenames.
    pub fn filename_token(self) -> &'static str {
        match self {
            Category::Harmonies => "HARMONY",
            Category::Rhythms => "RHYTHM",
            Category::Textures => "TEXTURE",
            Category::Percussion => "PERC",
            Category::Bass => "BASS",
            Category::Voices => "VOICE",
            Category::FieldFx => "FIELDFX",
            Category::Other => "OTHER",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StorageKind {
    Local,
    DropboxFolder,
    GoogleDriveFolder,
    CustomFolder,
}

impl StorageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            StorageKind::Local => "LOCAL",
            StorageKind::DropboxFolder => "DROPBOX_FOLDER",
            StorageKind::GoogleDriveFolder => "GOOGLE_DRIVE_FOLDER",
            StorageKind::CustomFolder => "CUSTOM_FOLDER",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "DROPBOX_FOLDER" => StorageKind::DropboxFolder,
            "GOOGLE_DRIVE_FOLDER" => StorageKind::GoogleDriveFolder,
            "CUSTOM_FOLDER" => StorageKind::CustomFolder,
            _ => StorageKind::Local,
        }
    }

    pub fn label_es(self) -> &'static str {
        match self {
            StorageKind::Local => "Biblioteca local",
            StorageKind::DropboxFolder => "Carpeta Dropbox",
            StorageKind::GoogleDriveFolder => "Carpeta Google Drive",
            StorageKind::CustomFolder => "Carpeta personalizada",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CopyStatus {
    Pending,
    Copied,
    Failed,
}

impl CopyStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            CopyStatus::Pending => "PENDING",
            CopyStatus::Copied => "COPIED",
            CopyStatus::Failed => "FAILED",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "COPIED" => CopyStatus::Copied,
            "FAILED" => CopyStatus::Failed,
            _ => CopyStatus::Pending,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SessionStatus {
    Active,
    Review,
    Archived,
    Cancelled,
}

impl SessionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            SessionStatus::Active => "ACTIVE",
            SessionStatus::Review => "REVIEW",
            SessionStatus::Archived => "ARCHIVED",
            SessionStatus::Cancelled => "CANCELLED",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "REVIEW" => SessionStatus::Review,
            "ARCHIVED" => SessionStatus::Archived,
            "CANCELLED" => SessionStatus::Cancelled,
            _ => SessionStatus::Active,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub ableton_set_path: String,
    pub project_root: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileFingerprint {
    pub relative_path: String,
    pub size_bytes: u64,
    pub modified_unix_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionSnapshot {
    pub files: Vec<FileFingerprint>,
}

impl SessionSnapshot {
    pub fn get(&self, relative_path: &str) -> Option<&FileFingerprint> {
        self.files.iter().find(|f| f.relative_path == relative_path)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub project_id: ProjectId,
    pub project_name: String,
    pub ableton_set_path: String,
    pub project_root: String,
    pub consolidate_dir: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub source_session_bpm: Option<f64>,
    pub snapshot: SessionSnapshot,
    pub status: SessionStatus,
}

/// Future-compatible audio asset. Identity is `id` + `content_hash`, never a path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioAsset {
    pub id: AssetId,
    pub source_type: SourceType,
    pub ingest_type: IngestType,
    pub original_filename: String,
    pub original_path: String,
    pub canonical_filename: String,
    pub canonical_path: String,
    pub project_id: ProjectId,
    pub session_id: SessionId,
    pub category: Category,
    pub year: i32,
    pub source_session_bpm: Option<f64>,
    pub detected_bpm: Option<f64>,
    pub created_at: DateTime<Utc>,
    pub harvested_at: DateTime<Utc>,
    pub source_modified_at: Option<DateTime<Utc>>,
    pub duration_seconds: Option<f64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub size_bytes: u64,
    pub content_hash: String,
    pub metadata: serde_json::Value,
    /// Reserved for SonoBus / remote jam (Sprint 1 always null).
    pub participant: Option<String>,
    pub sync_group: Option<String>,
    pub timeline_offset_seconds: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageLocation {
    pub id: StorageLocationId,
    pub kind: StorageKind,
    pub label: String,
    pub root_path: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetStorageLocation {
    pub id: String,
    pub asset_id: AssetId,
    pub storage_location_id: StorageLocationId,
    pub relative_path: String,
    pub copy_status: CopyStatus,
    pub error_message: Option<String>,
    pub copied_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarvestEvent {
    pub id: HarvestEventId,
    pub session_id: SessionId,
    pub asset_id: Option<AssetId>,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChangeKind {
    New,
    Modified,
}

/// Consolidate on disk not yet represented in the library (by content hash).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalConsolidate {
    pub original_path: String,
    pub original_filename: String,
    /// Musical library name with Ableton timestamp stripped only.
    pub library_filename: String,
    pub relative_path: String,
    pub size_bytes: u64,
    pub modified_at: DateTime<Utc>,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IgnoredConsolidateInput {
    pub original_path: String,
    pub content_hash: String,
    pub original_filename: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectLibraryStatus {
    pub project_id: String,
    pub project_name: String,
    pub consolidate_dir: String,
    pub pending: Vec<HistoricalConsolidate>,
    pub archived_count: u32,
    pub synced: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarvestCandidate {
    pub original_path: String,
    pub original_filename: String,
    /// Musical library name with Ableton timestamp stripped only.
    pub library_filename: String,
    pub relative_path: String,
    pub size_bytes: u64,
    pub modified_at: DateTime<Utc>,
    pub change_kind: ChangeKind,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateSkip {
    pub original_filename: String,
    pub existing_asset_id: AssetId,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageCopySummary {
    pub storage_location_id: StorageLocationId,
    pub kind: StorageKind,
    pub label: String,
    pub copied: u32,
    pub failed: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarvestedAssetSummary {
    pub asset_id: AssetId,
    pub canonical_filename: String,
    pub category: Category,
    pub original_filename: String,
    pub duplicate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageDeleteResult {
    pub storage_location_id: StorageLocationId,
    pub kind: StorageKind,
    pub label: String,
    pub path: String,
    pub deleted: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteFromLibraryReport {
    pub asset_id: AssetId,
    pub canonical_filename: String,
    pub removed_from_db: bool,
    pub source_preserved: bool,
    pub locations: Vec<StorageDeleteResult>,
    pub errors: Vec<String>,
}
