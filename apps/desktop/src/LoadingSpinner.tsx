interface LoadingSpinnerProps {
  label?: string;
  size?: "sm" | "md";
}

export function LoadingSpinner({ label, size = "md" }: LoadingSpinnerProps) {
  return (
    <div className={`loading-spinner-block size-${size}`} role="status" aria-live="polite">
      <div className="loading-spinner" aria-hidden="true" />
      {label ? <span className="loading-spinner-label">{label}</span> : null}
    </div>
  );
}
