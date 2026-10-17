//! Playback of the latest render. Native: the 24-bit WAV master is fetched, decoded and played through rodio by a
//! custom source (seek, pause, loop regions and position are lock-free atomics). Web: an `HtmlAudioElement` plays
//! the mp3, `currentTime` is the playhead. Generated audio (the mock's preview) goes through the same players.

use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::Player;

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::Player;

/// decoded audio, interleaved samples
pub struct Pcm {
    pub rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

impl Pcm {
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1) as usize
    }

    pub fn seconds(&self) -> f64 {
        self.frames() as f64 / self.rate.max(1) as f64
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn decode_wav(bytes: &[u8]) -> Result<Pcm, String> {
        let mut reader =
            hound::WavReader::new(std::io::Cursor::new(bytes)).map_err(|e| format!("not a WAV file: {e}"))?;
        let spec = reader.spec();
        let samples: Result<Vec<f32>, hound::Error> = match spec.sample_format {
            hound::SampleFormat::Float => reader.samples::<f32>().collect(),
            hound::SampleFormat::Int => {
                let scale = 1.0 / (1u64 << (spec.bits_per_sample.clamp(1, 32) - 1)) as f32;
                reader.samples::<i32>().map(|s| s.map(|v| v as f32 * scale)).collect()
            }
        };
        let samples = samples.map_err(|e| format!("broken WAV data: {e}"))?;
        Ok(Pcm { rate: spec.sample_rate, channels: spec.channels.max(1), samples })
    }

    /// 16-bit PCM WAV (for handing generated audio to the browser)
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub fn encode_wav(&self) -> Vec<u8> {
        let data_len = (self.samples.len() * 2) as u32;
        let ch = self.channels.max(1);
        let mut out = Vec::with_capacity(44 + data_len as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&ch.to_le_bytes());
        out.extend_from_slice(&self.rate.to_le_bytes());
        out.extend_from_slice(&(self.rate * ch as u32 * 2).to_le_bytes());
        out.extend_from_slice(&(ch * 2).to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        for s in &self.samples {
            out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
        }
        out
    }
}

/// what to play
pub enum Source {
    /// generated audio
    Pcm(Arc<Pcm>),
    /// a render on the server (native: the .wav master, web: the .mp3)
    Url(String),
}
