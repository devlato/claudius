//! Chords for a melody: a line of single notes harmonized as a player would under it. The song's own chords stay
//! where the melody's notes are their chord tones or pleasant extensions of them (a 9th, a 6th, a 7th); a held note
//! they clash with gets the chord of the key that holds it and shares most with theirs. The chords are then played
//! `block`, one under each note of the melody with the melody on top, in one of the chord parts' rhythms
//! (`melody::CHORD_STYLES`), or in one of the genres' comping idioms (`comp::IDIOMS`).

use super::chord::{Chord, Quality, Voicing, voice};
use super::melody;
use super::scale::Key;
use super::{ChordSpan, Note, chord_at};
use crate::rng::Rng;

/// the ways a harmony plays under its melody: a chord on each of its notes, or the chord parts' rhythms
pub const STYLES: &[&str] = &["block", "sustain", "half", "stabs", "offbeat", "pump", "syncopated", "charleston"];

/// every style a harmony plays in: `STYLES`, and the genres' comping idioms
pub fn styles() -> Vec<&'static str> {
    STYLES.iter().copied().chain(super::comp::IDIOMS.iter().map(|i| i.name)).collect()
}

/// how long a stretch of the melody one chord is chosen for, in beats: half a bar
const SLOT: f64 = 2.0;

/// how well melody pitch class `pc` sits on `chord`: 1 a chord tone, 0.6 a pleasant extension, below zero a clash
pub fn fits(chord: &Chord, pc: u8) -> f64 {
    let iv = (pc as i32 - chord.root as i32).rem_euclid(12);
    if chord.quality.intervals().iter().any(|i| i.rem_euclid(12) == iv) {
        return 1.0;
    }
    let minor = chord.quality.is_minorish();
    let dominant = matches!(chord.quality, Quality::Dom7 | Quality::Dom9);
    match iv {
        // the 9th, the 6th (13th) and the 7ths colour a chord
        2 => 0.6,
        9 if !matches!(chord.quality, Quality::Dim | Quality::HalfDim7) => 0.6,
        10 if !matches!(chord.quality, Quality::Maj7 | Quality::Maj9) => 0.5,
        11 if !minor && !dominant => 0.6,
        // the 11th sits on a minor chord, and against a major one's third
        5 if minor => 0.5,
        5 => -0.8,
        // the flat 9th rubs against the root, but leans a dominant onwards
        1 if dominant => 0.1,
        1 => -1.0,
        8 if !minor => -0.8,
        _ => -0.4,
    }
}

/// the triad on degree `d` of `key`, or its seventh chord
fn diatonic(key: &Key, d: i32, seventh: bool) -> Chord {
    let root = key.degree_pc(d);
    let up = |k: i32| (key.degree_pc(d + k) as i32 - root as i32).rem_euclid(12);
    let quality = match (up(2), up(4), seventh.then(|| up(6))) {
        (4, 7, None) => Quality::Maj,
        (3, 7, None) => Quality::Min,
        (3, 6, None) => Quality::Dim,
        (4, 8, None) => Quality::Aug,
        (4, 7, Some(11)) => Quality::Maj7,
        (4, 7, Some(10)) => Quality::Dom7,
        (3, 7, Some(10)) => Quality::Min7,
        (3, 6, Some(10)) => Quality::HalfDim7,
        (3, 6, Some(9)) => Quality::Dim7,
        (3, _, _) => Quality::Min,
        _ => Quality::Maj,
    };
    Chord::new(root, quality)
}

/// how many pitch classes two chords share
fn shared(a: &Chord, b: &Chord) -> usize {
    a.pcs().iter().filter(|pc| b.has_pc(**pc)).count()
}

/// The chord to play over `notes` (each with how long it sounds in the stretch, and whether it is on a beat) where
/// the song plays `song`: the song's own unless the notes fit another of the key's chords clearly better, a held
/// clash being what moves it.
fn choose(song: &Chord, notes: &[(u8, f64, bool)], key: &Key) -> Chord {
    let weight = |held: f64, strong: bool| held * if strong { 1.5 } else { 1.0 };
    let score = |c: &Chord| -> f64 { notes.iter().map(|&(pc, held, strong)| fits(c, pc) * weight(held, strong)).sum() };
    let seventh = song.quality.intervals().len() > 3;
    // the song's chord holds as long as the melody does not clash with it: a 9th over it stays a 9th
    let all: f64 = notes.iter().map(|&(_, held, strong)| weight(held, strong)).sum();
    let mut best = (score(song) + 0.5 * all + 0.5, *song);
    for d in 0..7 {
        let c = diatonic(key, d, seventh);
        // the closer to the song's chord, the likelier: a substitute rather than a new harmony
        let near =
            0.25 * shared(&c, song) as f64 - if matches!(c.quality, Quality::Dim | Quality::Dim7) { 0.6 } else { 0.0 };
        let s = score(&c) + near;
        if s > best.0 + 1e-9 {
            best = (s, c);
        }
    }
    best.1
}

