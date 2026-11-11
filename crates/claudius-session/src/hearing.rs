//! Whistle, Cactus's speech model (`whistle`), held once for the process: what is said to the palette, as words. It
//! downloads the first time words are said to it (17 MB, its pinned revision, checked against its size and hash) into
//! claudius's cache, and hears English, the language the readings read.

use std::sync::Mutex;

use claudius_api::{Heard, State};
use claudius_intent::model::Source;
use whistle::{Options, Whistle};

/// Whistle's weights (Apache-2.0), at the revision claudius was checked with
pub const WHISTLE: Source = Source {
    name: "whistle",
    repo: "Cactus-Compute/whistle",
    revision: "ad73f1468dd5203791e0cc81d31544c553563645",
    files: &[("whistle.cact", 16_919_407, "b6e02f048568ac5d01a2042556c658061e699acbc0aa2a1439f52f3d461dffeb")],
};

/// the rate Whistle hears at
const RATE: u32 = 16_000;

/// the most it hears at once, in seconds
pub const MOST: f64 = whistle::MAX_SECONDS as f64;

/// the model once it is there, which hears one recording at a time
static HELD: Mutex<Option<Whistle>> = Mutex::new(None);

/// What a recording says (`wav`, a WAV file, as the palette sends what it heard), in English: as the model heard it,
/// and in the words the readings read, those of the song of `state` put right (`claudius_intent::spoken`). The model
/// downloads the first time.
pub fn hear(wav: Vec<u8>, state: &State) -> Result<Heard, String> {
    let audio = claudius_decode::decode_bytes(wav, "wav").map_err(|e| format!("what was said is no WAV file: {e}"))?;
    let frames = audio.frames();
    if frames == 0 {
        return Ok(Heard::default());
    }
    let secs = frames as f64 / f64::from(audio.rate);
    if secs > MOST {
        return Err(format!("{secs:.0} s said at once: the palette hears {MOST:.0} s at most"));
    }
    let mono: Vec<f32> =
        audio.samples.chunks_exact(audio.channels).map(|f| f.iter().sum::<f32>() / f.len() as f32).collect();
    // nothing as loud as speech at its loudest (-40 dBFS): silence, a quiet room, or an input the system keeps from
    // claudius, which the model, raising its level, would hear words in
    if mono.iter().all(|s| s.abs() < 0.01) {
        return Ok(Heard::default());
    }
    let pcm = match audio.rate {
        RATE => mono,
        rate => {
            let n = (frames as u64 * u64::from(RATE) / u64::from(rate)) as usize;
            claudius::audio::resample_rate(&mono, rate, RATE, n)
        }
    };
    let mut held = HELD.lock().unwrap_or_else(|e| e.into_inner());
    if held.is_none() {
        let file = crate::meaning::cached(&WHISTLE)?;
        *held = Some(Whistle::from_bytes(file).map_err(|e| format!("whistle.cact: {e}"))?);
    }
    let Some(whistle) = held.as_ref() else { return Err("the speech model did not load".into()) };
    let options = Options { language: Some("en".into()), ..Options::default() };
    let heard = whistle.transcribe(&pcm, &options)?.text;
    let text = claudius_intent::spoken::heard(&heard, state);
    Ok(Heard { heard, text })
}
