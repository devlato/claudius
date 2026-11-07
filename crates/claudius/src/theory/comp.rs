//! Comping: the ways genres' players accompany on chord instruments, as idioms (`idiom::Idiom`) whose cells are
//! written in chords instead of single notes: a house pianist's stabs, a funk guitarist's scratch, reggae's skank and
//! organ bubble, a gospel player's pushes and slips, lo-fi's lazy jazz chords, a ballad's arpeggio, a string section's
//! ostinato, pizzicato, tremolo and disco runs, a horn section's stabs. Each chord is voiced once, leading smoothly
//! from the one before (`chord::voice`), and its cells play it, a step at a time:
//!
//! | step | what sounds |
//! |---|---|
//! | `C`, `c` | the chord, every voice of it (`c` softly) |
//! | `T`, `B` | its top voice; its bottom voice |
//! | `1` to `6` | one voice, counted from the bottom (the top one, of a chord with fewer) |
//! | `U`, `D` | the chord strummed up, from its bottom voice; down, from its top |
//! | `x` | a muted strum: short and soft |
//! | `.`, `-` | a rest; what sounds before going on |
//!
//! A `'` accents the step after it. A take varies its details as a line's are: a soft chord in an empty step after a
//! hit (`ghost`), a hit off the beat left out (`drop`), a new chord's first hit an eighth early (`push`), and on the
//! last sixteenth before a new chord, that chord a semitone up slipping into it (`approach`), as gospel and funk
//! players slide into their changes.

use super::chord::{Voicing, voice};
use super::idiom::{Idiom, Vary};
use super::{ChordSpan, Note};
use crate::rng::{Rng, hash_str};

const STILL: Vary = Vary { ghost: 0.0, octave: 0.0, approach: 0.0, push: 0.0, drop: 0.0, slide: 0.0 };

