interface LoadingBarProps {
  label?: string;
  compact?: boolean;
}

export function LoadingBar({ label, compact }: LoadingBarProps) {
  return (
    <div className={`loading-bar-block${compact ? " compact" : ""}`} role="status" aria-live="polite">
      {label ? <span className="loading-bar-label">{label}</span> : null}
      <div className="loading-bar-track" aria-hidden="true">
        <div className="loading-bar-fill" />
      </div>
    </div>
  );
}
