use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

use crate::error::{AppError, AppResult};
use crate::hash::hash_file;

/// Copy `source` → `dest` without modifying source.
///
/// Writes to a sibling `.tmp` file, verifies BLAKE3, then renames.
/// If `dest` already exists with the same hash, this is a successful no-op (idempotent).
pub fn copy_verified(source: &Path, dest: &Path) -> AppResult<()> {
    if !source.is_file() {
        return Err(AppError::InvalidPath(format!(
            "origen no es un archivo: {}",
            source.display()
        )));
    }
    let source_hash = hash_file(source)?;

    if dest.exists() {
        if dest.is_file() && hash_file(dest)? == source_hash {
            return Ok(());
        }
        return Err(AppError::CopyFailed {
            destination: dest.display().to_string(),
            reason: "ya existe un archivo distinto en el destino".into(),
        });
    }

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| AppError::from_io(e, "crear carpeta destino"))?;
        if !parent.is_dir() {
            return Err(AppError::DestinationUnavailable(
                parent.display().to_string(),
            ));
        }
    }

    let tmp = dest.with_file_name(format!(
        "{}.part",
        dest.file_name().and_then(|n| n.to_str()).unwrap_or("copy")
    ));

    if let Err(e) = stream_copy(source, &tmp) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }

    let tmp_hash = match hash_file(&tmp) {
        Ok(h) => h,
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            return Err(e);
        }
    };
    if tmp_hash != source_hash {
        let _ = fs::remove_file(&tmp);
        return Err(AppError::CopyFailed {
            destination: dest.display().to_string(),
            reason: "el hash de la copia no coincide con el origen".into(),
        });
    }

    if let Err(e) = fs::rename(&tmp, dest) {
        let _ = fs::remove_file(&tmp);
        return Err(AppError::from_io(e, "renombrar copia verificada"));
    }
    Ok(())
}

fn stream_copy(source: &Path, dest: &Path) -> AppResult<()> {
    let mut in_f = File::open(source).map_err(|e| AppError::from_io(e, "abrir origen"))?;
    let mut out_f = File::create(dest).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            AppError::PermissionDenied(dest.display().to_string())
        } else if e.kind() == std::io::ErrorKind::NotFound {
            AppError::DestinationUnavailable(
                dest.parent()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default(),
            )
        } else {
            AppError::from_io(e, "crear archivo temporal de copia")
        }
    })?;
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = in_f
            .read(&mut buf)
            .map_err(|e| AppError::from_io(e, "leer origen"))?;
        if n == 0 {
            break;
        }
        out_f
            .write_all(&buf[..n])
            .map_err(|e| AppError::from_io(e, "escribir copia"))?;
    }
    out_f
        .flush()
        .map_err(|e| AppError::from_io(e, "flush copia"))?;
    Ok(())
}

/// Re-hash the source and compare to a previously captured hash. Path must be unchanged.
pub fn source_bytes_unchanged(
    source: &Path,
    expected_path: &Path,
    expected_hash: &str,
) -> AppResult<bool> {
    if source != expected_path {
        return Ok(false);
    }
    Ok(hash_file(source)? == expected_hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_does_not_modify_source() {
        let dir = std::env::temp_dir().join(format!("aud-copy-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src.wav");
        let dst = dir.join("dst.wav");
        fs::write(&src, b"hello-audio").unwrap();
        let before = hash_file(&src).unwrap();
        copy_verified(&src, &dst).unwrap();
        assert_eq!(hash_file(&src).unwrap(), before);
        assert_eq!(hash_file(&dst).unwrap(), before);
        assert_eq!(src, dir.join("src.wav"));
        fs::remove_dir_all(&dir).ok();
    }
}
