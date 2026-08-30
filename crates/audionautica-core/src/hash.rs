use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::error::{AppError, AppResult};

/// BLAKE3 hex digest of file contents. This is content identity.
pub fn hash_file(path: &Path) -> AppResult<String> {
    let mut file = File::open(path).map_err(|e| AppError::from_io(e, "abrir para hash"))?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| AppError::from_io(e, "leer para hash"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn identical_bytes_same_hash() {
        assert_eq!(hash_bytes(b"loop"), hash_bytes(b"loop"));
    }

    #[test]
    fn different_bytes_different_hash() {
        assert_ne!(hash_bytes(b"loop"), hash_bytes(b"loop "));
    }

    #[test]
    fn file_hash_matches_bytes() {
        let dir = std::env::temp_dir().join(format!("aud-hash-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.wav");
        let mut f = File::create(&path).unwrap();
        f.write_all(b"RIFF....WAVE").unwrap();
        drop(f);
        assert_eq!(hash_file(&path).unwrap(), hash_bytes(b"RIFF....WAVE"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
