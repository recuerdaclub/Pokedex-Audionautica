import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  AbletonSetInfo,
  AppState,
  AudioAsset,
  CandidateSelection,
  Category,
  DeleteFromLibraryReport,
  HarvestCandidate,
  HarvestReport,
  IgnoredConsolidateInput,
  Project,
  ProjectLibraryStatus,
  Session,
  StorageKind,
  UpdateLibraryAssetReport,
} from "./types";

export async function getAppState(): Promise<AppState> {
  return invoke("get_app_state");
}

export async function inspectAbletonSet(path: string): Promise<AbletonSetInfo> {
  return invoke("inspect_ableton_set", { path });
}

export async function scanHistoricalConsolidates(
  alsPath: string,
): Promise<ProjectLibraryStatus> {
  return invoke("scan_historical_consolidates", { alsPath });
}

export async function importHistorical(
  alsPath: string,
  bpmOverride: number | null,
  selections: CandidateSelection[],
): Promise<HarvestReport> {
  return invoke("import_historical", { alsPath, bpmOverride, selections });
}

export async function abandonSession(): Promise<boolean> {
  return invoke("abandon_session");
}

export async function startSession(
  alsPath: string,
  bpmOverride: number | null,
): Promise<Session> {
  return invoke("start_session", { alsPath, bpmOverride });
}

export async function endSession(): Promise<{
  session: Session;
  candidates: HarvestCandidate[];
}> {
  return invoke("end_session");
}

export async function archiveSession(
  sessionId: string,
  selections: CandidateSelection[],
): Promise<HarvestReport> {
  return invoke("archive_session", { sessionId, selections });
}

export async function setSessionBpm(
  sessionId: string,
  bpm: number | null,
): Promise<Session> {
  return invoke("set_session_bpm", { sessionId, bpm });
}

export async function saveStorageLocation(input: {
  id: string | null;
  kind: StorageKind;
  label: string;
  rootPath: string;
  enabled: boolean;
}): Promise<AppState> {
  return invoke("save_storage_location", input);
}

export async function deleteStorageLocation(id: string): Promise<AppState> {
  return invoke("delete_storage_location", { id });
}

export async function ignoreConsolidates(
  projectId: string,
  items: IgnoredConsolidateInput[],
): Promise<number> {
  return invoke("ignore_consolidates", { projectId, items });
}

export async function deleteFromLibrary(assetId: string): Promise<DeleteFromLibraryReport> {
  return invoke("delete_from_library", { assetId });
}

export async function updateLibraryAsset(
  assetId: string,
  newCategory: Category | null,
  newFilename: string | null,
): Promise<UpdateLibraryAssetReport> {
  return invoke("update_library_asset", {
    assetId,
    newCategory,
    newFilename,
  });
}

export async function listLibrary(filter: {
  year: number | null;
  category: Category | null;
  projectId: string | null;
}): Promise<AudioAsset[]> {
  return invoke("list_library", filter);
}

export async function listProjects(): Promise<Project[]> {
  return invoke("list_projects");
}

export async function pickAbletonSet(): Promise<string | null> {
  const selected = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "Ableton Set", extensions: ["als"] }],
  });
  if (!selected || Array.isArray(selected)) return null;
  return selected;
}

export async function pickFolder(): Promise<string | null> {
  const selected = await open({ directory: true, multiple: false });
  if (!selected || Array.isArray(selected)) return null;
  return selected;
}

export async function revealPath(path: string): Promise<void> {
  await invoke("reveal_path", { path });
}

export function formatDuration(seconds: number | null): string {
  if (seconds == null || Number.isNaN(seconds)) return "—";
  const s = Math.round(seconds);
  const m = Math.floor(s / 60);
  const r = s % 60;
  return `${m}:${r.toString().padStart(2, "0")}`;
}

export function formatBpm(bpm: number | null): string {
  if (bpm == null) return "—";
  return Number.isInteger(bpm) ? String(bpm) : bpm.toFixed(1);
}
