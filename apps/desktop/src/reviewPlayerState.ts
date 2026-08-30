/** Pure helpers for the pre-import review player (unit-testable). */

export function formatPlayerTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "00:00";
  const s = Math.floor(seconds);
  const m = Math.floor(s / 60);
  const r = s % 60;
  return `${String(m).padStart(2, "0")}:${String(r).padStart(2, "0")}`;
}

export function seekFromRatio(ratio: number, duration: number): number {
  if (!Number.isFinite(duration) || duration <= 0) return 0;
  const clamped = Math.max(0, Math.min(1, ratio));
  return clamped * duration;
}

export function ratioFromTime(currentTime: number, duration: number): number {
  if (!Number.isFinite(duration) || duration <= 0) return 0;
  return Math.max(0, Math.min(1, currentTime / duration));
}

export function stopResetsTime(): number {
  return 0;
}

export interface PlayerSource {
  key: string;
  label: string;
  src: string;
}

export interface PlayerTransition {
  previousKey: string | null;
  nextKey: string;
  shouldAutoplay: boolean;
}

/** When switching sources, reset position and optionally autoplay. */
export function transitionSource(
  previousKey: string | null,
  nextKey: string,
  wasPlaying: boolean,
): PlayerTransition {
  return {
    previousKey,
    nextKey,
    shouldAutoplay: wasPlaying,
  };
}

/** Preview must not mutate review/import selection state. */
export function previewDoesNotChangeSelection<T extends { selected: boolean }>(
  before: T,
  after: T,
): boolean {
  return before.selected === after.selected;
}
