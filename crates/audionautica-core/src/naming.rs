use std::collections::HashSet;
use std::path::Path;

use unicode_normalization::UnicodeNormalization;

use crate::error::{AppError, AppResult};

/// Ableton Live appends a timestamp suffix immediately before the extension:
/// `nombre musical [YYYY-MM-DD HHMMSS].ext`
///
/// Example: `textura agua [2026-08-29 184322].wav` → `textura agua.wav`
pub fn strip_ableton_consolidate_timestamp(filename: &str) -> String {
    let path = Path::new(filename);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_string());
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);

    if let Some(open) = stem.rfind(" [") {
        let suffix = &stem[open..];
        if is_ableton_timestamp_suffix(suffix) {
            let clean_stem = &stem[..open];
            return match ext {
                Some(e) if !e.is_empty() => format!("{clean_stem}.{e}"),
                _ => clean_stem.to_string(),
            };
        }
    }

    filename.to_string()
}

/// Library filename preserves the musical name with only Ableton's automatic timestamp removed.
pub fn library_filename_from_original(original_filename: &str) -> String {
    strip_ableton_consolidate_timestamp(original_filename)
}

const FILENAME_FORBIDDEN: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

/// Normalize a user-edited library filename. Spaces and unicode are preserved; path separators are rejected.
pub fn normalize_library_filename_input(input: &str, fallback_ext: &str) -> AppResult<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidPath(
            "El nombre del archivo no puede estar vacío".into(),
        ));
    }
    let nfc: String = trimmed.nfc().collect();
    let mut cleaned = String::with_capacity(nfc.len());
    for ch in nfc.chars() {
        if ch.is_control() || FILENAME_FORBIDDEN.contains(&ch) {
            cleaned.push('_');
        } else {
            cleaned.push(ch);
        }
    }
    let cleaned = cleaned.trim_matches(|c: char| c == '.' || c == ' ');
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        return Err(AppError::InvalidPath("Nombre de archivo inválido".into()));
    }

    let path = Path::new(&cleaned);
    let actual_ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    let has_known_ext = actual_ext
        .as_deref()
        .is_some_and(|e| matches!(e, "wav" | "aif" | "aiff" | "flac"));
    let mut name = if has_known_ext {
        cleaned.to_string()
    } else {
        let fe = fallback_ext.trim_start_matches('.');
        if fe.is_empty() {
            format!("{cleaned}.wav")
        } else {
            format!("{cleaned}.{fe}")
        }
    };

    if let Some(stem) = Path::new(&name).file_stem().and_then(|s| s.to_str()) {
        if stem.trim().is_empty() {
            return Err(AppError::InvalidPath(
                "El nombre del archivo no puede estar vacío".into(),
            ));
        }
    }

    if !is_supported_audio_filename(&name) {
        return Err(AppError::InvalidPath(format!(
            "Extensión no soportada en «{name}». Usa wav, aif, aiff o flac."
        )));
    }

    // Windows: no trailing dots/spaces in the final component.
    while name.ends_with('.') || name.ends_with(' ') {
        name.pop();
    }
    if name.is_empty() {
        return Err(AppError::InvalidPath("Nombre de archivo inválido".into()));
    }

    Ok(name)
}