/// The ways the genres' players comp, each as its players would.
pub const IDIOMS: &[Idiom] = &[
    Idiom {
        name: "house_piano",
        about: "house's piano: short stabbed chords off the beat, three, three and two around the kick",
        cells: &["'C..C ..C. .C.. C...", "..C. .C.. C..C ..C.", "'C..C ..C. ..C. .C.."],
        turns: &["'C..C ..C. .C.C .C.C"],
        phrase: 4,
        legato: 0.35,
        vary: Vary { ghost: 0.05, approach: 0.1, push: 0.2, drop: 0.1, ..STILL },
    },
    Idiom {
        name: "gospel",
        about: "gospel's and neo-soul's comping: chords pushed ahead of the beat, short and long, slipped into",
        cells: &["C-.c .C-. .c.C -.C.", "'C--. .C-. C-.. .C-."],
        turns: &["C-.c .C-. C.C. C-C."],
        phrase: 4,
        legato: 0.8,
        vary: Vary { ghost: 0.15, approach: 0.35, push: 0.3, drop: 0.1, ..STILL },
    },
    Idiom {
        name: "lofi_comp",
        about: "lo-fi's lazy jazz chords: one on the one, another late in the bar, room between",
        cells: &["C--- --.. ...c --..", "C--- ---- --C- ----", ".C-- ---- ..C- ----"],
        turns: &["C--- --.. C--- -.c-"],
        phrase: 4,
        legato: 0.9,
        vary: Vary { ghost: 0.05, approach: 0.15, push: 0.25, drop: 0.1, ..STILL },
    },
    Idiom {
        name: "trap_keys",
        about: "trap's dark keys: the chord held from the one, its top voice picked out above it",
        cells: &["C--- ---- T--- T---", "C--- ---- ---- T---", "C--- --T- --T- ----"],
        turns: &["C--- ---- T--. T-T-"],
        phrase: 4,
        legato: 0.95,
        vary: Vary { push: 0.05, drop: 0.05, ..STILL },
    },
    Idiom {
        name: "rnb_keys",
        about: "R&B's keys: smooth extended chords, pushed into the next, a passing chord slipped in",
        cells: &["C--- -.C- --.. C-..", "C--- ---. .C-- ----"],
        turns: &["C--- -.C- C-.. C-C-"],
        phrase: 4,
        legato: 0.9,
        vary: Vary { ghost: 0.05, approach: 0.25, push: 0.25, drop: 0.05, ..STILL },
    },
    Idiom {
        name: "ballad",
        about: "the ballad pianist's arpeggio: the chord broken from its bottom voice to its top and back, let ring",
        cells: &["B.2.3.T.3.2.3.T.", "B.3.T.3.2.3.T.3."],
        turns: &["B.2.3.T.C---.---"],
        phrase: 4,
        legato: 1.0,
        vary: Vary { drop: 0.05, ..STILL },
    },
    Idiom {
        name: "bubble",
        about: "reggae's organ bubble: soft chords on the eighths, the offbeats pressed",
        cells: &["c.'C.c.'C.c.'C.c.'C."],
        turns: &["c.'C.c.'C.c.'C.'C'C'C."],
        phrase: 4,
        legato: 0.5,
        vary: Vary { drop: 0.05, ..STILL },
    },
    Idiom {
        name: "skank",
        about: "reggae's skank: a short chord chopped on every offbeat",
        cells: &["..'C. ..'C. ..'C. ..'C."],
        turns: &["..'C. ..'C. ..'C. .'C'C."],
        phrase: 4,
        legato: 0.25,
        vary: Vary { ghost: 0.05, ..STILL },
    },
    Idiom {
        name: "scratch",
        about: "funk's chicken scratch: muted strums in sixteenths, a chord hit hard now and then, on the backbeat",
        cells: &["x.xx 'Cxx. x.x'C x.xx", "xx.x 'C.xx .x'Cx xx.x"],
        turns: &["x.xx 'Cxx. 'C.'C. 'Cxx."],
        phrase: 4,
        legato: 0.3,
        vary: Vary { push: 0.1, drop: 0.1, ..STILL },
    },
    Idiom {
        name: "strum",
        about: "a strummed guitar: down on the beats, up on the offbeats, in eighths",
        cells: &["D...D.U...U.D.U.", "D.D.U.D.U.U.D.U."],
        turns: &["D...D.U.D.D.D.D."],
        phrase: 4,
        legato: 0.85,
        vary: Vary { push: 0.15, drop: 0.05, ..STILL },
    },
    Idiom {
        name: "highlife",
        about: "highlife's and afrobeats' guitar: a bright picked pattern through the chord's upper voices, running on",
        cells: &["T.3.2.3.T.3.2.3.", "2.T.3.T.2.T.3.2."],
        turns: &["T.3.2.3.T.T.3.2."],
        phrase: 4,
        legato: 0.6,
        vary: Vary { push: 0.1, drop: 0.1, ..STILL },
    },
    Idiom {
        name: "arpeggio",
        about: "a picked arpeggio: the chord's voices one at a time, up and back",
        cells: &["B.2.3.T.3.2.B.2.", "B.3.2.T.2.3.B.3."],
        turns: &["B.2.3.T.T.3.2.B."],
        phrase: 4,
        legato: 0.7,
        vary: Vary { drop: 0.05, ..STILL },
    },
    Idiom {
        name: "legato",
        about: "a string section's legato: each chord held and bowed again halfway through a phrase's last bar",
        cells: &["C--- ---- ---- ----"],
        turns: &["C--- ---- C--- ----"],
        phrase: 4,
        legato: 1.0,
        vary: Vary { push: 0.1, ..STILL },
    },
    Idiom {
        name: "ostinato",
        about: "the cinematic strings' ostinato: one voice after another in short driving eighths",
        cells: &["'1.1.2.1.'1.1.3.1.", "'1.2.3.2.'1.2.3.2."],
        turns: &["'1.2.3.T.'T.3.2.1."],
        phrase: 4,
        legato: 0.4,
        vary: Vary { drop: 0.05, ..STILL },
    },
    Idiom {
        name: "pizzicato",
        about: "plucked strings: short notes through the chord, room between",
        cells: &["1...3...2...T...", "1.3...2.T...3..."],
        turns: &["1...3...T.3.2.1."],
        phrase: 4,
        legato: 0.25,
        vary: Vary { drop: 0.1, ..STILL },
    },
    Idiom {
        name: "disco_strings",
        about: "disco's strings: stabs off the beat, and a run up through the chord into the next phrase",
        cells: &["..'C. ..C- ..'C. ...."],
        turns: &["..'C. ..C- 1234 T---"],
        phrase: 4,
        legato: 0.6,
        vary: Vary { push: 0.1, drop: 0.05, ..STILL },
    },
    Idiom {
        name: "tremolo",
        about: "tremolo strings: the chord bowed in fast soft sixteenths, for tension",
        cells: &["cccc cccc cccc cccc"],
        turns: &["cccc cccc CCCC CCCC"],
        phrase: 4,
        legato: 0.95,
        vary: STILL,
    },
    Idiom {
        name: "brass_stabs",
        about: "a horn section's stabs: short, hard and syncopated, falling off the beat",
        cells: &["..'C. .... 'C..C ....", "'C... ..'C. .... 'C.C."],
        turns: &["..'C. ..'C. 'C.'C. 'C---"],
        phrase: 4,
        legato: 0.3,
        vary: Vary { push: 0.15, drop: 0.1, ..STILL },
    },
];

