//! Questions about the song as it is, answered from what the song holds: "what's the bass volume", "how much reverb is
//! on the snare", "what key is it in", "which track is the loudest". A track's level, drive, reverb, delay, pan and
//! sound, whether it is muted or soloed and where it plays; what plays in a section; the song's tempo, key, genre,
//! title, length, sections and tracks, which tracks are muted or soloed, and which is loudest or quietest. An answer
//! changes nothing. The palette shows it first, so that Enter on a question makes no change, and a question is not
//! asked of the request model, which would read it as one.

use claudius_api::{SectionView, SongView, State, TrackView};
use serde::{Deserialize, Serialize};

use crate::ask::{clock, contains_word, db, list, named_tracks, pan, trim};

/// An answer to a question about the song, in words: a line, and what more there is to say under it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Told {
    pub said: String,
    #[serde(default)]
    pub detail: String,
}

/// the words a question about the song opens with
const OPENERS: &[&str] = &[
    "what", "what's", "whats", "which", "how", "is", "are", "does", "do", "where", "when", "tell me", "show me", "list",
];

/// the openings of questions that ask for something the song does not hold (how to do a thing, what a word means,
/// what the person should do), for a change put as a question, or for talk
const NOT_ASKED: &[&str] = &[
    "how do",
    "how can",
    "how to",
    "how about",
    "how would",
    "how should",
    "how are",
    "what if",
    "what can",
    "what should",
    "what does",
    "what do",
    "can you",
    "could you",
    "would you",
    "will you",
    "do you",
    "are you",
    "is it possible",
    "is this song",
    "is it good",
    "does it sound",
];

/// words a question may open with before it asks, which say nothing
const FILLERS: &[&str] = &["hey", "hi", "so", "ok", "okay", "and", "um", "claudius", "please", "quick question"];

/// `query` lowercase, its curly apostrophes straight, without the words it opens with that say nothing
fn plain(query: &str) -> String {
    let mut q = query.trim().to_lowercase().replace('\u{2019}', "'");
    while let Some(rest) = FILLERS.iter().find_map(|f| {
        q.strip_prefix(f).filter(|r| r.starts_with([' ', ',', '!'])).map(|r| r.trim_start_matches([' ', ',', '!']))
    }) {
        q = rest.to_string();
    }
    q
}

/// whether `q` opens with the words `w`
fn opens(q: &str, w: &str) -> bool {
    q.strip_prefix(w).is_some_and(|rest| rest.is_empty() || rest.starts_with([' ', '?', ',']))
}

/// Whether `query` asks a question about the song rather than for a change to it: it opens as a question does ("what",
/// "how loud", "is", "which" ...) and asks neither how to do something nor for a change put as a question ("can you
/// ..."); or it is a few words and a question mark ("bass volume?").
pub fn asking(query: &str) -> bool {
    let q = plain(query);
    if NOT_ASKED.iter().any(|w| opens(&q, w)) {
        return false;
    }
    OPENERS.iter().any(|w| opens(&q, w)) || (q.ends_with('?') && q.split_whitespace().count() <= 3)
}

/// The answers to `query` on the song in `state`, when it asks a question about it (`asking`), from the song as it is:
/// of the song as a whole when it asks of that (its key, its tempo ...), else of the tracks it names, a line for each
/// reading of them, else of a section it names. None for a request, or a question about nothing the song holds.
pub fn tell(query: &str, state: &State) -> Vec<Told> {
    let Some(song) = state.song.as_ref().filter(|_| asking(query)) else { return Vec::new() };
    let q = plain(query);
    let q = q.trim_end_matches(|c: char| c == '?' || c == '.' || c.is_whitespace());
    if let Some(t) = of_the_song(q, song) {
        return vec![t];
    }
    let groups = named_tracks(q, song);
    if !groups.is_empty() {
        return groups.iter().take(4).map(|g| of_tracks(q, g, song)).collect();
    }
    in_a_section(q, song).into_iter().collect()
}

/// whether `q` holds any of `words`, each a word or words of its own
fn has(q: &str, words: &[&str]) -> bool {
    words.iter().any(|w| contains_word(q, w))
}

/// a share, 0 to 1, as a percent
fn percent(share: f32) -> String {
    format!("{}%", (share * 100.0).round())
}

/// the genre as people write it: "deep house", "trip hop and idm"
fn genre(g: &str) -> String {
    g.split('+').map(|p| p.trim().replace('_', " ")).collect::<Vec<_>>().join(" and ")
}

