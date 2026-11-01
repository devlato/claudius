//! Takes: recordings for the song, sent by the studio as a file's bytes or named by path, kept with the song
//! (`claudius::take`). And recordings the studio sends to be listened to (`analyze_bytes`, `remake_bytes`), which
//! claudius keeps nothing of.

use std::path::Path;
use std::sync::Arc;

use claudius::audio::{self, Stereo};
use claudius::{config, model, take};
use claudius_api::Reply;

use super::{App, no};

/// the largest take the studio may send: about 13 minutes of 32-bit float stereo at 48 kHz
pub const MAX_TAKE: usize = 300 << 20;

impl App {
    /// a recording sent as a file's bytes (WAV, FLAC, Ogg, MP3 or AIFF), added as a take; it answers take request
    /// `request` when it was recorded for one
    pub fn add_take_bytes(
        self: &Arc<Self>,
        bytes: Vec<u8>,
        from: &str,
        name: Option<&str>,
        role: Option<&str>,
        request: Option<u64>,
    ) -> Reply {
        let Some(ext) = kind(&bytes) else {
            return no("send the take as a WAV, FLAC, Ogg, MP3 or AIFF file", self.version());
        };
        match audio::load_bytes(bytes, ext) {
            Ok(x) => self.answer(request, self.add_take(x, from, name, role)),
            Err(e) => no(format!("cannot read the take: {e:#}"), self.version()),
        }
    }

    /// what claudius hears in a recording sent as a file's bytes, named `name` (whose extension says its format, if it
    /// has one); nothing of it is kept
    pub fn analyze_bytes(self: &Arc<Self>, bytes: Vec<u8>, name: &str) -> Reply {
        match self.decode(bytes, name) {
            Ok(x) => self.analyze_audio(&x, name),
            Err(e) => no(e, self.version()),
        }
    }

    /// a new song on the skeleton of a recording sent as a file's bytes, as `remake` makes one from a path; nothing of
    /// the recording is kept
    pub fn remake_bytes(
        self: &Arc<Self>,
        bytes: Vec<u8>,
        name: &str,
        genre: Option<String>,
        seed: Option<u64>,
        bpm: Option<f64>,
    ) -> Reply {
        match self.decode(bytes, name) {
            Ok(x) => self.remake_audio(&x, name, genre, seed, bpm),
            Err(e) => no(e, self.version()),
        }
    }

    /// a recording's bytes decoded: the format its name's extension says, else the one its first bytes show
    fn decode(&self, bytes: Vec<u8>, name: &str) -> Result<Stereo, String> {
        let named = Path::new(name).extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
        let Some(ext) = named.or_else(|| kind(&bytes).map(String::from)) else {
            return Err("send the recording as the file itself: WAV, FLAC, MP3, AAC (m4a), Ogg or AIFF".into());
        };
        audio::load_bytes(bytes, &ext).map_err(|e| format!("cannot read {name}: {e:#}"))
    }

    /// the recording at `path` added as a take; the song keeps a copy of it
    pub(super) fn add_take_file(
        self: &Arc<Self>,
        path: &Path,
        from: &str,
        name: Option<&str>,
        role: Option<&str>,
    ) -> Reply {
        if let Err(e) = self.readable(path) {
            return no(e, self.version());
        }
        match audio::load_once(path) {
            Ok(x) => self.add_take(x, from, name, role),
            Err(e) => no(format!("cannot read {}: {e:#}", path.display()), self.version()),
        }
    }

    fn add_take(self: &Arc<Self>, x: Stereo, from: &str, name: Option<&str>, role: Option<&str>) -> Reply {
        let v = self.version();
        if self.blueprint().is_none() {
            return no("no song yet: start one with new_song, then add the take", v);
        }
        let secs = x.len() as f64 / config::SRF;
        if secs > model::MAX_SECONDS {
            return no(format!("the take is {:.0} s long, longer than a song may be", secs), v);
        }
        let dir = take::dir(self.lock().folder.as_deref());
        if !config::write_allowed(&dir, self.roots.as_deref()) {
            return no(format!("{} is outside the folders this claudius may write", dir.display()), v);
        }
        let file = match take::store(&x, &dir) {
            Ok(f) => f,
            Err(e) => return no(format!("cannot keep the take: {e:#}"), v),
        };
        self.edit_with(|bp| take::add(bp, &file, secs, from, name, role))
    }
}

/// which kind of audio file the bytes are, by how they start
fn kind(b: &[u8]) -> Option<&'static str> {
    let at = |i: usize, s: &[u8]| b.get(i..i + s.len()) == Some(s);
    if at(0, b"RIFF") && at(8, b"WAVE") {
        Some("wav")
    } else if at(0, b"FORM") && (at(8, b"AIFF") || at(8, b"AIFC")) {
        Some("aiff")
    } else if at(0, b"fLaC") {
        Some("flac")
    } else if at(0, b"OggS") {
        Some("ogg")
    } else if at(0, b"ID3") || (b.len() >= 2 && b[0] == 0xFF && b[1] & 0xE0 == 0xE0) {
        Some("mp3")
    } else {
        None
    }
}
