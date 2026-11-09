//! The notes one instrument plays in a recording (a stem), heard with the ears: Basic Pitch's (`earshot::notes`), its
//! model loaded the first time it is asked for (downloaded first when the ears may download, as Spleeter's nets are,
//! `stems::may_download`). Without the ears, or while the model is neither in the cache nor may be downloaded, there
//! are none, and what wants them makes do without (`crate::rebuild`).

use crate::audio::Stereo;

/// a note heard: from `start` to `end` seconds into the recording, its MIDI `pitch`, and how sure the ears are of it,
/// 0 to 1
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Heard {
    pub start: f64,
    pub end: f64,
    pub pitch: i32,
    pub sure: f32,
}

/// The notes in `x` between `lo` and `hi` Hz, `shortest` ms long at least, by the start: none without the ears, or
/// while their model is not there.
pub fn notes(x: &Stereo, lo: f64, hi: f64, shortest: f64) -> Option<Vec<Heard>> {
    hearing::notes(x, lo, hi, shortest)
}

/// whether `notes` hears any: the ears, and their model in the cache or allowed to be downloaded
pub fn can() -> bool {
    hearing::can()
}

#[cfg(feature = "ears")]
mod hearing {
    use std::sync::{Mutex, OnceLock};

    use super::Heard;
    use crate::audio::Stereo;
    use crate::config;

    type Slot = Mutex<Option<earshot::notes::Transcriber>>;

    /// Basic Pitch, loaded the first time it is asked for; none while its model is not in the cache and may not be
    /// downloaded, and none when it cannot load, which is said once
    fn transcriber() -> Option<&'static Slot> {
        static T: OnceLock<Slot> = OnceLock::new();
        if T.get().is_none() && !can() {
            return None;
        }
        Some(T.get_or_init(|| {
            let cache = earshot::models::default_cache_dir();
            let loaded = earshot::notes::Transcriber::load(&cache, false, earshot::models::default_threads());
            if let Err(e) = &loaded {
                eprintln!("claudius: the notes cannot be heard: {e:#}");
            }
            Mutex::new(loaded.ok())
        }))
    }

    pub fn can() -> bool {
        crate::stems::downloads() || earshot::notes::Transcriber::cached(&earshot::models::default_cache_dir())
    }

    pub fn notes(x: &Stereo, lo: f64, hi: f64, shortest: f64) -> Option<Vec<Heard>> {
        if x.is_empty() {
            return Some(Vec::new());
        }
        let mut guard = transcriber()?.lock().unwrap_or_else(|e| e.into_inner());
        let t = guard.as_mut()?;
        let o =
            earshot::notes::Options { min_hz: Some(lo), max_hz: Some(hi), min_note_ms: shortest, ..Default::default() };
        let heard = t.notes(&x.mono(), config::SR, &o).ok()?;
        Some(
            heard
                .into_iter()
                .map(|n| Heard { start: n.start, end: n.end, pitch: n.pitch as i32, sure: n.amplitude })
                .collect(),
        )
    }
}

#[cfg(not(feature = "ears"))]
mod hearing {
    use super::Heard;
    use crate::audio::Stereo;

    pub fn notes(_: &Stereo, _: f64, _: f64, _: f64) -> Option<Vec<Heard>> {
        None
    }

    pub fn can() -> bool {
        false
    }
}