/// An answer about the song as a whole, when `q` asks one: its tempo, key, genre, title, length, sections, its tracks
/// and how many, which are muted or soloed, or which is the loudest or quietest.
fn of_the_song(q: &str, song: &SongView) -> Option<Told> {
    let told = |said: String, detail: String| Some(Told { said, detail });
    // "the keys" are a track; "what key" is the song's
    if has(q, &["key", "scale"]) && !has(q, &["keys"]) {
        return told(format!("The song is in {}", song.key), String::new());
    }
    if has(q, &["tempo", "bpm", "how fast", "speed"]) {
        let swing = if song.swing > 0.51 { format!("swung {}%", (song.swing * 100.0).round()) } else { String::new() };
        return told(format!("The song is at {} BPM", trim(song.bpm)), swing);
    }
    if has(q, &["genre", "style", "kind of music", "type of music", "kind of song"]) {
        return told(format!("The song is {}", genre(&song.genre)), String::new());
    }
    if has(q, &["title", "called", "name of the song", "song's name", "song called"]) {
        return told(format!("The song is called {}", song.title), String::new());
    }
    if has(q, &["how long", "length", "duration", "minutes"]) {
        return told(format!("The song is {} long", clock(song.seconds)), format!("{} bars", song.bars));
    }
    let tracks = &song.tracks;
    if has(q, &["loudest", "quietest", "softest"]) {
        // by how loud it played in the last render, else by its level
        let loud = |t: &TrackView| t.loudness.unwrap_or(f64::from(t.level_db) - 20.0);
        let playing = tracks.iter().filter(|t| !t.muted);
        let pick = match has(q, &["loudest"]) {
            true => playing.max_by(|a, b| loud(a).total_cmp(&loud(b))),
            false => playing.min_by(|a, b| loud(a).total_cmp(&loud(b))),
        }?;
        let which = if has(q, &["loudest"]) { "loudest" } else { "quietest" };
        let how = match pick.loudness {
            Some(l) => format!("{} LUFS in the last render, at {} dB", trim(l), db(pick.level_db)),
            None => format!("at {} dB", db(pick.level_db)),
        };
        return told(format!("The {which} track is {}", pick.name), how);
    }
    if has(q, &["muted", "silenced"]) && named_tracks(q, song).is_empty() {
        let muted: Vec<&str> = tracks.iter().filter(|t| t.muted).map(|t| t.name.as_str()).collect();
        return match muted.is_empty() {
            true => told("No track is muted".into(), String::new()),
            false => told(format!("Muted: {}", list(muted.into_iter())), String::new()),
        };
    }
    if has(q, &["soloed", "solo"]) && named_tracks(q, song).is_empty() {
        let solo: Vec<&str> = tracks.iter().filter(|t| t.solo).map(|t| t.name.as_str()).collect();
        return match solo.is_empty() {
            true => told("No track is soloed".into(), String::new()),
            false => told(format!("Soloed: {}", list(solo.into_iter())), String::new()),
        };
    }
    if has(q, &["sections", "structure", "form", "arrangement"]) {
        let each: Vec<String> = song.sections.iter().map(|s| format!("{} ({} bars)", s.name, s.bars)).collect();
        return told(format!("{} sections", song.sections.len()), each.join(", "));
    }
    let asks_tracks = has(q, &["tracks", "instruments", "how many tracks", "what's playing", "what is playing"])
        || (has(q, &["in the song", "in the mix"]) && !has(q, LEVEL));
    let a_section = song.sections.iter().any(|s| contains_word(q, &s.name.to_lowercase()));
    if asks_tracks && !a_section && named_tracks(q, song).is_empty() {
        // what plays: the tracks not muted, or only those soloed when any is
        let soloed = tracks.iter().any(|t| t.solo);
        let plays: Vec<&str> =
            tracks.iter().filter(|t| !t.muted && (!soloed || t.solo)).map(|t| t.name.as_str()).collect();
        return match has(q, &["playing", "play", "plays", "hear"]) {
            true => told(format!("{} tracks play", plays.len()), list(plays.into_iter())),
            false => {
                let quiet: Vec<&str> = tracks.iter().filter(|t| t.muted).map(|t| t.name.as_str()).collect();
                let muted =
                    if quiet.is_empty() { String::new() } else { format!(" (muted: {})", list(quiet.into_iter())) };
                told(
                    format!("{} tracks", tracks.len()),
                    format!("{}{muted}", list(tracks.iter().map(|t| t.name.as_str()))),
                )
            }
        };
    }
    None
}

