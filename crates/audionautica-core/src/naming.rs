use chrono::{DateTime, Utc};

use crate::domain::Category;
use crate::fsutil::sanitize_filename_token;

/// Canonical library filename convention (deterministic):
///
/// ```text
/// AUD_{YYYYMMDD}_{BPMTOKEN}_{PROJECT}_{CATEGORY}_{NNN}.{ext}
/// ```
///
/// - `BPMTOKEN` is `{n}BPM` when `sourceSessionBpm` is known, otherwise `BPMUNK`.
///   BPM is never invented.
/// - `PROJECT` is a sanitized uppercase slug of the Ableton set / project name.
/// - `CATEGORY` is the filename token (TEXTURE, RHYTHM, …).
/// - `NNN` is a 3-digit counter (expands if needed).
///
/// Examples:
/// - `AUD_20260829_126BPM_HYDRA_TEXTURE_001.wav`
/// - `AUD_20260829_BPMUNK_HYDRA_OTHER_001.aiff`
pub fn canonical_filename(
    harvested_at: DateTime<Utc>,
    source_session_bpm: Option<f64>,
    project_name: &str,
    category: Category,
    sequence: u32,
    original_filename: &str,
) -> String {
    let date = harvested_at.format("%Y%m%d").to_string();
    let bpm = bpm_token(source_session_bpm);
    let project = sanitize_filename_token(project_name, 24);
    let project = if project.is_empty() {
        "PROJECT".to_string()
    } else {
        project
    };
    let cat = category.filename_token();
    let seq = format!("{sequence:03}");
    let ext = extension_of(original_filename);
    format!("AUD_{date}_{bpm}_{project}_{cat}_{seq}.{ext}")
}

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

pub fn extension_of(filename: &str) -> String {
    std::path::Path::new(filename)
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ts() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 29, 12, 0, 0).unwrap()
    }

    #[test]
    fn known_bpm_in_name() {
        let name = canonical_filename(
            ts(),
            Some(126.0),
            "HYDRA",
            Category::Textures,
            1,
            "0003.wav",
        );
        assert_eq!(name, "AUD_20260829_126BPM_HYDRA_TEXTURE_001.wav");
    }

    #[test]
    fn unknown_bpm_never_invented() {
        let name = canonical_filename(ts(), None, "HYDRA", Category::Other, 7, "clip.aiff");
        assert_eq!(name, "AUD_20260829_BPMUNK_HYDRA_OTHER_007.aiff");
        assert!(!name.contains("120BPM"));
        assert!(!name.contains("126BPM"));
    }

    #[test]
    fn non_positive_bpm_is_unknown() {
        assert_eq!(bpm_token(Some(0.0)), "BPMUNK");
        assert_eq!(bpm_token(Some(-10.0)), "BPMUNK");
        assert_eq!(bpm_token(Some(f64::NAN)), "BPMUNK");
    }

    #[test]
    fn sequence_and_category_and_project() {
        let name = canonical_filename(
            ts(),
            Some(90.0),
            "my project!!",
            Category::Rhythms,
            12,
            "loop.wav",
        );
        assert!(name.contains("_RHYTHM_"));
        assert!(name.contains("_012."));
        assert!(name.contains("90BPM"));
    }

    #[test]
    fn preserves_original_extension() {
        assert!(
            canonical_filename(ts(), None, "P", Category::Other, 1, "a.FLAC").ends_with(".flac")
        );
    }
}
