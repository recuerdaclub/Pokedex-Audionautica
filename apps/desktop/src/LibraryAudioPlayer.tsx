import { convertFileSrc } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import { claimLibraryPlayback, releaseLibraryPlayback } from "./libraryAudioPlayback";
import { formatPlayerTime, ratioFromTime, seekFromRatio } from "./reviewPlayerState";

export function LibraryAudioPlayer(props: { src: string; label: string }) {
  const audioRef = useRef<HTMLAudioElement>(null);
  const [playing, setPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const [seeking, setSeeking] = useState(false);

  const stopSelf = useCallback(() => {
    const audio = audioRef.current;
    if (audio) audio.pause();
    setPlaying(false);
  }, []);

  useEffect(() => {
    const audio = audioRef.current;
    if (!audio) return;

    const onTime = () => {
      if (!seeking) setCurrentTime(audio.currentTime);
    };
    const onMeta = () => setDuration(audio.duration || 0);
    const onPlay = () => setPlaying(true);
    const onPause = () => setPlaying(false);
    const onEnded = () => {
      setPlaying(false);
      setCurrentTime(0);
      releaseLibraryPlayback(stopSelf);
    };

    audio.addEventListener("timeupdate", onTime);
    audio.addEventListener("loadedmetadata", onMeta);
    audio.addEventListener("durationchange", onMeta);
    audio.addEventListener("play", onPlay);
    audio.addEventListener("pause", onPause);
    audio.addEventListener("ended", onEnded);
    return () => {
      audio.removeEventListener("timeupdate", onTime);
      audio.removeEventListener("loadedmetadata", onMeta);
      audio.removeEventListener("durationchange", onMeta);
      audio.removeEventListener("play", onPlay);
      audio.removeEventListener("pause", onPause);
      audio.removeEventListener("ended", onEnded);
    };
  }, [seeking, props.src, stopSelf]);

  useEffect(() => {
    setPlaying(false);
    setCurrentTime(0);
    setDuration(0);
    setSeeking(false);
  }, [props.src]);

  useEffect(() => {
    return () => {
      stopSelf();
      releaseLibraryPlayback(stopSelf);
    };
  }, [stopSelf]);

  const onSeek = useCallback(
    (ratio: number) => {
      const audio = audioRef.current;
      if (!audio || !duration) return;
      const next = seekFromRatio(ratio, duration);
      audio.currentTime = next;
      setCurrentTime(next);
    },
    [duration],
  );

  async function togglePlay() {
    const audio = audioRef.current;
    if (!audio) return;
    if (playing) {
      audio.pause();
      releaseLibraryPlayback(stopSelf);
      return;
    }
    claimLibraryPlayback(stopSelf);
    if (!audio.src) {
      audio.src = convertFileSrc(props.src);
    }
    try {
      await audio.play();
    } catch {
      setPlaying(false);
      releaseLibraryPlayback(stopSelf);
    }
  }

  const ratio = ratioFromTime(currentTime, duration);
  const pct = `${ratio * 100}%`;

  return (
    <div className="library-audio-player">
      <button
        type="button"
        className={`btn library-audio-play${playing ? " playing" : ""}`}
        title={playing ? "Pausar" : "Reproducir"}
        aria-label={playing ? `Pausar ${props.label}` : `Reproducir ${props.label}`}
        onClick={() => void togglePlay()}
      >
        {playing ? "❚❚" : "▶"}
      </button>
      <div className="library-audio-player-track">
        <span className="library-audio-time">{formatPlayerTime(currentTime)}</span>
        <div
          className="review-inline-track library-audio-seek"
          role="slider"
          aria-label={`Posición de ${props.label}`}
          aria-valuemin={0}
          aria-valuemax={duration || 0}
          aria-valuenow={currentTime}
          tabIndex={0}
          onMouseDown={(e) => {
            setSeeking(true);
            const rect = e.currentTarget.getBoundingClientRect();
            onSeek((e.clientX - rect.left) / rect.width);
          }}
          onMouseMove={(e) => {
            if (!seeking) return;
            const rect = e.currentTarget.getBoundingClientRect();
            onSeek((e.clientX - rect.left) / rect.width);
          }}
          onMouseUp={() => setSeeking(false)}
          onMouseLeave={() => {
            if (seeking) setSeeking(false);
          }}
        >
          <div className="review-inline-fill" style={{ width: pct }} />
          <div className="review-inline-thumb" style={{ left: pct }} />
        </div>
        <span className="library-audio-time">{formatPlayerTime(duration)}</span>
      </div>
      <audio ref={audioRef} preload="none" className="review-hidden-audio" />
    </div>
  );
}