/// The chords for `melody` over the song's `spans`, which come round every `cycle` beats, from beat 0 to `until`:
/// one for each half bar the melody sounds in (the song's own where it fits), joined where they are the same.
pub fn chords(melody: &[Note], spans: &[ChordSpan], cycle: f64, key: &Key, until: f64) -> Vec<ChordSpan> {
    let mut out: Vec<ChordSpan> = Vec::new();
    let song_at = |t: f64| -> Option<Chord> {
        let t = if cycle > 0.0 { t.rem_euclid(cycle) } else { t };
        chord_at(spans, t).map(|s| s.chord)
    };
    let mut t = 0.0;
    while t < until - 1e-9 {
        let end = (t + SLOT).min(until);
        let Some(song) = song_at(t) else { break };
        let heard: Vec<(u8, f64, bool)> = melody
            .iter()
            .filter_map(|n| {
                let held = (n.t + n.d).min(end) - n.t.max(t);
                (held > 1e-9).then(|| ((n.p.rem_euclid(12)) as u8, held, n.t >= t && (n.t - n.t.round()).abs() < 1e-6))
            })
            .collect();
        let chord = if heard.is_empty() { song } else { choose(&song, &heard, key) };
        match out.last_mut() {
            Some(last) if last.chord == chord && (last.start + last.len - t).abs() < 1e-9 => last.len += end - t,
            _ => out.push(ChordSpan { start: t, len: end - t, chord }),
        }
        t = end;
    }
    out
}

/// `chords` (which may change mid-bar) played in a chord rhythm that keeps to the bar: each hit on the chord the
/// harmony has when it falls, voiced anew only when the chord changes
fn in_rhythm(chords: &[ChordSpan], period: f64, play: &Play, rng: &mut Rng) -> Vec<Note> {
    let hits = melody::chord_hits(play.style);
    let mut out = Vec::new();
    let mut prev: Option<Vec<i32>> = None;
    let mut held: Option<usize> = None;
    for bar in 0..(period / 4.0).ceil().max(1.0) as usize {
        for &(at, d, v) in &hits {
            let t = bar as f64 * 4.0 + at;
            let Some(i) = chords.iter().position(|c| t >= c.start - 1e-9 && t < c.start + c.len - 1e-9) else {
                continue;
            };
            if held != Some(i) {
                let (low, high) = play.register;
                prev = Some(voice(
                    &chords[i].chord,
                    prev.as_deref(),
                    low,
                    high,
                    play.voices,
                    play.voicing,
                    Some(&mut *rng),
                ));
                held = Some(i);
            }
            let d = d.min(chords[i].start + chords[i].len - t).min(period - t);
            for &p in prev.iter().flatten() {
                out.push(Note { t, d, p, v, lock: None });
            }
        }
    }
    out
}

/// `v` without a semitone between neighbouring voices, which rubs in a close voicing: of the two, the root goes (the
/// bass plays it), else the lower; two voices are left at least
fn unrub(mut v: Vec<i32>, chord: &Chord) -> Vec<i32> {
    v.sort_unstable();
    let mut i = 0;
    while i + 1 < v.len() {
        if v[i + 1] - v[i] == 1 && v.len() > 2 {
            v.remove(if v[i + 1].rem_euclid(12) == chord.root as i32 { i + 1 } else { i });
        } else {
            i += 1;
        }
    }
    v
}

/// the song a harmony is made for: a cycle of its progression's chords, `cycle` beats long, and its key
pub struct Song<'a> {
    pub spans: &'a [ChordSpan],
    pub cycle: f64,
    pub key: &'a Key,
}

