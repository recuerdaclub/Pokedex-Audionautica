import {
  formatFileSize,
  platformInstallInstructions,
  type UpdateInfo,
} from "./updates";

interface UpdateDialogProps {
  info: UpdateInfo;
  busy?: boolean;
  onInstall: () => void;
  onOpenGitHub: () => void;
  onDismiss: () => void;
}

export function UpdateDialog(props: UpdateDialogProps) {
  const instructions = platformInstallInstructions(props.info.platform);
  const asset = props.info.asset;

  return (
    <div className="update-overlay" role="presentation" onClick={props.onDismiss}>
      <div
        className="update-dialog"
        role="dialog"
        aria-labelledby="update-dialog-title"
        aria-modal="true"
        onClick={(event) => event.stopPropagation()}
      >
        <header className="update-dialog-header">
          <h2 id="update-dialog-title">Actualización disponible</h2>
          <p className="update-dialog-subtitle">
            {props.info.releaseName} · v{props.info.latestVersion}
          </p>
        </header>

        <div className="update-dialog-body">
          <p className="update-version-line">
            Instalada: <strong>v{props.info.currentVersion}</strong> → Nueva:{" "}
            <strong>v{props.info.latestVersion}</strong>
          </p>

          {props.info.releaseNotes ? (
            <section className="update-notes">
              <h3>Notas de la versión</h3>
              <pre>{props.info.releaseNotes}</pre>
            </section>
          ) : null}

          <section className="update-instructions">
            <h3>Instrucciones</h3>
            <ol>
              {instructions.map((line) => (
                <li key={line}>{line}</li>
              ))}
            </ol>
          </section>

          {asset ? (
            <p className="update-asset-line">
              Archivo: <code>{asset.name}</code> ({formatFileSize(asset.size)})
            </p>
          ) : (
            <p className="update-asset-line warn">
              No encontramos un instalador automático para tu plataforma. Usa Ver en GitHub.
            </p>
          )}
        </div>

        <footer className="update-dialog-actions">
          <button type="button" className="secondary" onClick={props.onDismiss} disabled={props.busy}>
            Más tarde
          </button>
          <button type="button" className="secondary" onClick={props.onOpenGitHub} disabled={props.busy}>
            Ver en GitHub
          </button>
          <button
            type="button"
            className="primary"
            onClick={props.onInstall}
            disabled={props.busy || !asset}
          >
            {props.busy ? "Abriendo…" : "Descargar e instalar"}
          </button>
        </footer>
      </div>
    </div>
  );
}
