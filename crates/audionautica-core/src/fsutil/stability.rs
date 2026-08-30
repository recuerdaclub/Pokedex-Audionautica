use std::fs::{self, File};
use std::path::Path;
use std::thread;
use std::time::{Duration, SystemTime};

use thiserror::Error;

#[derive(Debug, Clone)]
pub struct StabilityConfig {
    pub checks: u32,
    pub interval: Duration,
    pub max_wait: Duration,
}

impl Default for StabilityConfig {
    fn default() -> Self {
        Self {
            checks: 2,
            interval: Duration::from_millis(250),
            max_wait: Duration::from_secs(8),
        }
    }
}

impl StabilityConfig {
    pub fn fast_test() -> Self {
        Self {
            checks: 2,
            interval: Duration::from_millis(15),
            max_wait: Duration::from_millis(400),
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StabilityError {
    #[error("el archivo todavía se está escribiendo")]
    StillWriting,
    #[error("el archivo no existe")]
    Missing,
    #[error("el archivo está vacío")]
    Empty,
    #[error("el archivo no se puede leer")]
    Unreadable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Meta {
    size: u64,
    modified: Option<SystemTime>,
}

fn read_meta(path: &Path) -> Result<Meta, StabilityError> {
    let md = fs::metadata(path).map_err(|_| {
        if path.exists() {
            StabilityError::Unreadable
        } else {
            StabilityError::Missing
        }
    })?;
    if !md.is_file() {
        return Err(StabilityError::Unreadable);
    }
    Ok(Meta {
        size: md.len(),
        modified: md.modified().ok(),
    })
}

fn try_open_readable(path: &Path) -> Result<(), StabilityError> {
    File::open(path)
        .map(|_| ())
        .map_err(|_| StabilityError::Unreadable)
}

/// Wait until the file exists, size+mtime are stable across checks, and it is readable.
///
/// No giant arbitrary delays: we poll with a short interval until `max_wait`.
pub fn wait_until_stable(path: &Path, cfg: &StabilityConfig) -> Result<(), StabilityError> {
    let deadline = std::time::Instant::now() + cfg.max_wait;
    let mut last: Option<Meta> = None;
    let mut stable_hits = 0u32;

    loop {
        match read_meta(path) {
            Ok(meta) => {
                if meta.size == 0 {
                    last = Some(meta);
                    stable_hits = 0;
                } else if let Some(prev) = &last {
                    if prev.size == meta.size && prev.modified == meta.modified {
                        stable_hits += 1;
                        if stable_hits >= cfg.checks.saturating_sub(1).max(1) {
                            try_open_readable(path)?;
                            return Ok(());
                        }
                    } else {
                        stable_hits = 0;
                    }
                    last = Some(meta);
                } else {
                    last = Some(meta);
                    stable_hits = 0;
                }
            }
            Err(StabilityError::Missing) => {
                last = None;
                stable_hits = 0;
            }
            Err(other) => return Err(other),
        }

        if std::time::Instant::now() >= deadline {
            if last.as_ref().map(|m| m.size).unwrap_or(0) == 0 && path.exists() {
                return Err(StabilityError::Empty);
            }
            return Err(StabilityError::StillWriting);
        }
        thread::sleep(cfg.interval);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("aud-stab-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn stable_file_passes() {
        let dir = tmp();
        let path = dir.join("ok.wav");
        fs::write(&path, vec![0u8; 256]).unwrap();
        wait_until_stable(&path, &StabilityConfig::fast_test()).unwrap();
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn growing_file_is_not_copied() {
        let dir = tmp();
        let path = dir.join("partial.wav");
        let path2 = path.clone();
        let handle = std::thread::spawn(move || {
            let mut f = File::create(&path2).unwrap();
            for _ in 0..80 {
                f.write_all(&[1u8; 256]).unwrap();
                f.flush().unwrap();
                thread::sleep(Duration::from_millis(8));
            }
        });

        thread::sleep(Duration::from_millis(20));
        let result = wait_until_stable(
            &path,
            &StabilityConfig {
                checks: 3,
                interval: Duration::from_millis(25),
                max_wait: Duration::from_millis(180),
            },
        );
        handle.join().ok();
        assert_eq!(result, Err(StabilityError::StillWriting));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_file_times_out() {
        let dir = tmp();
        let path = dir.join("nope.wav");
        let result = wait_until_stable(&path, &StabilityConfig::fast_test());
        assert_eq!(result, Err(StabilityError::StillWriting));
        fs::remove_dir_all(&dir).ok();
    }
}
