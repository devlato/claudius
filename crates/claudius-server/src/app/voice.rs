//! The voice a pitch check heard, kept between runs in the settings folder (`voice.json`), as any claudius wrote it
//! and any later one reads it (`claudius_api::settings`).

use claudius::voice::singer::Voice;
use claudius_api::settings::Fields;

/// the version of the file claudius writes: 1, the first with one; claudius before 0.6 wrote 0
pub const VERSION: u64 = 1;

/// The voice a file holds: each field read on its own, and anyone's where it does not read; none when the file holds
/// no voice.
pub fn read(text: &str) -> Option<Voice> {
    let f = Fields::parse(text);
    let d = Voice::default();
    let notes = f.get::<usize>("notes")?;
    Some(Voice {
        low: f.or("low", d.low),
        high: f.or("high", d.high),
        bias: f.or("bias", d.bias),
        spread: f.or("spread", d.spread),
        settle_ms: f.or("settle_ms", d.settle_ms),
        vibrato_hz: f.or("vibrato_hz", d.vibrato_hz),
        vibrato_st: f.or("vibrato_st", d.vibrato_st),
        notes,
        missed: f.or("missed", d.missed),
        long: f.or("long", d.long),
    })
}

/// the file of `voice`, keeping what the file as it was (`earlier`) holds that this claudius does not know
pub fn written(voice: &Voice, earlier: &str) -> String {
    Fields::parse(earlier).with(voice, VERSION).to_text()
}

#[cfg(test)]
mod tests {
    use claudius::voice::singer::Voice;

    use super::{VERSION, read, written};

    #[test]
    fn the_voice_file_any_claudius_wrote_is_read() {
        // as claudius 0.4 wrote it, before it heard long notes; and 0.5, with them; neither with a version
        let v04 = r#"{"low": 50.0, "high": 74.0, "bias": [-0.2, -0.1, 0.0], "spread": [0.3, 0.25, 0.3],
            "settle_ms": 80.0, "vibrato_hz": 5.5, "vibrato_st": 0.2, "notes": 24, "missed": 1}"#;
        let v04 = read(v04).unwrap();
        assert_eq!((v04.low, v04.bias[0], v04.notes, v04.long), (50.0, -0.2, 24, 0));
        let v05 = read(&serde_json::to_string(&Voice { long: 6, ..v04.clone() }).unwrap()).unwrap();
        assert_eq!(v05.long, 6);
        // a later one's, with a field changed and one this claudius does not know: kept when written again
        let later = r#"{"version": 4, "low": "C3", "high": 76.0, "notes": 30, "timbre": {"bright": 0.4}}"#;
        let v = read(later).unwrap();
        assert_eq!((v.low, v.high, v.notes), (Voice::default().low, 76.0, 30));
        let again = written(&v, later);
        assert!(again.contains("\"timbre\"") && again.contains(&format!("\"version\": {VERSION}")), "{again}");
        assert_eq!(read(&again), Some(v));
        assert_eq!(read("{}"), None, "no voice heard");
        assert_eq!(read("not a voice"), None);
    }
}
