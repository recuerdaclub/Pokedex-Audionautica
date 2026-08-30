use std::fs::File;
use std::path::Path;

use serde::{Deserialize, Serialize};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Best-effort technical metadata. Incomplete probe never blocks harvest.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AudioProbe {
    pub duration_seconds: Option<f64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
}

pub fn probe_file(path: &Path) -> AudioProbe {
    probe_inner(path).unwrap_or_default()
}

fn probe_inner(path: &Path) -> Result<AudioProbe, ()> {
    let file = File::open(path).map_err(|_| ())?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|_| ())?;
    let format = probed.format;
    let track = format.default_track().ok_or(())?;
    let codec = &track.codec_params;
    let sample_rate = codec.sample_rate;
    let channels = codec.channels.map(|c| c.count() as u16);
    let duration_seconds = match (codec.n_frames, sample_rate) {
        (Some(frames), Some(rate)) if rate > 0 => Some(frames as f64 / f64::from(rate)),
        _ => None,
    };
    Ok(AudioProbe {
        duration_seconds,
        sample_rate,
        channels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsutil::wav::write_pcm_wav;

    #[test]
    fn probes_wav_duration() {
        let dir = std::env::temp_dir().join(format!("aud-probe-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.wav");
        let samples = vec![0i16; 44100];
        write_pcm_wav(&path, 44100, 1, &samples);
        let probe = probe_file(&path);
        assert_eq!(probe.sample_rate, Some(44100));
        assert_eq!(probe.channels, Some(1));
        let dur = probe.duration_seconds.unwrap();
        assert!((dur - 1.0).abs() < 0.05);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn garbage_file_does_not_panic() {
        let dir = std::env::temp_dir().join(format!("aud-probe2-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("x.wav");
        std::fs::write(&path, b"not audio").unwrap();
        let probe = probe_file(&path);
        assert!(probe.duration_seconds.is_none());
        std::fs::remove_dir_all(&dir).ok();
    }
}
