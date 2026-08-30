use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::fsutil::paths::is_audio_path;

/// Read-only Ableton Live project inspector.
///
/// Guarantees:
/// - NEVER modifies the `.als`
/// - NEVER resaves / rewrites XML
/// - Treats the internal format as unstable
pub struct AbletonProjectReader;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbletonSetInfo {
    pub als_path: PathBuf,
    pub project_root: PathBuf,
    pub project_name: String,
    pub consolidate_dir: PathBuf,
    pub consolidate_exists: bool,
    /// Base tempo from the set, if parsed with confidence.
    pub tempo: Option<f64>,
    pub tempo_source: TempoSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TempoSource {
    AlsManual,
    Unknown,
}

impl AbletonProjectReader {
    pub fn inspect(als_path: &Path) -> AppResult<AbletonSetInfo> {
        if !als_path.is_file() {
            return Err(AppError::AbletonSetNotFound(als_path.display().to_string()));
        }
        let project_root = als_path
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| AppError::AbletonSetNotFound(als_path.display().to_string()))?;
        let project_name = als_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("PROJECT")
            .trim()
            .to_string();
        let consolidate_dir = consolidate_dir(&project_root);
        let consolidate_exists = consolidate_dir.is_dir();
        let (tempo, tempo_source) = match read_tempo_readonly(als_path) {
            Ok(Some(bpm)) => (Some(bpm), TempoSource::AlsManual),
            Ok(None) => (None, TempoSource::Unknown),
            Err(_) => (None, TempoSource::Unknown),
        };
        Ok(AbletonSetInfo {
            als_path: als_path.to_path_buf(),
            project_root,
            project_name,
            consolidate_dir,
            consolidate_exists,
            tempo,
            tempo_source,
        })
    }
}

pub fn consolidate_dir(project_root: &Path) -> PathBuf {
    project_root
        .join("Samples")
        .join("Processed")
        .join("Consolidate")
}

/// Open the `.als` for reading only. Live sets are gzip-compressed XML (sometimes plain XML).
fn read_tempo_readonly(als_path: &Path) -> AppResult<Option<f64>> {
    let mut file = File::open(als_path).map_err(|_| AppError::AbletonSetUnreadable)?;
    let mut header = [0u8; 2];
    let n = file
        .read(&mut header)
        .map_err(|_| AppError::AbletonSetUnreadable)?;
    drop(file);

    let xml = if n == 2 && header == [0x1f, 0x8b] {
        let file = File::open(als_path).map_err(|_| AppError::AbletonSetUnreadable)?;
        let mut decoder = GzDecoder::new(file);
        let mut s = String::new();
        decoder
            .read_to_string(&mut s)
            .map_err(|_| AppError::AbletonSetUnreadable)?;
        s
    } else {
        let mut file = File::open(als_path).map_err(|_| AppError::AbletonSetUnreadable)?;
        let mut s = String::new();
        file.read_to_string(&mut s)
            .map_err(|_| AppError::AbletonSetUnreadable)?;
        s
    };

    Ok(extract_tempo(&xml))
}

/// Live 12 stores the set tempo on `<MainTrack><Tempo><Manual Value>`.
/// Older sets used `<MasterTrack>`. Other `<Tempo>` nodes appear earlier in
/// the XML (often with 0 or 2) and are **not** the transport BPM.
pub fn extract_tempo(xml: &str) -> Option<f64> {
    for anchor in ["<MainTrack", "<MasterTrack"] {
        if let Some(idx) = xml.find(anchor) {
            if let Some(rel) = xml[idx..].find("<Tempo") {
                if let Some(bpm) = tempo_at(xml, idx + rel) {
                    return Some(bpm);
                }
            }
        }
    }
    let mut search = 0usize;
    while let Some(rel) = xml[search..].find("<Tempo") {
        let abs = search + rel;
        if let Some(bpm) = tempo_at(xml, abs) {
            return Some(bpm);
        }
        search = abs + 6;
    }
    None
}

fn tempo_at(xml: &str, lower_idx: usize) -> Option<f64> {
    let end_bound = xml.len().min(lower_idx.saturating_add(8192));
    let window = xml.get(lower_idx..end_bound)?;
    let close = window
        .find("</Tempo>")
        .or_else(|| window.find("</tempo>"))
        .unwrap_or(window.len().saturating_sub(1));
    let block = window.get(..=close.min(window.len().saturating_sub(1)))?;
    parse_manual_value(block)
}

