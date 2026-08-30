use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Minimal PCM WAV writer used by tests (and only tests).
pub fn write_pcm_wav(path: &Path, sample_rate: u32, channels: u16, samples: &[i16]) {
    let data_len = (samples.len() * 2) as u32;
    let mut f = File::create(path).expect("create wav");
    f.write_all(b"RIFF").unwrap();
    f.write_all(&(36 + data_len).to_le_bytes()).unwrap();
    f.write_all(b"WAVEfmt ").unwrap();
    f.write_all(&16u32.to_le_bytes()).unwrap();
    f.write_all(&1u16.to_le_bytes()).unwrap();
    f.write_all(&channels.to_le_bytes()).unwrap();
    f.write_all(&sample_rate.to_le_bytes()).unwrap();
    let byte_rate = sample_rate * u32::from(channels) * 2;
    f.write_all(&byte_rate.to_le_bytes()).unwrap();
    f.write_all(&(channels * 2).to_le_bytes()).unwrap();
    f.write_all(&16u16.to_le_bytes()).unwrap();
    f.write_all(b"data").unwrap();
    f.write_all(&data_len.to_le_bytes()).unwrap();
    for s in samples {
        f.write_all(&s.to_le_bytes()).unwrap();
    }
}