/// the comping idiom of that name
pub fn idiom(name: &str) -> Option<&'static Idiom> {
    IDIOMS.iter().find(|i| i.name == name)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Which {
    All,
    Top,
    Bottom,
    /// a voice, counted from the bottom from 0
    Voice(usize),
    Up,
    Down,
    Muted,
}

/// a hit of a cell: the step it starts on, how many it holds, what it sounds and how
#[derive(Clone, Copy, Debug)]
struct Strike {
    step: usize,
    len: usize,
    which: Which,
    soft: bool,
    accent: bool,
}

/// A cell's hits and the steps its bar has, or what is wrong with it.
fn parse(cell: &str) -> Result<(Vec<Strike>, usize), String> {
    let mut hits: Vec<Strike> = Vec::new();
    let (mut step, mut accent) = (0usize, false);
    for c in cell.chars() {
        let which = match c {
            ' ' | '|' => continue,
            '\'' => {
                accent = true;
                continue;
            }
            '.' => {
                step += 1;
                continue;
            }
            '-' => {
                if let Some(h) = hits.last_mut().filter(|h| h.step + h.len == step) {
                    h.len += 1;
                }
                step += 1;
                continue;
            }
            'C' | 'c' => Which::All,
            'T' => Which::Top,
            'B' => Which::Bottom,
            '1'..='6' => Which::Voice(c as usize - '1' as usize),
            'U' => Which::Up,
            'D' => Which::Down,
            'x' => Which::Muted,
            other => return Err(format!("'{cell}': no step '{other}' (. - C c T B 1-6 U D x and ')")),
        };
        hits.push(Strike { step, len: 1, which, soft: c == 'c', accent });
        accent = false;
        step += 1;
    }
    if ![12, 16, 24].contains(&step) {
        return Err(format!("'{cell}': {step} steps, where a bar has 16 (or 12 or 24, in triplets)"));
    }
    Ok((hits, step))
}

/// Whether every cell and turn of `idiom` reads as comping, or the first that does not and why.
pub fn check(idiom: &Idiom) -> Result<(), String> {
    if idiom.cells.is_empty() {
        return Err(format!("{}: no cells", idiom.name));
    }
    for cell in idiom.cells.iter().chain(idiom.turns) {
        let (hits, _) = parse(cell).map_err(|e| format!("{}: {e}", idiom.name))?;
        if hits.is_empty() {
            return Err(format!("{}: '{cell}' plays nothing", idiom.name));
        }
    }
    Ok(())
}

/// how hard a step is played by where it falls: the downbeat hardest, then the beats, the eighths, the rest
fn weight(step: usize, steps: usize) -> f32 {
    let beat = steps / 4;
    if step == 0 {
        1.0
    } else if step.is_multiple_of(beat) {
        0.88
    } else if beat.is_multiple_of(2) && step.is_multiple_of(beat / 2) {
        0.82
    } else {
        0.74
    }
}

