import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState, type Dispatch, type SetStateAction } from "react";
import {
  archiveSession,
  abandonSession,
  deleteStorageLocation,
  deleteFromLibrary,
  endSession,
  formatBpm,
  formatDuration,
  getAppState,
  importHistorical,
  inspectAbletonSet,
  listLibrary,
  listProjects,
  pickAbletonSet,
  pickFolder,
  revealPath,
  saveStorageLocation,
  scanHistoricalConsolidates,
  setSessionBpm,
  startSession,
} from "./api";
import {
  CATEGORIES,
  categoryLabel,
  storageKindLabel,
  type AbletonSetInfo,
  type AppState,
  type AudioAsset,
  type CandidateSelection,
  type Category,
  type HarvestCandidate,
  type HarvestReport,
  type HistoricalConsolidate,
  type Project,
  type ProjectLibraryStatus,
  type StorageKind,
} from "./types";

type Screen = "home" | "historical" | "session" | "harvest" | "result" | "library";

export default function App() {
  const [screen, setScreen] = useState<Screen>("home");
  const [state, setState] = useState<AppState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const [alsPath, setAlsPath] = useState("");
  const [info, setInfo] = useState<AbletonSetInfo | null>(null);
  const [bpm, setBpm] = useState("");

  const [candidates, setCandidates] = useState<HarvestCandidate[]>([]);
  const [selections, setSelections] = useState<Record<string, CandidateSelection>>({});
  const [bulkCategory, setBulkCategory] = useState<Category>("OTHER");
  const [report, setReport] = useState<HarvestReport | null>(null);
  const [resultMode, setResultMode] = useState<"session" | "historical">("session");

  const [libraryStatus, setLibraryStatus] = useState<ProjectLibraryStatus | null>(null);
  const [historicalCandidates, setHistoricalCandidates] = useState<HistoricalConsolidate[]>([]);
  const [historicalSelections, setHistoricalSelections] = useState<Record<string, CandidateSelection>>({});
  const [historicalBulkCategory, setHistoricalBulkCategory] = useState<Category>("OTHER");

  const [assets, setAssets] = useState<AudioAsset[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [filterYear, setFilterYear] = useState("");
  const [filterCategory, setFilterCategory] = useState<Category | "">("");
  const [filterProject, setFilterProject] = useState("");

  useEffect(() => {
    void refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function scanLibrary(path: string) {
    try {
      const status = await scanHistoricalConsolidates(path);
      setLibraryStatus(status);
      return status;
    } catch {
      setLibraryStatus(null);
      return null;
    }
  }

  async function refresh() {
    try {
      const next = await getAppState();
      setState(next);
      if (next.last_als_path && !alsPath) {
        setAlsPath(next.last_als_path);
        try {
          const inspected = await inspectAbletonSet(next.last_als_path);
          setInfo(inspected);
          if (inspected.tempo != null) setBpm(String(inspected.tempo));
          await scanLibrary(next.last_als_path);
        } catch {
          /* last path may no longer exist */
        }
      }
      if (next.active_session?.status === "ACTIVE") {
        setScreen("session");
        if (next.active_session.source_session_bpm != null) {
          setBpm(String(next.active_session.source_session_bpm));
        }
      } else if (next.active_session?.status === "REVIEW") {
        setScreen("harvest");
      } else {
        setScreen((current) =>
          current === "session" || current === "harvest" ? "home" : current,
        );
      }
    } catch (e) {
      setError(String(e));
    }
  }

  async function resetToHome() {
    setBusy(true);
    setError(null);
    try {
      await abandonSession();
    } catch {
      /* UI may be ahead of DB — still reset locally */
    }
    setCandidates([]);
    setSelections({});
    try {
      const next = await getAppState();
      setState({ ...next, active_session: null });
    } catch {
      setState((prev) => (prev ? { ...prev, active_session: null } : prev));
    }
    setScreen("home");
    setBusy(false);
  }

  async function chooseAls() {
    setError(null);
    const path = await pickAbletonSet();
    if (!path) return;
    setAlsPath(path);
    try {
      const inspected = await inspectAbletonSet(path);
      setInfo(inspected);
      setBpm(inspected.tempo != null ? String(inspected.tempo) : "");
      await scanLibrary(path);
    } catch (e) {
      setError(String(e));
    }
  }

  function openHistoricalReview(status: ProjectLibraryStatus) {
    setHistoricalCandidates(status.pending);
    const next: Record<string, CandidateSelection> = {};
    for (const c of status.pending) {
      next[c.original_path] = {
        original_path: c.original_path,
        selected: true,
        category: "OTHER",
      };
    }
    setHistoricalSelections(next);
    setScreen("historical");
  }

  async function onImportHistorical() {
    if (!alsPath) return;
    setBusy(true);
    setError(null);
    try {
      const parsed = bpm.trim() === "" ? null : Number(bpm);
      const payload = Object.values(historicalSelections);
      const result = await importHistorical(alsPath, parsed, payload);
      setReport(result);
      setResultMode("historical");
      setScreen("result");
      await scanLibrary(alsPath);
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function onStart() {
    if (!alsPath) {
      setError("Elige un Ableton Set (.als).");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const parsed = bpm.trim() === "" ? null : Number(bpm);
      if (parsed != null && (!Number.isFinite(parsed) || parsed <= 0)) {
        setError("Session BPM inválido. Déjalo vacío si es desconocido.");
        return;
      }
      await startSession(alsPath, parsed);
      await refresh();
      setScreen("session");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function onEnd() {
    setBusy(true);
    setError(null);
    try {
      const result = await endSession();
      setCandidates(result.candidates);
      const next: Record<string, CandidateSelection> = {};
      for (const c of result.candidates) {
        next[c.original_path] = {
          original_path: c.original_path,
          selected: true,
          category: "OTHER",
        };
      }
      setSelections(next);
      setScreen("harvest");
    } catch (e) {
      const msg = String(e);
      if (msg.includes("No hay una sesión activa")) {
        await resetToHome();
      } else {
        setError(msg);
      }
    } finally {
      setBusy(false);
    }
  }

  async function onArchive() {
    if (!state?.active_session) return;
    setBusy(true);
    setError(null);
    try {
      const payload = Object.values(selections);
      const result = await archiveSession(state.active_session.id, payload);
      setReport(result);
      setResultMode("session");
      setScreen("result");
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function openLibrary() {
    setScreen("library");
    try {
      const [list, projs] = await Promise.all([
        listLibrary({
          year: filterYear ? Number(filterYear) : null,
          category: filterCategory || null,
          projectId: filterProject || null,
        }),
        listProjects(),
      ]);
      setAssets(list);
      setProjects(projs);
    } catch (e) {
      setError(String(e));
    }
  }

  useEffect(() => {
    if (screen === "library") {
      void openLibrary();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filterYear, filterCategory, filterProject, screen]);

  const local = state?.storage_locations.find((l) => l.kind === "LOCAL" && l.enabled);

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">AUDIONÁUTICA</div>
        <nav className="nav">
          <button
            className={screen === "home" || screen === "historical" || screen === "session" || screen === "harvest" || screen === "result" ? "active" : ""}
            onClick={() => setScreen(state?.active_session?.status === "ACTIVE" ? "session" : "home")}
          >
            Sesión
          </button>
          <button className={screen === "library" ? "active" : ""} onClick={() => void openLibrary()}>
            Biblioteca
          </button>
        </nav>
      </header>
      <main className="layout">
        {error ? <div className="error-banner">{error}</div> : null}
        {screen === "home" ? (
          <Home
            alsPath={alsPath}
            info={info}
            bpm={bpm}
            setBpm={setBpm}
            state={state}
            libraryStatus={libraryStatus}
            busy={busy}
            onChooseAls={() => void chooseAls()}
            onStart={() => void onStart()}
            onReviewHistorical={() => {
              if (libraryStatus) openHistoricalReview(libraryStatus);
            }}
            onPickStorage={async (kind, label) => {
              const folder = await pickFolder();
              if (!folder) return;
              try {
                const existing = state?.storage_locations.find((l) => l.kind === kind);
                const next = await saveStorageLocation({
                  id: existing?.id ?? null,
                  kind,
                  label,
                  rootPath: folder,
                  enabled: true,
                });
                setState(next);
              } catch (e) {
                setError(String(e));
              }
            }}
            onClearStorage={async (id) => {
              try {
                setState(await deleteStorageLocation(id));
              } catch (e) {
                setError(String(e));
              }
            }}
          />
        ) : null}
        {screen === "historical" ? (
          <HistoricalImportView
            candidates={historicalCandidates}
            selections={historicalSelections}
            setSelections={setHistoricalSelections}
            bulkCategory={historicalBulkCategory}
            setBulkCategory={setHistoricalBulkCategory}
            busy={busy}
            onImport={() => void onImportHistorical()}
            onBack={() => setScreen("home")}
          />
        ) : null}
        {screen === "session" && state?.active_session ? (
          <SessionView
            session={state.active_session}
            bpm={bpm}
            setBpm={setBpm}
            busy={busy}
            onSaveBpm={async () => {
              const parsed = bpm.trim() === "" ? null : Number(bpm);
              try {
                await setSessionBpm(state.active_session!.id, parsed);
              } catch (e) {
                setError(String(e));
              }
            }}
            onEnd={() => void onEnd()}
            onReset={() => void resetToHome()}
          />
        ) : null}
        {screen === "session" && !state?.active_session ? (
          <section>
            <h1>Sesión desincronizada</h1>
            <p className="lede">
              La interfaz muestra una sesión que ya no existe en la base de datos. Puedes volver al inicio y
              empezar de cero.
            </p>
            <div className="actions">
              <button className="btn primary" disabled={busy} onClick={() => void resetToHome()}>
                VOLVER AL INICIO
              </button>
            </div>
          </section>
        ) : null}
        {screen === "harvest" ? (
          <HarvestView
            candidates={candidates}
            selections={selections}
            setSelections={setSelections}
            bulkCategory={bulkCategory}
            setBulkCategory={setBulkCategory}
            busy={busy}
            onArchive={() => void onArchive()}
            onReset={() => void resetToHome()}
          />
        ) : null}
        {screen === "result" && report ? (
          <ResultView
            report={report}
            mode={resultMode}
            localOk={Boolean(local)}
            onLibrary={() => void openLibrary()}
            onHome={() => {
              setReport(null);
              setScreen("home");
              void refresh();
            }}
            onOpenLocal={() => {
              if (local) void revealPath(local.root_path);
            }}
          />
        ) : null}
        {screen === "library" ? (
          <LibraryView
            assets={assets}
            projects={projects}
            filterYear={filterYear}
            filterCategory={filterCategory}
            filterProject={filterProject}
            setFilterYear={setFilterYear}
            setFilterCategory={setFilterCategory}
            setFilterProject={setFilterProject}
            onDelete={async (assetId) => {
              setBusy(true);
              setError(null);
              try {
                const report = await deleteFromLibrary(assetId);
                if (report.errors.length) {
                  setError(report.errors.join(" · "));
                }
                await openLibrary();
              } catch (e) {
                setError(String(e));
              } finally {
                setBusy(false);
              }
            }}
            busy={busy}
          />
        ) : null}
      </main>
    </div>
  );
}

function Home(props: {
  alsPath: string;
  info: AbletonSetInfo | null;
  bpm: string;
  setBpm: (v: string) => void;
  state: AppState | null;
  libraryStatus: ProjectLibraryStatus | null;
  busy: boolean;
  onChooseAls: () => void;
  onStart: () => void;
  onReviewHistorical: () => void;
  onPickStorage: (kind: StorageKind, label: string) => void;
  onClearStorage: (id: string) => void;
}) {
  const locations = props.state?.storage_locations ?? [];
  const local = locations.find((l) => l.kind === "LOCAL");
  const dropbox = locations.find((l) => l.kind === "DROPBOX_FOLDER");
  const drive = locations.find((l) => l.kind === "GOOGLE_DRIVE_FOLDER");

  return (
    <section>
      <h1>Sesión</h1>
      <p className="lede">Ableton → Consolidate → biblioteca de loops. Las fuentes nunca se modifican.</p>
      <div className="grid">
        <div className="card">
          <h2>Ableton Set</h2>
          <div className="row">
            <div className="grow path">{props.alsPath || "Ningún .als seleccionado"}</div>
            <button className="btn" onClick={props.onChooseAls}>
              Elegir .als
            </button>
          </div>
          {props.info ? (
            <p className="muted">
              Project <b>{props.info.project_name}</b>
              {" · "}
              Consolidate {props.info.consolidate_exists ? "encontrada" : "aún no existe (ok)"}
            </p>
          ) : null}
        </div>
        <div className="card">
          <h2>Project / Session BPM</h2>
          <div className="row">
            <label className="field grow">
              Project
              <input type="text" readOnly value={props.info?.project_name ?? "—"} />
            </label>
            <label className="field" style={{ width: 140 }}>
              Session BPM
              <input
                type="number"
                min={1}
                step={0.1}
                placeholder="—"
                value={props.bpm}
                onChange={(e) => props.setBpm(e.target.value)}
              />
            </label>
          </div>
          <p className="muted">
            {props.info?.tempo != null
              ? `Tempo leído del set: ${props.info.tempo}. Puedes corregirlo.`
              : "BPM desconocido — no se inventa. Puedes escribirlo a mano o dejarlo vacío (BPMUNK)."}
          </p>
        </div>
        <div className="card">
          <h2>Library status</h2>
          {!props.alsPath ? (
            <p className="muted">Elige un proyecto Ableton para revisar consolidates existentes.</p>
          ) : props.libraryStatus?.synced ? (
            <>
              <p className="ok">✓ Proyecto sincronizado</p>
              <p className="muted">
                {props.libraryStatus.archived_count > 0
                  ? `${props.libraryStatus.archived_count} consolidate${props.libraryStatus.archived_count === 1 ? "" : "s"} en biblioteca · 0 pendientes`
                  : "0 consolidates pendientes"}
              </p>
            </>
          ) : props.libraryStatus && props.libraryStatus.pending.length > 0 ? (
            <>
              <p>
                <b>{props.libraryStatus.pending.length}</b> consolidate
                {props.libraryStatus.pending.length === 1 ? "" : "s"} existente
                {props.libraryStatus.pending.length === 1 ? "" : "s"}
              </p>
              <p className="muted">Estos archivos todavía no están en tu biblioteca de Audionáutica.</p>
              <div className="actions" style={{ marginTop: 12 }}>
                <button className="btn primary" onClick={props.onReviewHistorical}>
                  REVISAR E IMPORTAR {props.libraryStatus.pending.length}
                </button>
              </div>
            </>
          ) : (
            <p className="muted">Escaneando biblioteca…</p>
          )}
        </div>
        <div className="card">
          <h2>Storage</h2>
          <StorageRow
            kind="LOCAL"
            location={local}
            onPick={() => props.onPickStorage("LOCAL", "Biblioteca local")}
            onClear={props.onClearStorage}
          />
          <StorageRow
            kind="DROPBOX_FOLDER"
            location={dropbox}
            onPick={() => props.onPickStorage("DROPBOX_FOLDER", "Carpeta Dropbox")}
            onClear={props.onClearStorage}
          />
          <StorageRow
            kind="GOOGLE_DRIVE_FOLDER"
            location={drive}
            onPick={() => props.onPickStorage("GOOGLE_DRIVE_FOLDER", "Carpeta Google Drive")}
            onClear={props.onClearStorage}
          />
          <p className="muted">
            Dropbox y Drive son carpetas locales sincronizadas. Audionáutica copia archivos; no sube a la nube por API.
          </p>
        </div>
        <div className="actions">
          <button className="btn primary" disabled={props.busy || !props.alsPath || !local} onClick={props.onStart}>
            START SESSION
          </button>
        </div>
      </div>
    </section>
  );
}

function StorageRow(props: {
  kind: StorageKind;
  location?: { id: string; root_path: string; enabled: boolean };
  onPick: () => void;
  onClear: (id: string) => void;
}) {
  const ok = Boolean(props.location?.enabled);
  return (
    <div className="row" style={{ marginBottom: 8 }}>
      <span style={{ width: 180 }} className={ok ? "ok" : "muted"}>
        {ok ? "✓" : "○"} {storageKindLabel(props.kind)}
      </span>
      <span className="grow path">{props.location?.root_path ?? "—"}</span>
      <button className="btn" onClick={props.onPick}>
        Elegir carpeta
      </button>
      {props.location ? (
        <button className="btn" onClick={() => props.onClear(props.location!.id)}>
          Quitar
        </button>
      ) : null}
    </div>
  );
}

function SessionView(props: {
  session: NonNullable<AppState["active_session"]>;
  bpm: string;
  setBpm: (v: string) => void;
  busy: boolean;
  onSaveBpm: () => void;
  onEnd: () => void;
  onReset: () => void;
}) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, []);
  const elapsed = useMemo(() => {
    const start = new Date(props.session.start_time).getTime();
    const ms = Math.max(0, now - start);
    const s = Math.floor(ms / 1000);
    const hh = String(Math.floor(s / 3600)).padStart(2, "0");
    const mm = String(Math.floor((s % 3600) / 60)).padStart(2, "0");
    const ss = String(s % 60).padStart(2, "0");
    return `${hh}:${mm}:${ss}`;
  }, [now, props.session.start_time]);

  return (
    <section>
      <h1>SESSION ACTIVE</h1>
      <div className="big-timer">{elapsed}</div>
      <div className="card">
        <h2>{props.session.project_name}</h2>
        <div className="row">
          <label className="field" style={{ width: 160 }}>
            Session BPM
            <input
              type="number"
              value={props.bpm}
              onChange={(e) => props.setBpm(e.target.value)}
              onBlur={props.onSaveBpm}
            />
          </label>
          <span className="muted">Waiting for consolidates…</span>
        </div>
        <p className="muted path">{props.session.consolidate_dir}</p>
      </div>
      <div className="actions">
        <button className="btn primary" disabled={props.busy} onClick={props.onEnd}>
          END SESSION
        </button>
        <button className="btn" disabled={props.busy} onClick={props.onReset}>
          Cancelar y volver al inicio
        </button>
      </div>
    </section>
  );
}

function HistoricalImportView(props: {
  candidates: HistoricalConsolidate[];
  selections: Record<string, CandidateSelection>;
  setSelections: Dispatch<SetStateAction<Record<string, CandidateSelection>>>;
  bulkCategory: Category;
  setBulkCategory: (c: Category) => void;
  busy: boolean;
  onImport: () => void;
  onBack: () => void;
}) {
  const selectedCount = Object.values(props.selections).filter((s) => s.selected).length;
  function patch(path: string, partial: Partial<CandidateSelection>) {
    props.setSelections((prev) => {
      const cur = prev[path];
      if (!cur) return prev;
      return { ...prev, [path]: { ...cur, ...partial } };
    });
  }
  function selectAll(selected: boolean) {
    props.setSelections((prev) => {
      const next = { ...prev };
      for (const key of Object.keys(next)) {
        const item = next[key];
        if (item) next[key] = { ...item, selected };
      }
      return next;
    });
  }
  function applyBulk() {
    props.setSelections((prev) => {
      const next = { ...prev };
      for (const key of Object.keys(next)) {
        const item = next[key];
        if (item?.selected) next[key] = { ...item, category: props.bulkCategory };
      }
      return next;
    });
  }

  return (
    <section>
      <h1>HISTORICAL IMPORT</h1>
      <p className="lede">
        {props.candidates.length} consolidate{props.candidates.length === 1 ? "" : "s"} existente
        {props.candidates.length === 1 ? "" : "s"} sin archivar
      </p>
      <div className="row" style={{ marginBottom: 14 }}>
        <button className="btn" onClick={() => selectAll(true)}>
          Select all
        </button>
        <button className="btn" onClick={() => selectAll(false)}>
          Deselect
        </button>
        <select
          value={props.bulkCategory}
          onChange={(e) => props.setBulkCategory(e.target.value as Category)}
          style={{ width: 180 }}
        >
          {CATEGORIES.map((c) => (
            <option key={c.id} value={c.id}>
              {c.label}
            </option>
          ))}
        </select>
        <button className="btn" onClick={applyBulk}>
          Asignar categoría
        </button>
      </div>
      <div className="harvest-list">
        {props.candidates.map((c) => {
          const sel = props.selections[c.original_path];
          return (
            <div className="harvest-item" key={c.original_path}>
              <input
                type="checkbox"
                checked={sel?.selected ?? false}
                onChange={(e) => patch(c.original_path, { selected: e.target.checked })}
              />
              <div>
                <div>{c.original_filename}</div>
                <div className="muted">histórico · {(c.size_bytes / 1024).toFixed(1)} KB</div>
              </div>
              <select
                value={sel?.category ?? "OTHER"}
                onChange={(e) => patch(c.original_path, { category: e.target.value as Category })}
              >
                {CATEGORIES.map((cat) => (
                  <option key={cat.id} value={cat.id}>
                    {cat.label}
                  </option>
                ))}
              </select>
            </div>
          );
        })}
      </div>
      <div className="actions">
        <button className="btn" onClick={props.onBack}>
          Volver
        </button>
        <button className="btn primary" disabled={props.busy || selectedCount === 0} onClick={props.onImport}>
          IMPORTAR {selectedCount}
        </button>
      </div>
    </section>
  );
}

function HarvestView(props: {
  candidates: HarvestCandidate[];
  selections: Record<string, CandidateSelection>;
  setSelections: Dispatch<SetStateAction<Record<string, CandidateSelection>>>;
  bulkCategory: Category;
  setBulkCategory: (c: Category) => void;
  busy: boolean;
  onArchive: () => void;
  onReset: () => void;
}) {
  const selectedCount = Object.values(props.selections).filter((s) => s.selected).length;
  function patch(path: string, partial: Partial<CandidateSelection>) {
    props.setSelections((prev) => {
      const cur = prev[path];
      if (!cur) return prev;
      return { ...prev, [path]: { ...cur, ...partial } };
    });
  }
  function selectAll(selected: boolean) {
    props.setSelections((prev) => {
      const next = { ...prev };
      for (const key of Object.keys(next)) {
        const item = next[key];
        if (item) next[key] = { ...item, selected };
      }
      return next;
    });
  }
  function applyBulk() {
    props.setSelections((prev) => {
      const next = { ...prev };
      for (const key of Object.keys(next)) {
        const item = next[key];
        if (item?.selected) next[key] = { ...item, category: props.bulkCategory };
      }
      return next;
    });
  }

  return (
    <section>
      <h1>SESSION HARVEST</h1>
      <p className="lede">
        {props.candidates.length} loop{props.candidates.length === 1 ? "" : "s"} nuevo
        {props.candidates.length === 1 ? "" : "s"}
      </p>
      <div className="row" style={{ marginBottom: 14 }}>
        <button className="btn" onClick={() => selectAll(true)}>
          Select all
        </button>
        <button className="btn" onClick={() => selectAll(false)}>
          Deselect
        </button>
        <select
          value={props.bulkCategory}
          onChange={(e) => props.setBulkCategory(e.target.value as Category)}
          style={{ width: 180 }}
        >
          {CATEGORIES.map((c) => (
            <option key={c.id} value={c.id}>
              {c.label}
            </option>
          ))}
        </select>
        <button className="btn" onClick={applyBulk}>
          Asignar categoría
        </button>
      </div>
      <div className="harvest-list">
        {props.candidates.map((c) => {
          const sel = props.selections[c.original_path];
          return (
            <div className="harvest-item" key={c.original_path}>
              <input
                type="checkbox"
                checked={sel?.selected ?? false}
                onChange={(e) => patch(c.original_path, { selected: e.target.checked })}
              />
              <div>
                <div>{c.original_filename}</div>
                <div className="muted">
                  {c.change_kind === "NEW" ? "nuevo" : "modificado"} · {(c.size_bytes / 1024).toFixed(1)} KB
                </div>
              </div>
              <select
                value={sel?.category ?? "OTHER"}
                onChange={(e) => patch(c.original_path, { category: e.target.value as Category })}
              >
                {CATEGORIES.map((cat) => (
                  <option key={cat.id} value={cat.id}>
                    {cat.label}
                  </option>
                ))}
              </select>
            </div>
          );
        })}
        {props.candidates.length === 0 ? (
          <p className="muted">No hay consolidates nuevos en esta sesión.</p>
        ) : null}
      </div>
      <div className="actions">
        <button className="btn primary" disabled={props.busy} onClick={props.onArchive}>
          ARCHIVE {selectedCount} LOOP{selectedCount === 1 ? "" : "S"}
        </button>
        <button className="btn" disabled={props.busy} onClick={props.onReset}>
          Cancelar y volver al inicio
        </button>
      </div>
    </section>
  );
}

function ResultView(props: {
  report: HarvestReport;
  mode: "session" | "historical";
  localOk: boolean;
  onLibrary: () => void;
  onHome: () => void;
  onOpenLocal: () => void;
}) {
  return (
    <section>
      <h1>{props.mode === "historical" ? "HISTORICAL IMPORTED" : "SESSION ARCHIVED"}</h1>
      <div className="stats">
        <div className="stat">
          nuevos
          <b>{props.report.new_assets}</b>
        </div>
        <div className="stat">
          duplicados omitidos
          <b>{props.report.duplicates_skipped}</b>
        </div>
        <div className="stat">
          fallidos
          <b>{props.report.failed}</b>
        </div>
      </div>
      <div className="card" style={{ marginTop: 16 }}>
        <h2>Destinos (filesystem local)</h2>
        <ul className="status-list">
          {props.report.storage.map((s) => (
            <li key={s.storage_location_id}>
              {s.failed === 0 ? "✓" : "✗"} {storageKindLabel(s.kind)} {s.copied}/{s.total}
              {s.failed ? ` · ${s.failed} fallidos` : ""}
            </li>
          ))}
        </ul>
        <p className="muted">Esto confirma copia a carpeta, no subida a la nube.</p>
      </div>
      {props.report.errors.length ? (
        <div className="card">
          <h2>Errores</h2>
          {props.report.errors.map((e) => (
            <p key={e} className="danger">
              {e}
            </p>
          ))}
        </div>
      ) : null}
      <div className="actions">
        <button className="btn primary" onClick={props.onLibrary}>
          OPEN LIBRARY
        </button>
        {props.localOk ? (
          <button className="btn" onClick={props.onOpenLocal}>
            Abrir carpeta local
          </button>
        ) : null}
        <button className="btn" onClick={props.onHome}>
          Nueva sesión
        </button>
      </div>
    </section>
  );
}

function LibraryView(props: {
  assets: AudioAsset[];
  projects: Project[];
  filterYear: string;
  filterCategory: Category | "";
  filterProject: string;
  setFilterYear: (v: string) => void;
  setFilterCategory: (v: Category | "") => void;
  setFilterProject: (v: string) => void;
  onDelete: (assetId: string) => void;
  busy: boolean;
}) {
  return (
    <section>
      <h1>LIBRARY</h1>
      <p className="lede">AudioAssets persistidos en SQLite. La carpeta es una proyección, no la fuente de verdad.</p>
      <div className="filters">
        <label className="field" style={{ width: 120 }}>
          Año
          <input
            type="number"
            value={props.filterYear}
            placeholder="todos"
            onChange={(e) => props.setFilterYear(e.target.value)}
          />
        </label>
        <label className="field" style={{ width: 180 }}>
          Categoría
          <select
            value={props.filterCategory}
            onChange={(e) => props.setFilterCategory((e.target.value || "") as Category | "")}
          >
            <option value="">Todas</option>
            {CATEGORIES.map((c) => (
              <option key={c.id} value={c.id}>
                {c.label}
              </option>
            ))}
          </select>
        </label>
        <label className="field" style={{ width: 220 }}>
          Project
          <select value={props.filterProject} onChange={(e) => props.setFilterProject(e.target.value)}>
            <option value="">Todos</option>
            {props.projects.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
        </label>
      </div>
      <table>
        <thead>
          <tr>
            <th>Archivo</th>
            <th>Categoría</th>
            <th>BPM</th>
            <th>Project</th>
            <th>Fecha</th>
            <th>Duración</th>
            <th>Preview</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {props.assets.map((a) => (
            <tr key={a.id}>
              <td className="path">{a.canonical_filename}</td>
              <td>{categoryLabel(a.category)}</td>
              <td>{formatBpm(a.source_session_bpm)}</td>
              <td>{props.projects.find((p) => p.id === a.project_id)?.name ?? "—"}</td>
              <td>{a.harvested_at.slice(0, 10)}</td>
              <td>{formatDuration(a.duration_seconds)}</td>
              <td>
                <audio controls preload="none" src={convertFileSrc(a.canonical_path)} />
              </td>
              <td>
                <button
                  className="btn danger"
                  disabled={props.busy}
                  onClick={() => {
                    const ok = window.confirm(
                      `¿Eliminar "${a.canonical_filename}" de la biblioteca?\n\nSe borrarán las copias en Local/Drive/Dropbox.\nEl Consolidate original en Ableton NO se toca.`,
                    );
                    if (ok) props.onDelete(a.id);
                  }}
                >
                  Eliminar
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {props.assets.length === 0 ? <p className="muted">No hay assets todavía.</p> : null}
    </section>
  );
}
