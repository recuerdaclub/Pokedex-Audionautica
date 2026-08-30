use std::path::Path;

use tracing::{info, warn};
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

pub fn init_file_logging(log_dir: &Path) {
    let _ = std::fs::create_dir_all(log_dir);
    let file = log_dir.join("audionautica.log");
    let writer = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file);
    match writer {
        Ok(file) => {
            let _ = tracing_subscriber::registry()
                .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
                .with(
                    fmt::layer()
                        .with_ansi(false)
                        .with_target(false)
                        .with_writer(file),
                )
                .try_init();
        }
        Err(err) => {
            let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();
            warn!(error = %err, "no se pudo abrir el archivo de log; usando stderr");
        }
    }
    info!("logging initialized");
}

pub fn init_test_logging() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_test_writer()
        .try_init();
}