/// a hit placed in the cycle, its chord known
#[derive(Clone, Copy, Debug)]
struct Placed {
    t: f64,
    d: f64,
    which: Which,
    v: f32,
    span: usize,
    /// the chord a semitone up, slipping into the one after it
    slip: bool,
    pushed: bool,
}

/// The chords `idiom` comps over `chords` (a cycle of a progression from beat 0), each voiced with `voices` notes
/// (`voicing`) within `register`: the take `seed` draws, its cells from the seed's high half and its details from its
/// low half and `stream`.
pub fn comp(
    idiom: &Idiom,
    chords: &[ChordSpan],
    voicing: Voicing,
    register: (i32, i32),
    voices: usize,
    seed: u64,
    stream: u64,
) -> Vec<Note> {
    let read = |cells: &[&str]| -> Vec<(Vec<Strike>, usize)> { cells.iter().filter_map(|c| parse(c).ok()).collect() };
    let (cells, turns) = (read(idiom.cells), read(idiom.turns));
    if cells.is_empty() || chords.is_empty() {
        return Vec::new();
    }
    let mut take = Rng::new((seed >> 32) ^ hash_str(idiom.name));
    let main = take.below(cells.len());
    let answer =
        if cells.len() > 1 && take.chance(0.5) { (main + 1 + take.below(cells.len() - 1)) % cells.len() } else { main };
    let turn = (!turns.is_empty()).then(|| take.below(turns.len()));
    let mut detail = Rng::new((seed & 0xffff_ffff) ^ stream ^ hash_str(idiom.name));
    let v = idiom.vary;
    let beats = chords.last().map_or(16.0, |c| c.start + c.len);
    let bars = (beats / 4.0).ceil().max(1.0) as usize;
    let phrase = (idiom.phrase as usize).clamp(1, bars);
    let span_at = |t: f64| {
        chords.iter().position(|c| t >= c.start - 1e-9 && t < c.start + c.len - 1e-9).unwrap_or(chords.len() - 1)
    };
    let mut placed: Vec<Placed> = Vec::new();
    for b in 0..bars {
        let (cell, steps) = match turn {
            Some(t) if phrase >= 2 && (b + 1) % phrase == 0 => &turns[t],
            _ if phrase >= 4 && b % phrase == 1 => &cells[answer],
            _ => &cells[main],
        };
        let (steps, unit, bar0) = (*steps, 4.0 / *steps as f64, b as f64 * 4.0);
        let here = span_at(bar0);
        let changes = bar0 + 4.0 >= beats - 1e-9 || span_at(bar0 + 4.0) != here;
        let starts = b > 0 && span_at(bar0 - 1e-6) != here;
        let mut hits: Vec<Strike> =
            cell.iter().copied().filter(|h| h.step.is_multiple_of(steps / 4) || !detail.chance(v.drop)).collect();
        // soft chords in the empty steps right after a hit, two in a bar at most
        let mut ghosts = Vec::new();
        for s in 1..steps {
            let empty = !hits.iter().any(|h| s >= h.step && s < h.step + h.len);
            let after = hits.iter().any(|h| h.step + h.len == s && h.which != Which::Muted);
            if empty && after && ghosts.len() < 2 && detail.chance(v.ghost) {
                ghosts.push(Strike { step: s, len: 1, which: Which::All, soft: true, accent: false });
            }
        }
        hits.extend(ghosts);
        hits.sort_by_key(|h| h.step);
        // the next chord a semitone up on the last sixteenth, slipping into it
        let slip = changes && detail.chance(v.approach);
        if slip {
            let last = steps - steps / 16;
            hits.retain(|h| h.step < last);
            if let Some(h) = hits.last_mut().filter(|h| h.step + h.len > last) {
                h.len = last - h.step;
            }
            hits.push(Strike { step: last, len: steps - last, which: Which::All, soft: false, accent: false });
        }
        let pushed = starts && detail.chance(v.push);
        for h in hits {
            let (mut t, mut d) = (bar0 + h.step as f64 * unit, h.len as f64 * unit);
            if t >= beats - 1e-9 {
                continue;
            }
            let ahead = pushed && h.step == 0;
            if ahead {
                (t, d) = (t - 0.5, d + 0.5);
            }
            let mut vel = weight(h.step, steps) * if h.soft { 0.6 } else { 1.0 } + if h.accent { 0.12 } else { 0.0 };
            if h.which == Which::Muted {
                (d, vel) = ((unit * 0.5).min(0.08), 0.34);
            } else {
                d = (d * idiom.legato).max(0.06);
            }
            let is_slip = slip && h.step == steps - steps / 16 && !ahead;
            placed.push(Placed { t, d, which: h.which, v: vel.min(1.0), span: here, slip: is_slip, pushed: ahead });
        }
    }
    // a hit pushed ahead of its bar takes the place of what the bar before played there
    let pushes: Vec<f64> = placed.iter().filter(|p| p.pushed).map(|p| p.t).collect();
    placed.retain(|p| p.pushed || !pushes.iter().any(|&t| p.t >= t - 1e-9 && p.t < t + 0.5 - 1e-9));
    placed.sort_by(|a, b| a.t.total_cmp(&b.t));
    placed.dedup_by(|b, a| (a.t - b.t).abs() < 1e-9);
    sounded(&placed, chords, voicing, register, voices, seed)
}