/// An answer about `tracks`, a reading of the tracks `q` names: what it asks of them (their level, drive, reverb,
/// delay or pan, their sound, whether they are muted or soloed, where they play), or when it asks nothing in
/// particular, each one's sound and mix.
fn of_tracks(q: &str, tracks: &[&TrackView], song: &SongView) -> Told {
    let each = |f: &dyn Fn(&TrackView) -> String| tracks.iter().map(|t| f(t)).collect::<Vec<_>>().join("; ");
    let muted = |t: &TrackView| if t.muted { " (muted)" } else { "" };
    let said = if has(q, &["muted", "mute", "silenced", "silent", "turned off", "switched off"]) {
        each(&|t| format!("{} is {}", t.name, if t.muted { "muted" } else { "not muted" }))
    } else if has(q, &["soloed", "solo", "on its own"]) {
        each(&|t| format!("{} is {}", t.name, if t.solo { "soloed" } else { "not soloed" }))
    } else if has(q, &["drive", "overdrive", "overload", "distortion", "distorted", "saturation", "saturated", "grit"])
    {
        each(&|t| format!("{} has {} drive{}", t.name, percent(t.drive_db / 36.0), muted(t)))
    } else if has(q, &["reverb", "verb", "wet", "dry", "room", "hall", "space"]) {
        each(&|t| format!("{} has {} reverb{}", t.name, percent(t.reverb), muted(t)))
    } else if has(q, &["delay", "echo", "echoes"]) {
        each(&|t| format!("{} has {} delay{}", t.name, percent(t.delay), muted(t)))
    } else if has(q, PANNED) {
        each(&|t| format!("{} is panned to {}{}", t.name, pan(t.pan), muted(t)))
    } else if has(q, &["where", "when", "which sections", "what sections", "come in", "comes in"]) {
        each(&|t| format!("{} plays in {}", t.name, sections_of(t, &song.sections)))
    } else if has(q, LEVEL) {
        each(&|t| format!("{} is at {} dB{}", t.name, db(t.level_db), muted(t)))
    } else if has(q, &["sound", "sounds", "instrument", "preset", "patch", "play", "playing", "plays"]) {
        each(&|t| format!("{} plays {}{}", t.name, t.sound, muted(t)))
    } else {
        each(&|t| {
            format!(
                "{}: {}, at {} dB, panned to {}, {} reverb{}",
                t.name,
                t.sound,
                db(t.level_db),
                pan(t.pan),
                percent(t.reverb),
                muted(t)
            )
        })
    };
    // how loud they played in the last render, beside their levels
    let heard: Vec<String> =
        tracks.iter().filter_map(|t| t.loudness.map(|l| format!("{} {} LUFS", t.name, trim(l)))).collect();
    let detail = match has(q, LEVEL) && !heard.is_empty() {
        true => format!("in the last render: {}", heard.join(", ")),
        false => String::new(),
    };
    Told { said: capital(&said), detail }
}

/// the words a question asks a track's pan in ("right now" is no pan)
const PANNED: &[&str] = &[
    "pan",
    "panned",
    "panning",
    "stereo",
    "to the left",
    "to the right",
    "on the left",
    "on the right",
    "left or right",
    "in the center",
    "in the centre",
    "centered",
    "centred",
];

/// the words a question asks a track's level in
const LEVEL: &[&str] = &["volume", "level", "loud", "quiet", "db", "gain", "fader", "louder", "quieter"];

/// the sections a track plays in, by name: "intro, verse and drop", or "no section"
fn sections_of(t: &TrackView, sections: &[SectionView]) -> String {
    let names: Vec<&str> = sections.iter().filter(|s| s.layers.contains(&t.name)).map(|s| s.name.as_str()).collect();
    if names.is_empty() { "no section".into() } else { list(names.into_iter()) }
}

/// An answer about a section `q` names, when it asks what plays there ("what plays in the drop"): the tracks that play
/// in it, and how long it is and where it starts.
fn in_a_section(q: &str, song: &SongView) -> Option<Told> {
    if !has(q, &["play", "plays", "playing", "in the", "what's in", "what is in"]) {
        return None;
    }
    let s = song.sections.iter().find(|s| contains_word(q, &s.name.to_lowercase()))?;
    let at = clock(f64::from(s.start_bar) * 240.0 / song.bpm.max(1.0));
    let said = match s.layers.is_empty() {
        true => format!("Nothing plays in {}", s.name),
        false => format!("{}: {}", capital(&s.name), list(s.layers.iter().map(String::as_str))),
    };
    Some(Told { said, detail: format!("{} bars from {at}", s.bars) })
}

/// `s` with its first letter a capital
fn capital(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_question_opens_as_one_and_a_request_put_as_one_is_none() {
        for q in [
            "what's the bass volume",
            "What’s the tempo?",
            "how loud is the kick right now",
            "is the organ muted",
            "which track is the loudest",
            "hey claudius, what key is it in",
            "bass volume?",
            "tell me about the bass",
        ] {
            assert!(asking(q), "{q}");
        }
        for q in [
            "make the bass louder",
            "can you make the bass louder?",
            "how do i record my voice",
            "what does reverb actually do",
            "turn the reverb on the snare up to 40%",
            "is this song any good",
            "who are you",
        ] {
            assert!(!asking(q), "{q}");
        }
    }
}
