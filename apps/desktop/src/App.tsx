import { useEffect, useMemo, useState } from "react";
import { readStoredTheme, THEME_STORAGE_KEY, THEMES, type ThemeId } from "./theme";
import {
  archiveSession,
  abandonSession,
  deleteStorageLocation,
  deleteFromLibrary,
  updateLibraryAsset,
  endSession,
  formatBpm,
  formatDuration,
  getAppState,
  ignoreConsolidates,
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
} from "./api";
import { ReviewScreen } from "./ReviewScreen";
import { LibraryAudioPlayer } from "./LibraryAudioPlayer";
import {
  CATEGORIES,
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
  const [, setInfo] = useState<AbletonSetInfo | null>(null);
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
  const [theme, setTheme] = useState<ThemeId>(() => readStoredTheme());

  useEffect(() => {
    void refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
    try {
      localStorage.setItem(THEME_STORAGE_KEY, theme);
    } catch {
      /* ignore */
    }
  }, [theme]);

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
      if (libraryStatus) {
        const unselected = historicalCandidates.filter(
          (c) => !historicalSelections[c.original_path]?.selected,
        );
        const ignoreItems = unselected
          .filter((c) => Boolean(c.content_hash))
          .map((c) => ({
            original_path: c.original_path,
            original_filename: c.original_filename,
            content_hash: c.content_hash as string,
          }));
        if (ignoreItems.length) {
          await ignoreConsolidates(libraryStatus.project_id, ignoreItems);
        }
      }
      const payload = Object.values(historicalSelections).filter((s) => s.selected);
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
      const unselected = candidates.filter((c) => !selections[c.original_path]?.selected);
      const ignoreItems = unselected
        .filter((c) => Boolean(c.content_hash))
        .map((c) => ({
          original_path: c.original_path,
          original_filename: c.original_filename,
          content_hash: c.content_hash as string,
        }));
      if (ignoreItems.length) {
        await ignoreConsolidates(state.active_session.project_id, ignoreItems);
      }
      const payload = Object.values(selections).filter((s) => s.selected);
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
      <div className="pokedex">
        <header className="pokedex-lid">
          <div className="pokedex-lens" aria-hidden="true">
            <span className="pokedex-lens-inner" />
          </div>
          <div className="pokedex-lid-main">
            <div className="pokedex-lid-top">
              <div className="brand">Pokedex Audionautica</div>
              <div className="pokedex-lights" aria-hidden="true">
                <span className={`pokedex-light red${error ? " on" : ""}`} />
                <span className={`pokedex-light yellow${busy ? " on" : ""}`} />
                <span className={`pokedex-light green${!error && !busy ? " on" : ""}`} />
              </div>
            </div>
            <nav className="nav">
              <button
                className={
                  screen === "home" ||
                  screen === "historical" ||
                  screen === "session" ||
                  screen === "harvest" ||
                  screen === "result"
                    ? "active"
                    : ""
                }
                onClick={() => setScreen(state?.active_session?.status === "ACTIVE" ? "session" : "home")}
              >
                SESION
              </button>
              <button className={screen === "library" ? "active" : ""} onClick={() => void openLibrary()}>
                BIBLIOTECA
              </button>
            </nav>
          </div>
        </header>
        <div className="pokedex-hinge" aria-hidden="true" />
        <div className="pokedex-bezel">
          <div className="pokedex-screen">
            <main className="layout">
        {error ? <div className="error-banner">{error}</div> : null}
        {screen === "home" ? (
          <Home
            alsPath={alsPath}
            state={state}
            libraryStatus={libraryStatus}
            busy={busy}
            onChooseAls={() => void chooseAls()}
            onReviewHistorical={() => {
              if (libraryStatus) openHistoricalReview(libraryStatus);
            }}
            onPickStorage={async (kind, label) => {
              const folder = await pickFolder();
              if (!folder) return;
              setBusy(true);
              setError(null);
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
              } finally {
                setBusy(false);
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
          <ReviewScreen
            title="LOOPS POR FARMEAR"
            lede="Atrapalos a todos!"
            rows={historicalCandidates.map((c) => ({
              key: c.original_path,
              originalPath: c.original_path,
              originalFilename: c.original_filename,
              libraryFilename: c.library_filename,
              sizeBytes: c.size_bytes,
              subtitle: "histórico",
              contentHash: c.content_hash,
            }))}
            selections={historicalSelections}
            setSelections={setHistoricalSelections}
            bulkCategory={historicalBulkCategory}
            setBulkCategory={setHistoricalBulkCategory}
            busy={busy}
            primaryLabel="FARMEAR"
            onPrimary={() => void onImportHistorical()}
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
            <h1>SESION DESINCRONIZADA</h1>
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
          <ReviewScreen
            title="LOOPS POR FARMEAR"
            lede={`${candidates.length} loop${candidates.length === 1 ? "" : "s"} nuevo${candidates.length === 1 ? "" : "s"} en esta sesion`}
            rows={candidates.map((c) => ({
              key: c.original_path,
              originalPath: c.original_path,
              originalFilename: c.original_filename,
              libraryFilename: c.library_filename,
              sizeBytes: c.size_bytes,
              subtitle: c.change_kind === "NEW" ? "nuevo" : "modificado",
              contentHash: c.content_hash,
            }))}
            selections={selections}
            setSelections={setSelections}
            bulkCategory={bulkCategory}
            setBulkCategory={setBulkCategory}
            busy={busy}
            primaryLabel="FARMEAR"
            onPrimary={() => void onArchive()}
            onBack={() => void resetToHome()}
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
            onOpenLocal={async () => {
              if (!local) return;
              try {
                await revealPath(local.root_path);
              } catch (e) {
                setError(String(e));
              }
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
            onSave={async (assetId, category, filename) => {
              setBusy(true);
              setError(null);
              try {
                const asset = assets.find((a) => a.id === assetId);
                if (!asset) return;
                const newCategory = category !== asset.category ? category : null;
                const newFilename = filename !== asset.canonical_filename ? filename : null;
                if (!newCategory && !newFilename) return;
                const report = await updateLibraryAsset(assetId, newCategory, newFilename);
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
          <div className="pokedex-vents" aria-hidden="true">
            <span className="pokedex-led" />
            <span className="pokedex-grill" />
            <span className="pokedex-grill" />
          </div>
        </div>
        <footer className="pokedex-deck">
          <div className="pokedex-deck-left" aria-hidden="true">
            <span className="pokedex-circle-btn" />
            <span className="pokedex-pills">
              <i className="pill red" />
              <i className="pill blue" />
            </span>
            <span className="pokedex-dpad" />
          </div>
          <div className="theme-dots" role="radiogroup" aria-label="Paleta de color">
            {THEMES.map((item) => (
              <button
                key={item.id}
                type="button"
                role="radio"
                aria-checked={theme === item.id}
                aria-label={item.label}
                title={item.label}
                className={`theme-dot theme-dot-${item.id}${theme === item.id ? " selected" : ""}`}
                style={{ background: item.swatch }}
                onClick={() => setTheme(item.id)}
              />
            ))}
          </div>
        </footer>
      </div>
    </div>
  );
}

function Home(props: {
  alsPath: string;
  state: AppState | null;
  libraryStatus: ProjectLibraryStatus | null;
  busy: boolean;
  onChooseAls: () => void;
  onReviewHistorical: () => void;
  onPickStorage: (kind: StorageKind, label: string) => void;
  onClearStorage: (id: string) => void;
}) {
  const locations = props.state?.storage_locations ?? [];
  const local = locations.find((l) => l.kind === "LOCAL" && l.enabled);
  const dropbox = locations.find((l) => l.kind === "DROPBOX_FOLDER" && l.enabled);
  const drive = locations.find((l) => l.kind === "GOOGLE_DRIVE_FOLDER" && l.enabled);
  const pendingCount = props.libraryStatus?.pending.length ?? 0;
  const canReview = Boolean(props.alsPath && local && drive && pendingCount > 0);
  const foundCount = props.libraryStatus?.synced
    ? props.libraryStatus.archived_count
    : pendingCount;
  const loopsLabel =
    foundCount === 1 ? "1 LOOP Encontrado" : `${foundCount} LOOPS Encontrados`;

  return (
    <section className="screen-home">
      <div className="screen-header">
        <h1>Farmeador de Loops</h1>
      </div>
      <div className="screen-body">
        <div className="grid">
        <div className="card">
          <h2>analiza el live set</h2>
          <div className="row">
            <div className="grow path">{props.alsPath || "Ningún .als seleccionado"}</div>
            <button className="btn primary" onClick={props.onChooseAls}>
              Elegir .als
            </button>
          </div>
        </div>
        <div className="card library-status-card">
          <h2>Library status</h2>
          <div className="library-status-layout">
            <div className="library-status-copy">
              {!props.alsPath ? (
                <p className="muted">Elige un proyecto Ableton para revisar consolidates existentes.</p>
              ) : !local || !drive ? (
                <p className="warn">
                  Para farmear necesitas carpeta local y Google Drive. Dropbox se puede agregar después y se
                  sincroniza solo.
                </p>
              ) : props.libraryStatus ? (
                <p>
                  <b>{loopsLabel}</b>
                </p>
              ) : (
                <p className="muted">Escaneando biblioteca…</p>
              )}
            </div>
            {props.alsPath ? (
              <button
                type="button"
                className="btn primary library-status-action"
                disabled={props.busy || !canReview}
                onClick={props.onReviewHistorical}
              >
                REVISAR Y FARMEAR
              </button>
            ) : null}
          </div>
        </div>
        <div className="card">
          <h2>registros audionauticos</h2>
          <StorageRow
            kind="LOCAL"
            location={local}
            onPick={() => props.onPickStorage("LOCAL", "Biblioteca local")}
            onClear={props.onClearStorage}
          />
          <StorageRow
            kind="GOOGLE_DRIVE_FOLDER"
            location={drive}
            onPick={() => props.onPickStorage("GOOGLE_DRIVE_FOLDER", "Carpeta Google Drive")}
            onClear={props.onClearStorage}
          />
          <StorageRow
            kind="DROPBOX_FOLDER"
            location={dropbox}
            onPick={() => props.onPickStorage("DROPBOX_FOLDER", "Carpeta Dropbox")}
            onClear={props.onClearStorage}
          />
        </div>
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
          Nueva sesion
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
  onSave: (assetId: string, category: Category, filename: string) => void;
  busy: boolean;
}) {
  return (
    <section className="screen-library">
      <div className="screen-header">
        <h1>LOOP POKEDEX</h1>
      </div>
      <div className="screen-body">
        <div className="library-panel card">
          <div className="filters">
            <label className="field filter-year">
              Ano
              <input
                type="number"
                value={props.filterYear}
                placeholder="todos"
                onChange={(e) => props.setFilterYear(e.target.value)}
              />
            </label>
            <label className="field filter-category">
              Categoria
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
            <label className="field filter-project">
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
          <div className="table-wrap">
            <table className="library-table">
              <thead>
                <tr>
                  <th>Archivo</th>
                  <th>Categoria</th>
                  <th>BPM</th>
                  <th>Project</th>
                  <th>Fecha</th>
                  <th>Duracion</th>
                  <th>Preview</th>
                  <th className="col-actions">Acciones</th>
                </tr>
              </thead>
              <tbody>
                {props.assets.map((a) => (
                  <LibraryAssetRow
                    key={a.id}
                    asset={a}
                    projectName={props.projects.find((p) => p.id === a.project_id)?.name ?? "—"}
                    busy={props.busy}
                    onDelete={() => props.onDelete(a.id)}
                    onSave={(category, filename) => props.onSave(a.id, category, filename)}
                  />
                ))}
              </tbody>
            </table>
          </div>
          {props.assets.length === 0 ? <p className="muted library-empty">No hay assets todavía.</p> : null}
        </div>
      </div>
    </section>
  );
}

function LibraryAssetRow(props: {
  asset: AudioAsset;
  projectName: string;
  busy: boolean;
  onDelete: () => void;
  onSave: (category: Category, filename: string) => void;
}) {
  const [filename, setFilename] = useState(props.asset.canonical_filename);
  const [category, setCategory] = useState<Category>(props.asset.category);

  useEffect(() => {
    setFilename(props.asset.canonical_filename);
    setCategory(props.asset.category);
  }, [props.asset.id, props.asset.canonical_filename, props.asset.category]);

  const dirty =
    filename.trim() !== props.asset.canonical_filename || category !== props.asset.category;

  return (
    <tr>
      <td className="path col-file">
        <input
          type="text"
          className="library-filename-input"
          value={filename}
          onChange={(e) => setFilename(e.target.value)}
          aria-label={`Nombre de ${props.asset.canonical_filename}`}
        />
      </td>
      <td>
        <select value={category} onChange={(e) => setCategory(e.target.value as Category)}>
          {CATEGORIES.map((c) => (
            <option key={c.id} value={c.id}>
              {c.label}
            </option>
          ))}
        </select>
      </td>
      <td>{formatBpm(props.asset.source_session_bpm)}</td>
      <td className="col-project">{props.projectName}</td>
      <td>{props.asset.harvested_at.slice(0, 10)}</td>
      <td>{formatDuration(props.asset.duration_seconds)}</td>
      <td className="col-preview">
        <LibraryAudioPlayer src={props.asset.canonical_path} label={props.asset.canonical_filename} />
      </td>
      <td className="col-actions">
        <div className="library-row-actions">
          <button
            type="button"
            className="btn btn-compact"
            disabled={props.busy || !dirty || filename.trim() === ""}
            onClick={() => props.onSave(category, filename.trim())}
          >
            Guardar
          </button>
          <button
            type="button"
            className="btn btn-compact danger"
            disabled={props.busy}
            onClick={() => {
              const ok = window.confirm(
                `¿Eliminar "${props.asset.canonical_filename}" de la biblioteca?\n\nSe borrarán las copias en Local/Drive/Dropbox.\nEl Consolidate original en Ableton NO se toca.`,
              );
              if (ok) props.onDelete();
            }}
          >
            Eliminar
          </button>
        </div>
      </td>
    </tr>
  );
}
