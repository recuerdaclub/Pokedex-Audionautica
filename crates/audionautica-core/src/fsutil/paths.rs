use std::path::{Path, PathBuf};

use unicode_normalization::UnicodeNormalization;

pub const AUDIO_EXTENSIONS: &[&str] = &["wav", "aif", "aiff", "flac"];

/// Characters forbidden on Windows *and* `:` which is illegal on macOS.
const FORBIDDEN: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

pub fn sanitize_path_component(input: &str) -> String {
    let nfc: String = input.nfc().collect();
    let mut out = String::with_capacity(nfc.len());
    for ch in nfc.chars() {
        if ch.is_control() || FORBIDDEN.contains(&ch) {
            out.push('_');
        } else {
            out.push(ch);
        }
    }
    let out = out.trim_matches(|c: char| c == '.' || c == ' ' || c == '_');
    let out = out.replace(' ', "_");
    if out.is_empty() {
        "untitled".to_string()
    } else {
        out.to_string()
    }
}

/// Uppercase ASCII-ish token for canonical filenames.
pub fn sanitize_filename_token(input: &str, max_len: usize) -> String {
    let nfc: String = input.nfc().collect();
    let mut out = String::new();
    for ch in nfc.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_uppercase());
        } else if ch == '_' || ch == '-' {
            if !out.ends_with('_') {
                out.push('_');
            }
        } else if !ch.is_ascii() && ch.is_alphanumeric() {
            // Keep unicode letters (e.g. Ñ) but strip combining marks via NFC already.
            for u in ch.to_uppercase() {
                if FORBIDDEN.contains(&u) || u.is_control() {
                    continue;
                }
                out.push(u);
            }
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.len() > max_len {
        out.truncate(max_len);
        while out.ends_with('_') {
            out.pop();
        }
    }
    out
}

pub fn join_library_relative(year: i32, category_folder: &str, filename: &str) -> PathBuf {
    Path::new("Loops")
        .join(year.to_string())
        .join(category_folder)
        .join(filename)
}

pub fn is_audio_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_windows_forbidden() {
        let s = sanitize_path_component(r#"loop<>:"/\|?*name"#);
        assert!(!s.contains('<') && !s.contains(':') && !s.contains('*'));
    }

    #[test]
    fn keeps_unicode_and_spaces_as_underscore() {
        let s = sanitize_path_component("textura café 01");
        assert!(s.contains("café") || s.contains("cafe") || s.contains("caf"));
        assert!(!s.contains(' '));
    }

    #[test]
    fn filename_token_uppercases_ascii() {
        assert_eq!(sanitize_filename_token("hydra set", 24), "HYDRA_SET");
    }

    #[test]
    fn library_relative_uses_year_and_category() {
        let p = join_library_relative(2026, "Texturas", "AUD_X.wav");
        let expected = Path::new("Loops")
            .join("2026")
            .join("Texturas")
            .join("AUD_X.wav");
        assert_eq!(p, expected);
    }
}
