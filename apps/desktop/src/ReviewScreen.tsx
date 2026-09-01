import { convertFileSrc } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState, type Dispatch, type SetStateAction } from "react";
import type { CandidateSelection, Category } from "./types";
import { CATEGORIES } from "./types";
import { HiddenReviewAudio, InlineReviewTrack } from "./ReviewPlayer";
import { seekFromRatio } from "./reviewPlayerState";

export interface ReviewRow {
  key: string;
  originalPath: string;
  originalFilename: string;
  libraryFilename: string;
  sizeBytes: number;
  subtitle: string;
  contentHash?: string;
}

export function ReviewScreen(props: {
  title: string;
  lede: string;
  rows: ReviewRow[];
  selections: Record<string, CandidateSelection>;
  setSelections: Dispatch<SetStateAction<Record<string, CandidateSelection>>>;
  bulkCategory: Category;
  setBulkCategory: (c: Category) => void;
  busy: boolean;
  primaryLabel: string;
  onPrimary: () => void;
  onBack?: () => void;
}) {
  const audioRef = useRef<HTMLAudioElement>(null);
  const [activeKey, setActiveKey] = useState<string | null>(null);
  const [playing, setPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const [seeking, setSeeking] = useState(false);
  const [error, setError] = useState<string | null>(null);

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

  const loadAndMaybePlay = useCallback(async (row: ReviewRow, autoplay: boolean) => {
    setError(null);
    setActiveKey(row.key);
    setCurrentTime(0);
    setDuration(0);
    setPlaying(false);
    const audio = audioRef.current;
    if (!audio) return;
    audio.pause();
    audio.removeAttribute("src");
    audio.load();
    audio.src = convertFileSrc(row.originalPath);
    audio.load();
    if (autoplay) {
      try {
        await audio.play();
        setPlaying(true);
        setError(null);
      } catch {
        setError("No se pudo reproducir este archivo.");
        setPlaying(false);
      }
    }
  }, []);

  function onPlayRow(row: ReviewRow) {
    const same = activeKey === row.key;
    if (same && playing) {
      audioRef.current?.pause();
      setPlaying(false);
      return;
    }
    if (same && !playing) {
      void audioRef.current
        ?.play()
        .then(() => {
          setPlaying(true);
          setError(null);
        })
        .catch(() => setError("No se pudo reproducir este archivo."));
      return;
    }
    void loadAndMaybePlay(row, true);
  }

  useEffect(() => {
    if (!seeking) return;
    const onUp = () => setSeeking(false);
    window.addEventListener("mouseup", onUp);
    return () => window.removeEventListener("mouseup", onUp);
  }, [seeking]);

  useEffect(() => {
    const audio = audioRef.current;
    if (!audio) return;
    const onTime = () => {
      if (!seeking) {
        setCurrentTime(audio.currentTime);
        if (audio.currentTime > 0 && !audio.error) setError(null);
      }
    };
    const onMeta = () => setDuration(Number.isFinite(audio.duration) ? audio.duration : 0);
    const onEnded = () => {
      setPlaying(false);
      setCurrentTime(0);
    };
    const onErr = () => {
      if (audio.error) {
        setError("No se pudo reproducir este archivo.");
        setPlaying(false);
        console.error("review preview error", audio.error, activeKey);
      }
    };
    audio.addEventListener("timeupdate", onTime);
    audio.addEventListener("loadedmetadata", onMeta);
    audio.addEventListener("durationchange", onMeta);
    audio.addEventListener("ended", onEnded);
    audio.addEventListener("error", onErr);
    return () => {
      audio.removeEventListener("timeupdate", onTime);
      audio.removeEventListener("loadedmetadata", onMeta);
      audio.removeEventListener("durationchange", onMeta);
      audio.removeEventListener("ended", onEnded);
      audio.removeEventListener("error", onErr);
    };
  }, [activeKey, seeking]);

  function onSeek(ratio: number) {
    const audio = audioRef.current;
    if (!audio) return;
    const t = seekFromRatio(ratio, duration);
    audio.currentTime = t;
    setCurrentTime(t);
  }

  return (
    <section className="review-screen">
      <HiddenReviewAudio audioRef={audioRef} />
      <h1>{props.title}</h1>
      <p className="lede">{props.lede}</p>
      <div className="row" style={{ marginBottom: 14 }}>
        <button type="button" className="btn" onClick={() => selectAll(true)}>
          Select all
        </button>
        <button type="button" className="btn" onClick={() => selectAll(false)}>
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
        <button type="button" className="btn" onClick={applyBulk}>
          Asignar categoria
        </button>
      </div>
      <div className="harvest-list review-list">
        {props.rows.map((row) => {
          const sel = props.selections[row.originalPath];
          const isActive = activeKey === row.key;
          return (
            <div className={`review-row-block${isActive ? " active" : ""}`} key={row.key}>
              <div className="review-item-row">
                <label className="review-ticket">
                  <input
                    type="checkbox"
                    className="review-ticket-input"
                    checked={sel?.selected ?? false}
                    onChange={(e) => patch(row.originalPath, { selected: e.target.checked })}
                    aria-label={`Farmear ${row.libraryFilename}`}
                  />
                  <span className="review-ticket-mark" aria-hidden="true" />
                </label>
                <button
                  type="button"
                  className={`btn primary review-play-btn${isActive && playing ? " playing" : ""}`}
                  title={isActive && playing ? "Pausar" : "Reproducir"}
                  onClick={() => onPlayRow(row)}
                >
                  {isActive && playing ? "❚❚" : "▶"}
                </button>
                <div className="review-item-main">
                  <input
                    type="text"
                    className="review-item-title-input"
                    value={sel?.library_filename_override ?? row.libraryFilename}
                    onChange={(e) => {
                      const v = e.target.value;
                      patch(row.originalPath, {
                        library_filename_override:
                          v.trim() === row.libraryFilename ? null : v,
                      });
                    }}
                    aria-label={`Nombre en biblioteca para ${row.libraryFilename}`}
                  />
                </div>
                <select
                  value={sel?.category ?? "OTHER"}
                  onChange={(e) => patch(row.originalPath, { category: e.target.value as Category })}
                >
                  {CATEGORIES.map((cat) => (
                    <option key={cat.id} value={cat.id}>
                      {cat.label}
                    </option>
                  ))}
                </select>
              </div>
            </div>
          );
        })}
        {props.rows.length === 0 ? <p className="muted">No hay sonidos pendientes.</p> : null}
      </div>
      <div className="review-footer">
        {activeKey ? (
          <div className="review-player-dock">
            <InlineReviewTrack
              currentTime={currentTime}
              duration={duration}
              seeking={seeking}
              error={error}
              onSeekStart={() => setSeeking(true)}
              onSeek={onSeek}
              onSeekEnd={() => setSeeking(false)}
            />
          </div>
        ) : null}
        <div className="actions">
          {props.onBack ? (
            <button type="button" className="btn" onClick={props.onBack}>
              Volver
            </button>
          ) : null}
          <button
            type="button"
            className="btn primary"
            disabled={props.busy || selectedCount === 0}
            onClick={props.onPrimary}
          >
            {props.primaryLabel}
          </button>
        </div>
      </div>
    </section>
  );
}
