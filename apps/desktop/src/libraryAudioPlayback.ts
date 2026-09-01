type StopFn = () => void;

let activeStop: StopFn | null = null;

/** Pause any other library preview before starting a new one. */
export function claimLibraryPlayback(stop: StopFn) {
  if (activeStop && activeStop !== stop) {
    activeStop();
  }
  activeStop = stop;
}

export function releaseLibraryPlayback(stop: StopFn) {
  if (activeStop === stop) {
    activeStop = null;
  }
}
