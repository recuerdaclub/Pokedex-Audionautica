use std::path::PathBuf;

use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

/// User-facing errors. Messages are in Spanish and must remain understandable.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("No se encontró el Ableton Set o no está guardado: {0}")]
    AbletonSetNotFound(String),

    #[error("El Ableton Set no se pudo leer (solo lectura). El archivo no fue modificado.")]
    AbletonSetUnreadable,

    #[error("La carpeta Consolidate no está disponible: {0}")]
    ConsolidateUnavailable(String),

    #[error("El destino no está disponible: {0}")]
    DestinationUnavailable(String),

    #[error("Disco lleno al copiar hacia: {0}")]
    DiskFull(String),

    #[error("Permiso denegado: {0}")]
    PermissionDenied(String),

    #[error("El archivo todavía se está escribiendo: {0}")]
    FileStillWriting(String),

    #[error("Copia fallida hacia {destination}: {reason}")]
    CopyFailed { destination: String, reason: String },

    #[error("Fallo de base de datos: {0}")]
    Database(String),

    #[error("Ya hay una sesión activa. Termínala antes de iniciar otra.")]
    SessionAlreadyActive,

    #[error("No hay una sesión activa.")]
    NoActiveSession,

    #[error("Sesión no encontrada: {0}")]
    SessionNotFound(String),

    #[error("Configura primero la biblioteca local (Local Library).")]
    LocalLibraryMissing,

    #[error("Configura primero la carpeta de Google Drive para farmear.")]
    DriveLibraryMissing,

    #[error("Asset no encontrado en la biblioteca: {0}")]
    AssetNotFound(String),

    #[error("Ruta inválida: {0}")]
    InvalidPath(String),

    #[error("{0}")]
    Other(String),
}

impl AppError {
    pub fn from_io(err: std::io::Error, context: &str) -> Self {
        match err.kind() {
            std::io::ErrorKind::NotFound => AppError::InvalidPath(format!("{context}: no existe")),
            std::io::ErrorKind::PermissionDenied => {
                AppError::PermissionDenied(format!("{context}: {err}"))
            }
            std::io::ErrorKind::StorageFull => AppError::DiskFull(context.to_string()),
            _ => {
                let msg = err.to_string().to_lowercase();
                if msg.contains("no space") || msg.contains("disk full") {
                    AppError::DiskFull(context.to_string())
                } else {
                    AppError::Other(format!("{context}: {err}"))
                }
            }
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            AppError::AbletonSetNotFound(_) => "ableton_set_not_found",
            AppError::AbletonSetUnreadable => "ableton_set_unreadable",
            AppError::ConsolidateUnavailable(_) => "consolidate_unavailable",
            AppError::DestinationUnavailable(_) => "destination_unavailable",
            AppError::DiskFull(_) => "disk_full",
            AppError::PermissionDenied(_) => "permission_denied",
            AppError::FileStillWriting(_) => "file_still_writing",
            AppError::CopyFailed { .. } => "copy_failed",
            AppError::Database(_) => "database_failure",
            AppError::SessionAlreadyActive => "session_already_active",
            AppError::NoActiveSession => "no_active_session",
            AppError::SessionNotFound(_) => "session_not_found",
            AppError::LocalLibraryMissing => "local_library_missing",
            AppError::DriveLibraryMissing => "drive_library_missing",
            AppError::AssetNotFound(_) => "asset_not_found",
            AppError::InvalidPath(_) => "invalid_path",
            AppError::Other(_) => "other",
        }
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(value: rusqlite::Error) -> Self {
        AppError::Database(value.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        AppError::Other(format!("JSON: {value}"))
    }
}

/// Helper when a destination path is missing.
pub fn destination_missing(path: PathBuf) -> AppError {
    AppError::DestinationUnavailable(path.display().to_string())
}
