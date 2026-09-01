export type Category =
  | "HARMONIES"
  | "RHYTHMS"
  | "TEXTURES"
  | "PERCUSSION"
  | "BASS"
  | "VOICES"
  | "FIELD_FX"
  | "GLITCH"
  | "ATMOSPHERES"
  | "OTHER";

export const CATEGORIES: { id: Category; label: string }[] = [
  { id: "HARMONIES", label: "Armonias" },
  { id: "RHYTHMS", label: "Ritmos" },
  { id: "TEXTURES", label: "Texturas" },
  { id: "PERCUSSION", label: "Percusion" },
  { id: "BASS", label: "Bajos" },
  { id: "VOICES", label: "Voces" },
  { id: "FIELD_FX", label: "Field / FX" },
  { id: "GLITCH", label: "Glitch" },
  { id: "ATMOSPHERES", label: "Atmosferas" },
  { id: "OTHER", label: "Otros" },
];

export function categoryLabel(id: Category): string {
  return CATEGORIES.find((c) => c.id === id)?.label ?? id;
}

export type StorageKind =
  | "LOCAL"
  | "DROPBOX_FOLDER"
  | "GOOGLE_DRIVE_FOLDER"
  | "CUSTOM_FOLDER";

export type SessionStatus = "ACTIVE" | "REVIEW" | "ARCHIVED" | "CANCELLED";

export interface StorageLocation {
  id: string;
  kind: StorageKind;
  label: string;
  root_path: string;
  enabled: boolean;
  created_at: string;
}

export interface AbletonSetInfo {
  als_path: string;
  project_root: string;
  project_name: string;
  consolidate_dir: string;
  consolidate_exists: boolean;
  tempo: number | null;
  tempo_source: "ALS_MANUAL" | "UNKNOWN";
}

export interface Session {
  id: string;
  project_id: string;
  project_name: string;
  ableton_set_path: string;
  project_root: string;
  consolidate_dir: string;
  start_time: string;
  end_time: string | null;
  source_session_bpm: number | null;
  status: SessionStatus;
}

export interface HarvestCandidate {
  original_path: string;
  original_filename: string;
  library_filename: string;
  relative_path: string;
  size_bytes: number;
  modified_at: string;
  change_kind: "NEW" | "MODIFIED";
  content_hash: string;
}

export interface CandidateSelection {
  original_path: string;
  selected: boolean;
  category: Category;
  library_filename_override?: string | null;
}

export interface StorageCopySummary {
  storage_location_id: string;
  kind: StorageKind;
  label: string;
  copied: number;
  failed: number;
  total: number;
}

export interface HarvestReport {
  session_id: string;
  new_assets: number;
  duplicates_skipped: number;
  failed: number;
  assets: {
    asset_id: string;
    canonical_filename: string;
    category: Category;
    original_filename: string;
    duplicate: boolean;
  }[];
  duplicates: {
    original_filename: string;
    existing_asset_id: string;
    content_hash: string;
  }[];
  storage: StorageCopySummary[];
  errors: string[];
}

export type IngestType = "SESSION_HARVEST" | "HISTORICAL_IMPORT";

export interface HistoricalConsolidate {
  original_path: string;
  original_filename: string;
  library_filename: string;
  relative_path: string;
  size_bytes: number;
  modified_at: string;
  content_hash: string;
}

export interface IgnoredConsolidateInput {
  original_path: string;
  content_hash: string;
  original_filename: string;
}

export interface ProjectLibraryStatus {
  project_id: string;
  project_name: string;
  consolidate_dir: string;
  pending: HistoricalConsolidate[];
  archived_count: number;
  synced: boolean;
}

export interface DeleteFromLibraryReport {
  asset_id: string;
  canonical_filename: string;
  removed_from_db: boolean;
  source_preserved: boolean;
  locations: {
    storage_location_id: string;
    kind: StorageKind;
    label: string;
    path: string;
    deleted: boolean;
    error: string | null;
  }[];
  errors: string[];
}

export interface UpdateLibraryAssetReport {
  asset_id: string;
  old_filename: string;
  new_filename: string;
  old_category: Category;
  new_category: Category;
  old_relative_path: string;
  new_relative_path: string;
  locations: {
    storage_location_id: string;
    kind: StorageKind;
    label: string;
    old_path: string;
    new_path: string;
    moved: boolean;
    error: string | null;
  }[];
  errors: string[];
}

export interface AudioAsset {
  id: string;
  source_type: string;
  ingest_type: IngestType;
  original_filename: string;
  original_path: string;
  canonical_filename: string;
  canonical_path: string;
  project_id: string;
  session_id: string;
  category: Category;
  year: number;
  source_session_bpm: number | null;
  detected_bpm: number | null;
  created_at: string;
  harvested_at: string;
  duration_seconds: number | null;
  sample_rate: number | null;
  channels: number | null;
  size_bytes: number;
  content_hash: string;
}

export interface Project {
  id: string;
  name: string;
  ableton_set_path: string;
  project_root: string;
}

export interface AppState {
  data_dir: string;
  db_path: string;
  last_als_path: string | null;
  active_session: Session | null;
  storage_locations: StorageLocation[];
}

export function storageKindLabel(kind: StorageKind): string {
  switch (kind) {
    case "LOCAL":
      return "Biblioteca local";
    case "DROPBOX_FOLDER":
      return "Carpeta Dropbox";
    case "GOOGLE_DRIVE_FOLDER":
      return "Carpeta Google Drive";
    case "CUSTOM_FOLDER":
      return "Carpeta personalizada";
  }
}