/// how a harmony plays: in `style` (`styles`), `voices` notes to a chord voiced `voicing` within `register`; in an
/// idiom, the take `seed` draws, its details from `stream` too (`comp::comp`)
pub struct Play<'a> {
    pub style: &'a str,
    pub voicing: Voicing,
    pub register: (i32, i32),
    pub voices: usize,
    pub seed: u64,
    pub stream: u64,
}

/// What a harmony plays under `melody` (one cycle of it, `period` beats long) in `song`, as `play` says, below the
/// melody wherever it can be.
pub fn harmony(melody: &[Note], period: f64, song: &Song, play: &Play, rng: &mut Rng) -> Vec<Note> {
    let (key, register, voices, voicing) = (song.key, play.register, play.voices, play.voicing);
    let chords = chords(melody, song.spans, song.cycle, key, period);
    if let Some(idiom) = super::comp::idiom(play.style) {
        return super::comp::comp(idiom, &chords, voicing, register, voices, play.seed, play.stream);
    }
    // a pad changes chord as the harmony does, held through each
    if play.style == "sustain" {
        return melody::chord_part(&chords, "sustain", voicing, register, voices, rng);
    }
    if play.style != "block" {
        return in_rhythm(&chords, period, play, rng);
    }
    // the melody's top note at each onset, and the notes long enough, or on a beat, to carry a chord
    let mut tops: Vec<Note> = Vec::new();
    for n in melody {
        match tops.last_mut() {
            Some(last) if (last.t - n.t).abs() < 1e-6 => {
                if n.p > last.p {
                    *last = *n;
                }
            }
            _ => tops.push(*n),
        }
    }
    tops.sort_by(|a, b| a.t.total_cmp(&b.t));
    let carries = |n: &Note| n.d >= 0.5 - 1e-9 || (n.t - n.t.round()).abs() < 1e-6;
    let carried: Vec<Note> = tops.iter().copied().filter(carries).collect();
    let mut out = Vec::new();
    let mut prev: Option<Vec<i32>> = None;
    for (i, n) in carried.iter().enumerate() {
        let chord = chord_at(&chords, n.t).map_or_else(|| Chord::new(key.tonic, Quality::Maj), |s| s.chord);
        // held until the next chord, so that the short notes between ring over it
        let next = carried.get(i + 1).map_or(n.t + n.d, |m| m.t);
        let d = (next - n.t).max(n.d).min(4.0);
        // a whole step at least under the melody: a semitone under it rubs
        let high = (n.p - 2).min(register.1);
        let low = (n.p - 16).max(register.0).min(high - 5);
        let pitches = unrub(voice(&chord, prev.as_deref(), low, high, voices, voicing, Some(&mut *rng)), &chord);
        let melody_pc = n.p.rem_euclid(12);
        for &p in pitches.iter().filter(|p| p.rem_euclid(12) != melody_pc || **p < n.p - 11) {
            out.push(Note { t: n.t, d, p, v: n.v * 0.78, lock: None });
        }
        prev = Some(pitches);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theory::scale::Mode;

    fn note(t: f64, d: f64, p: i32) -> Note {
        Note { t, d, p, v: 0.9, lock: None }
    }

    fn c_major() -> Key {
        Key::new(0, Mode::Major)
    }

    /// C - Am - F - G, a bar each
    fn pop() -> Vec<ChordSpan> {
        ["C", "Am", "F", "G"]
            .iter()
            .enumerate()
            .map(|(i, c)| ChordSpan { start: i as f64 * 4.0, len: 4.0, chord: Chord::parse(c).unwrap() })
            .collect()
    }

    #[test]
    fn a_melody_on_the_songs_chord_tones_keeps_the_songs_chords() {
        // E over C, A over Am, C over F, B over G: a chord tone each
        let line = [note(0.0, 4.0, 76), note(4.0, 4.0, 69), note(8.0, 4.0, 72), note(12.0, 4.0, 71)];
        let got: Vec<String> =
            chords(&line, &pop(), 16.0, &c_major(), 16.0).iter().map(|s| s.chord.symbol(false)).collect();
        assert_eq!(got, ["C", "Am", "F", "G"]);
    }

    #[test]
    fn a_ninth_colours_the_chord_and_a_held_clash_moves_it() {
        // D held over C is its 9th: the song's C stays
        assert_eq!(chords(&[note(0.0, 4.0, 74)], &pop()[..1], 4.0, &c_major(), 4.0)[0].chord.symbol(false), "C");
        // B held over F rubs against it: the key's chord that holds B and shares most with F takes its place
        let f = vec![ChordSpan { start: 0.0, len: 4.0, chord: Chord::parse("F").unwrap() }];
        let moved = chords(&[note(0.0, 4.0, 71)], &f, 4.0, &c_major(), 4.0)[0].chord;
        assert!(moved.has_pc(11) && c_major().contains(moved.root as i32), "{}", moved.symbol(false));
        // a passing B on the offbeat, short, does not
        let passing = [note(0.0, 1.5, 72), note(1.5, 0.25, 71), note(1.75, 2.25, 69)];
        assert_eq!(chords(&passing, &f, 4.0, &c_major(), 4.0)[0].chord.symbol(false), "F");
    }

    #[test]
    fn block_chords_sit_under_each_note_the_melody_on_top_and_pads_hold_through_the_bar() {
        let line = [note(0.0, 1.0, 76), note(1.0, 1.0, 77), note(2.0, 0.25, 79), note(2.25, 1.75, 72)];
        let mut rng = Rng::new(1);
        let (spans, key) = (pop(), c_major());
        let song = Song { spans: &spans, cycle: 16.0, key: &key };
        let play = |style| Play { style, voicing: Voicing::Close, register: (48, 84), voices: 3, seed: 1, stream: 0 };
        let block = harmony(&line, 4.0, &song, &play("block"), &mut rng);
        for n in &line {
            let under: Vec<&Note> = block.iter().filter(|c| (c.t - n.t).abs() < 1e-9).collect();
            if n.d < 0.5 && (n.t - n.t.round()).abs() > 1e-6 {
                assert!(under.is_empty(), "a short passing note carries no chord of its own");
                continue;
            }
            assert!(!under.is_empty() && under.iter().all(|c| c.p < n.p && c.p >= n.p - 16), "{n:?}: {under:?}");
        }
        assert!(block.iter().all(|c| c.v < 0.9), "under the melody, softer");
        let pad = harmony(&line, 4.0, &song, &Play { register: (48, 72), ..play("sustain") }, &mut rng);
        assert!(!pad.is_empty() && pad.iter().all(|c| c.t.abs() < 1e-9 || (c.t - 2.0).abs() < 1e-9), "{pad:?}");
    }

    #[test]
    fn a_chord_rhythm_keeps_to_the_bar_when_the_harmony_changes_halfway_through() {
        // C over F, then a B held from beat 3: the chord changes halfway through the bar
        let f = vec![ChordSpan { start: 0.0, len: 4.0, chord: Chord::parse("F").unwrap() }];
        let line = [note(0.0, 2.0, 72), note(2.0, 2.0, 71)];
        let key = c_major();
        let changed = chords(&line, &f, 4.0, &key, 4.0);
        assert_eq!((changed.len(), changed[1].start), (2, 2.0), "{changed:?}");
        let song = Song { spans: &f, cycle: 4.0, key: &key };
        let play = Play { style: "stabs", voicing: Voicing::Close, register: (48, 72), voices: 3, seed: 1, stream: 0 };
        let stabs = harmony(&line, 4.0, &song, &play, &mut Rng::new(1));
        let at = |t: f64| -> Vec<u8> {
            stabs.iter().filter(|n| (n.t - t).abs() < 1e-9).map(|n| n.p.rem_euclid(12) as u8).collect()
        };
        assert!(stabs.iter().all(|n| [0.0, 1.5, 3.0].iter().any(|t| (n.t - t).abs() < 1e-9)), "{stabs:?}");
        for (t, chord) in [(0.0, changed[0].chord), (1.5, changed[0].chord), (3.0, changed[1].chord)] {
            let pcs = at(t);
            assert!(!pcs.is_empty() && pcs.iter().all(|&pc| chord.has_pc(pc)), "at {t}: {pcs:?}");
        }
        // in a comping idiom too: reggae's skank on the offbeats, each on the chord the harmony has there
        let skank = harmony(&line, 4.0, &song, &Play { style: "skank", ..play }, &mut Rng::new(1));
        assert!(!skank.is_empty());
        for n in &skank {
            let chord = if n.t < 2.0 { changed[0].chord } else { changed[1].chord };
            assert!(n.t.rem_euclid(1.0) > 1e-9, "a skank on the beat at {}", n.t);
            assert!(chord.has_pc(n.p.rem_euclid(12) as u8), "{} at {}: not {chord:?}", n.p, n.t);
        }
    }
}