/// When two different assets clean to the same filename, allocate `name (2).ext`, `name (3).ext`, …
pub fn resolve_filename_collision(base: &str, taken: &HashSet<String>) -> String {
    if !taken.contains(base) {
        return base.to_string();
    }
    let (stem, ext) = split_stem_ext(base);
    let mut n = 2u32;
    loop {
        let candidate = if ext.is_empty() {
            format!("{stem} ({n})")
        } else {
            format!("{stem} ({n}).{ext}")
        };
        if !taken.contains(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

pub fn extension_of(filename: &str) -> String {
    Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .filter(|e| !e.is_empty())
        .unwrap_or_else(|| "wav".to_string())
}

pub fn is_supported_audio_filename(filename: &str) -> bool {
    matches!(
        extension_of(filename).as_str(),
        "wav" | "aif" | "aiff" | "flac"
    )
}

fn split_stem_ext(filename: &str) -> (String, String) {
    match filename.rfind('.') {
        Some(i) if i > 0 => (filename[..i].to_string(), filename[i + 1..].to_string()),
        _ => (filename.to_string(), String::new()),
    }
}

/// ` [YYYY-MM-DD HHMMSS]` — only when immediately before extension (validated on suffix segment).
fn is_ableton_timestamp_suffix(suffix: &str) -> bool {
    // suffix begins with " [" and ends with ']'
    if suffix.len() != 20 || !suffix.starts_with(" [") || !suffix.ends_with(']') {
        return false;
    }
    let inner = &suffix[2..suffix.len() - 1];
    if inner.len() != 17 {
        return false;
    }
    let b = inner.as_bytes();
    b[0..4].iter().all(|c| c.is_ascii_digit())
        && b[4] == b'-'
        && b[5..7].iter().all(|c| c.is_ascii_digit())
        && b[7] == b'-'
        && b[8..10].iter().all(|c| c.is_ascii_digit())
        && b[10] == b' '
        && b[11..17].iter().all(|c| c.is_ascii_digit())
}

// Legacy BPM token helper — metadata only, not used in library filenames.
pub fn bpm_token(bpm: Option<f64>) -> String {
    match bpm {
        None => "BPMUNK".to_string(),
        Some(value) if !value.is_finite() || value <= 0.0 => "BPMUNK".to_string(),
        Some(value) => {
            let rounded = value.round();
            if (value - rounded).abs() < 0.05 {
                format!("{}BPM", rounded as i64)
            } else {
                let one = (value * 10.0).round() / 10.0;
                let s = format!("{one:.1}").replace('.', "p");
                format!("{s}BPM")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_simple_timestamp() {
        assert_eq!(
            strip_ableton_consolidate_timestamp("loop [2026-08-29 184322].wav"),
            "loop.wav"
        );
    }

    #[test]
    fn strip_spaced_musical_name() {
        assert_eq!(
            strip_ableton_consolidate_timestamp("textura agua [2026-08-29 184322].wav"),
            "textura agua.wav"
        );
    }

    #[test]
    fn strip_preserves_embedded_year_in_name() {
        assert_eq!(
            strip_ableton_consolidate_timestamp("jam 2025 textura [2026-08-29 184322].wav"),
            "jam 2025 textura.wav"
        );
    }

    #[test]
    fn does_not_strip_arbitrary_date_in_name() {
        let unchanged = "grabacion 2026-08-29 final.wav";
        assert_eq!(
            strip_ableton_consolidate_timestamp(unchanged),
            unchanged
        );
    }

    #[test]
    fn unchanged_without_ableton_timestamp() {
        let unchanged = "ritmo base.wav";
        assert_eq!(strip_ableton_consolidate_timestamp(unchanged), unchanged);
    }

    #[test]
    fn unicode_and_accents_preserved() {
        assert_eq!(
            strip_ableton_consolidate_timestamp("Percusión Acuática Ñ [2026-08-29 184322].wav"),
            "Percusión Acuática Ñ.wav"
        );
    }

    #[test]
    fn normalize_user_filename_preserves_spaces() {
        assert_eq!(
            normalize_library_filename_input("textura agua", "wav").unwrap(),
            "textura agua.wav"
        );
    }

    #[test]
    fn collision_suffix_human() {
        let mut taken = HashSet::new();
        taken.insert("textura.wav".to_string());
        assert_eq!(resolve_filename_collision("textura.wav", &taken), "textura (2).wav");
        taken.insert("textura (2).wav".to_string());
        assert_eq!(resolve_filename_collision("textura.wav", &taken), "textura (3).wav");
    }

    #[test]
    fn preserves_original_extension() {
        assert_eq!(
            library_filename_from_original("clip [2026-08-29 184322].aiff"),
            "clip.aiff"
        );
        assert_eq!(
            extension_of("a.FLAC"),
            "flac"
        );
    }

    #[test]
    fn non_positive_bpm_is_unknown() {
        assert_eq!(bpm_token(Some(0.0)), "BPMUNK");
        assert_eq!(bpm_token(Some(-10.0)), "BPMUNK");
        assert_eq!(bpm_token(Some(f64::NAN)), "BPMUNK");
    }
}
