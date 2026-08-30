import type { RefObject } from "react";
import { formatPlayerTime, ratioFromTime } from "./reviewPlayerState";

/** Wide inline scrub bar shown directly under the active review row. */
export function InlineReviewTrack(props: {
  currentTime: number;
  duration: number;
  seeking: boolean;
  error: string | null;
  onSeekStart: () => void;
  onSeek: (ratio: number) => void;
  onSeekEnd: () => void;
}) {
  const ratio = ratioFromTime(props.currentTime, props.duration);
  const pct = `${ratio * 100}%`;

  return (
    <div className="review-inline-player">
      <span className="review-inline-time">{formatPlayerTime(props.currentTime)}</span>
      <div
        className="review-inline-track"
        role="slider"
        aria-label="Posición de reproducción"
        aria-valuemin={0}
        aria-valuemax={props.duration || 0}
        aria-valuenow={props.currentTime}
        tabIndex={0}
        onMouseDown={(e) => {
          props.onSeekStart();
          const rect = e.currentTarget.getBoundingClientRect();
          props.onSeek((e.clientX - rect.left) / rect.width);
        }}
        onMouseMove={(e) => {
          if (!props.seeking) return;
          const rect = e.currentTarget.getBoundingClientRect();
          props.onSeek((e.clientX - rect.left) / rect.width);
        }}
        onMouseUp={props.onSeekEnd}
      >
        <div className="review-inline-fill" style={{ width: pct }} />
        <div className="review-inline-thumb" style={{ left: pct }} />
      </div>
      <span className="review-inline-time">{formatPlayerTime(props.duration)}</span>
      {props.error ? <p className="danger review-inline-error">{props.error}</p> : null}
    </div>
  );
}

export function HiddenReviewAudio(props: { audioRef: RefObject<HTMLAudioElement> }) {
  return <audio ref={props.audioRef} preload="metadata" className="review-hidden-audio" />;
}

export { formatPlayerTime };