/// The notes of `placed`: each chord voiced once, from the voicing before it, a hit ending where the next begins.
fn sounded(
    placed: &[Placed],
    chords: &[ChordSpan],
    voicing: Voicing,
    register: (i32, i32),
    voices: usize,
    seed: u64,
) -> Vec<Note> {
    let mut rng = Rng::new(seed ^ 0x766f_6963_6573);
    let mut voiced: Vec<Vec<i32>> = Vec::with_capacity(chords.len());
    let mut prev: Option<Vec<i32>> = None;
    for c in chords {
        let v = voice(&c.chord, prev.as_deref(), register.0, register.1, voices.max(2), voicing, Some(&mut rng));
        prev = Some(v.clone());
        voiced.push(v);
    }
    let mut out = Vec::new();
    for (i, p) in placed.iter().enumerate() {
        let mut pitches = voiced[p.span].clone();
        pitches.sort_unstable();
        if p.slip {
            // the chord after this one (round to the first, at the end of the cycle), a semitone up
            pitches = voiced[(p.span + 1) % voiced.len()].iter().map(|n| n + 1).collect();
            pitches.sort_unstable();
        }
        let end = placed.get(i + 1).map_or(p.t + p.d, |n| n.t);
        let d = p.d.min(end - p.t).max(0.04);
        let chosen: Vec<(usize, i32)> = match p.which {
            Which::Top => pitches.last().map(|&n| vec![(0, n)]).unwrap_or_default(),
            Which::Bottom => pitches.first().map(|&n| vec![(0, n)]).unwrap_or_default(),
            Which::Voice(k) => {
                pitches.get(k.min(pitches.len().saturating_sub(1))).map(|&n| vec![(0, n)]).unwrap_or_default()
            }
            Which::Down => pitches.iter().rev().copied().enumerate().collect(),
            _ => pitches.iter().copied().enumerate().collect(),
        };
        // a strum's voices a few hundredths of a beat apart
        let gap = if matches!(p.which, Which::Up | Which::Down) { 0.015 } else { 0.0 };
        for (k, n) in chosen {
            let t = p.t + k as f64 * gap;
            out.push(Note { t, d: (d - k as f64 * gap).max(0.03), p: n, v: p.v, lock: None });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theory::chord::Chord;

    fn spans(chords: &[&str]) -> Vec<ChordSpan> {
        chords
            .iter()
            .enumerate()
            .map(|(i, c)| ChordSpan { start: i as f64 * 4.0, len: 4.0, chord: Chord::parse(c).unwrap() })
            .collect()
    }

    const TAKES: [u64; 3] = [1 << 32 | 1, 7 << 32 | 3, 21 << 32 | 9];

    #[test]
    fn every_comping_idiom_reads_and_has_a_name_of_its_own() {
        for i in IDIOMS {
            check(i).unwrap();
            assert!(!i.about.is_empty(), "{}", i.name);
        }
        let mut names: Vec<&str> = IDIOMS.iter().map(|i| i.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), IDIOMS.len());
        assert!(parse("C... C...").unwrap_err().contains("8 steps"));
        assert!(parse("C... C... C... C..Q").unwrap_err().contains("no step 'Q'"));
    }

    #[test]
    fn reggaes_skank_chops_every_offbeat_and_nothing_on_the_beat() {
        for seed in TAKES {
            let notes =
                comp(idiom("skank").unwrap(), &spans(&["Am", "Dm", "Am", "Em"]), Voicing::Close, (55, 76), 3, seed, 0);
            assert!(!notes.is_empty());
            for n in notes.iter().filter(|n| n.v > 0.5) {
                let in_beat = n.t.rem_euclid(1.0);
                assert!((in_beat - 0.5).abs() < 1e-6 || (in_beat - 0.25).abs() < 1e-6, "a chord at {}", n.t);
                assert!(n.d <= 0.25 + 1e-9, "chopped short");
            }
        }
    }

    #[test]
    fn a_strum_spreads_its_voices_and_a_chord_hit_sounds_them_together() {
        let notes = comp(idiom("strum").unwrap(), &spans(&["C"]), Voicing::Close, (52, 76), 4, 1, 0);
        let first: Vec<&Note> = notes.iter().filter(|n| n.t < 0.1).collect();
        assert_eq!(first.len(), 4, "a down strum of four voices");
        assert!(first.windows(2).all(|w| w[0].t < w[1].t && w[0].p > w[1].p), "from the top down, a little apart");
        let house = comp(idiom("house_piano").unwrap(), &spans(&["Am"]), Voicing::Close, (55, 76), 3, 1, 0);
        let first = house.iter().map(|n| n.t).fold(f64::MAX, f64::min);
        assert_eq!(house.iter().filter(|n| (n.t - first).abs() < 1e-9).count(), 3, "a stab: every voice at once");
        assert!(house.iter().all(|n| n.d < 0.4), "stabbed short");
    }

    #[test]
    fn a_funk_scratch_mutes_most_of_its_strums_and_a_string_ostinato_plays_one_voice_at_a_time() {
        let scratch = comp(idiom("scratch").unwrap(), &spans(&["E7", "E7"]), Voicing::Close, (52, 76), 4, 3, 0);
        let muted = scratch.iter().filter(|n| n.v < 0.4).count();
        assert!(muted * 2 > scratch.len(), "{muted} muted of {}", scratch.len());
        let ostinato = comp(idiom("ostinato").unwrap(), &spans(&["Am", "F"]), Voicing::Close, (48, 72), 3, 3, 0);
        for w in ostinato.windows(2) {
            assert!(w[1].t > w[0].t + 1e-9, "one voice at a time");
        }
        assert!(ostinato.iter().all(|n| n.d < 0.25), "short");
    }

    #[test]
    fn a_slip_plays_the_next_chord_a_semitone_up_on_the_last_sixteenth() {
        const SLIPPING: Idiom =
            Idiom { vary: Vary { approach: 1.0, ..STILL }, turns: &[], cells: &["C--- ---- ---- ----"], ..IDIOMS[0] };
        let notes = comp(&SLIPPING, &spans(&["C", "F"]), Voicing::Close, (55, 76), 3, 1, 0);
        let slip: Vec<i32> = notes.iter().filter(|n| (n.t - 3.75).abs() < 1e-9).map(|n| n.p).collect();
        let f: Vec<i32> = notes.iter().filter(|n| (n.t - 4.0).abs() < 1e-9).map(|n| n.p).collect();
        assert_eq!(slip.len(), 3, "{notes:?}");
        let mut up: Vec<i32> = f.iter().map(|p| p + 1).collect();
        up.sort_unstable();
        let mut slip = slip;
        slip.sort_unstable();
        assert_eq!(slip, up, "F a semitone up, then F");
    }
}