fn parse_manual_value(block: &str) -> Option<f64> {
    const NEEDLE: &str = "Manual Value=\"";
    const NEEDLE2: &str = "Manual Value='";
    let (idx, q) = if let Some(i) = block.find(NEEDLE) {
        (i + NEEDLE.len(), '"')
    } else {
        let i = block.find(NEEDLE2)?;
        (i + NEEDLE2.len(), '\'')
    };
    let rest = block.get(idx..)?;
    let end = rest.find(q)?;
    let raw = rest.get(..end)?.trim();
    let value: f64 = raw.parse().ok()?;
    if value.is_finite() && value > 0.0 && value < 999.0 {
        Some(value)
    } else {
        None
    }
}

pub fn list_audio_files(dir: &Path) -> AppResult<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    if !dir.is_dir() {
        return Err(AppError::ConsolidateUnavailable(dir.display().to_string()));
    }
    let mut out = Vec::new();
    collect_audio(dir, &mut out)?;
    out.sort();
    Ok(out)
}

fn collect_audio(dir: &Path, out: &mut Vec<PathBuf>) -> AppResult<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            return Err(AppError::from_io(e, "leer carpeta Consolidate"));
        }
    };
    for entry in entries {
        let entry = entry.map_err(|e| AppError::from_io(e, "entrada Consolidate"))?;
        let path = entry.path();
        if path.is_dir() {
            collect_audio(&path, out)?;
        } else if is_audio_path(&path) {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    fn sample_xml(bpm: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Ableton MajorVersion="5" Creator="Ableton Live 12">
  <LiveSet>
    <MasterTrack>
      <Tempo>
        <LomId Value="0" />
        <Manual Value="{bpm}" />
      </Tempo>
    </MasterTrack>
  </LiveSet>
</Ableton>
"#
        )
    }

    #[test]
    fn extracts_manual_tempo() {
        assert_eq!(extract_tempo(&sample_xml("126")), Some(126.0));
        assert_eq!(extract_tempo(&sample_xml("98.5")), Some(98.5));
    }

    #[test]
    fn refuses_nonsense_tempo() {
        assert_eq!(extract_tempo(&sample_xml("0")), None);
        assert_eq!(extract_tempo(&sample_xml("-4")), None);
        assert_eq!(extract_tempo(&sample_xml("abc")), None);
    }

    #[test]
    fn inspect_gzip_als_read_only() {
        let dir = std::env::temp_dir().join(format!("aud-als-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let als = dir.join("HYDRA.als");
        let xml = sample_xml("133");
        let mut enc = GzEncoder::new(File::create(&als).unwrap(), Compression::default());
        enc.write_all(xml.as_bytes()).unwrap();
        enc.finish().unwrap();
        let before = std::fs::read(&als).unwrap();
        let info = AbletonProjectReader::inspect(&als).unwrap();
        let after = std::fs::read(&als).unwrap();
        assert_eq!(before, after, "parser must never rewrite the .als");
        assert_eq!(info.tempo, Some(133.0));
        assert_eq!(info.project_name, "HYDRA");
        assert_eq!(
            info.consolidate_dir,
            dir.join("Samples").join("Processed").join("Consolidate")
        );
        assert!(!info.consolidate_exists);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn live12_prefers_maintrack_over_earlier_decoy_tempo() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Ableton MajorVersion="5" Creator="Ableton Live 12.2.5">
  <LiveSet>
    <Tracks>
      <AudioTrack>
        <DeviceChain>
          <Mixer>
            <Tempo>
              <Manual Value="0" />
            </Tempo>
          </Mixer>
        </DeviceChain>
      </AudioTrack>
    </Tracks>
    <MainTrack>
      <DeviceChain>
        <Mixer>
          <Tempo>
            <LomId Value="0" />
            <Manual Value="126" />
          </Tempo>
        </Mixer>
      </DeviceChain>
    </MainTrack>
  </LiveSet>
</Ableton>
"#;
        assert_eq!(extract_tempo(xml), Some(126.0));
    }

    #[test]
    fn live12_decoy_tempo_two_is_not_used() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Ableton Creator="Ableton Live 12.2.5">
  <LiveSet>
    <Foo>
      <Tempo><Manual Value="2" /></Tempo>
    </Foo>
    <MainTrack>
      <Tempo><Manual Value="20" /></Tempo>
    </MainTrack>
  </LiveSet>
</Ableton>
"#;
        assert_eq!(extract_tempo(xml), Some(20.0));
    }

    #[test]
    fn missing_consolidate_is_ok() {
        let dir = std::env::temp_dir().join(format!("aud-als2-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let als = dir.join("X.als");
        std::fs::write(&als, sample_xml("120")).unwrap();
        let info = AbletonProjectReader::inspect(&als).unwrap();
        assert!(!info.consolidate_exists);
        std::fs::remove_dir_all(&dir).ok();
    }
}
