//! Idioms: the ways genres play a part, as their players would. An idiom is a few bars a player plays over a chord
//! (`cells`, written a step at a time), the bars they end a phrase on (`turns`), how much of each step they hold a
//! note for (`legato`), and how often they vary its details from bar to bar (`Vary`). A take plays one of the cells,
//! answers it with another in the second bar of a four-bar phrase now and then, and ends every phrase on a turn, so
//! that a line has phrases rather than one bar over and over. The cells come from the high half of the part's seed
//! and the details from the low half, so that a variation of a take (another low half) keeps its rhythm and changes
//! its details.
//!
//! A cell is one bar of steps: 16 sixteenths, or 12 or 24 for triplets (spaces and `|` are only for reading). A step
//! is a rest (`.`), the note before going on (`-`), or a note:
//!
//! | step | the note |
//! |---|---|
//! | `R` | the chord's root, its bass note (`r` softly) |
//! | `L`, `8` | the root an octave down, an octave up |
//! | `3`, `5`, `7` | the chord's third, fifth and seventh above the root (the key's seventh on a chord with none) |
//! | `2`, `4`, `6` | the key's second, fourth and sixth above the root |
//! | `A` | an approach: a semitone under the note after it (over it, coming down) |
//! | `W` | a walk: the key's step from the note before towards the note after |
//! | `x` | a dead note: short and soft, on the pitch before |
//!
//! A `'` before a note accents it, and a `~` after one slides it into the next.

use super::chord::Chord;
use super::scale::Key;
use super::{ChordSpan, Note};
use crate::rng::{Rng, hash_str};

/// How often a take varies an idiom's details, bar by bar: the chance of each.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vary {
    /// a dead note in an empty step after a note
    pub ghost: f64,
    /// a root off the downbeat an octave up, or an octave back down
    pub octave: f64,
    /// the last note before a new chord an approach into it
    pub approach: f64,
    /// a new chord's first note pushed an eighth ahead of its bar
    pub push: f64,
    /// a note off the beat left out
    pub drop: f64,
    /// a note sliding into the next
    pub slide: f64,
}

/// A way of playing a part: the bars it plays (`cells`) and those that end its phrases (`turns`), in the step
/// notation above, its phrases `phrase` bars long, its notes held for `legato` of the steps they span.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Idiom {
    pub name: &'static str,
    /// what it is, in a few words
    pub about: &'static str,
    pub cells: &'static [&'static str],
    pub turns: &'static [&'static str],
    pub phrase: u32,
    pub legato: f64,
    pub vary: Vary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tone {
    Root,
    Low,
    High,
    Third,
    Fifth,
    Seventh,
    Second,
    Fourth,
    Sixth,
    Approach,
    Walk,
    Dead,
}

/// a note of a cell: the step it starts on, how many it holds, what it plays and how
#[derive(Clone, Copy, Debug)]
struct Event {
    step: usize,
    len: usize,
    tone: Tone,
    accent: bool,
    soft: bool,
    slide: bool,
}

/// A cell's notes and the steps its bar has, or what is wrong with it.
fn parse(cell: &str) -> Result<(Vec<Event>, usize), String> {
    let mut events: Vec<Event> = Vec::new();
    let (mut step, mut accent) = (0usize, false);
    for c in cell.chars() {
        let tone = match c {
            ' ' | '|' => continue,
            '\'' => {
                accent = true;
                continue;
            }
            '~' => {
                let Some(e) = events.last_mut() else {
                    return Err(format!("'{cell}': a slide (~) comes after a note"));
                };
                e.slide = true;
                continue;
            }
            '.' => {
                step += 1;
                continue;
            }
            '-' => {
                if let Some(e) = events.last_mut().filter(|e| e.step + e.len == step) {
                    e.len += 1;
                }
                step += 1;
                continue;
            }
            'R' | 'r' => Tone::Root,
            'L' => Tone::Low,
            '8' => Tone::High,
            '3' => Tone::Third,
            '5' => Tone::Fifth,
            '7' => Tone::Seventh,
            '2' => Tone::Second,
            '4' => Tone::Fourth,
            '6' => Tone::Sixth,
            'A' => Tone::Approach,
            'W' => Tone::Walk,
            'x' => Tone::Dead,
            other => return Err(format!("'{cell}': no step '{other}' (. - R r L 8 3 5 7 2 4 6 A W x, ' and ~)")),
        };
        events.push(Event { step, len: 1, tone, accent, soft: c == 'r', slide: false });
        accent = false;
        step += 1;
    }
    if ![12, 16, 24].contains(&step) {
        return Err(format!("'{cell}': {step} steps, where a bar has 16 (or 12 or 24, in triplets)"));
    }
    Ok((events, step))
}

/// Whether every cell and turn of `idiom` reads, or the first that does not and why.
pub fn check(idiom: &Idiom) -> Result<(), String> {
    if idiom.cells.is_empty() {
        return Err(format!("{}: no cells", idiom.name));
    }
    for cell in idiom.cells.iter().chain(idiom.turns) {
        let (events, _) = parse(cell).map_err(|e| format!("{}: {e}", idiom.name))?;
        if events.iter().all(|e| e.tone == Tone::Dead) {
            return Err(format!("{}: '{cell}' plays no note", idiom.name));
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
        0.9
    } else if beat.is_multiple_of(2) && step.is_multiple_of(beat / 2) {
        0.82
    } else {
        0.74
    }
}

/// the pitch of class `pc` nearest to `center`
fn nearest(pc: u8, center: i32) -> i32 {
    let d = (pc as i32 - center).rem_euclid(12);
    center + if d > 6 { d - 12 } else { d }
}

/// the pitch of class `pc` just above `from` (or on it)
fn above(pc: u8, from: i32) -> i32 {
    from + (pc as i32 - from).rem_euclid(12)
}

/// what a tone plays over `chord` from `root` in `key`, within `register`; None for those that depend on the notes
/// either side (an approach, a walk, a dead note)
fn pitch(tone: Tone, chord: &Chord, key: &Key, root: i32, register: (i32, i32)) -> Option<i32> {
    // an octave goes the other way where its own does not fit the register
    let (down, up) = (root - 12 >= register.0 - 3, root + 12 <= register.1 + 3);
    Some(match tone {
        Tone::Root => root,
        Tone::Low | Tone::High if !down && !up => root,
        Tone::Low if down => root - 12,
        Tone::High if up => root + 12,
        Tone::Low => root + 12,
        Tone::High => root - 12,
        Tone::Third => above(chord.third_pc(), root + 1),
        Tone::Fifth => above(chord.fifth_pc(), root + 1),
        Tone::Seventh => match chord.tones().iter().find(|i| matches!(i.rem_euclid(12), 10 | 11)) {
            Some(i) => above(((chord.root as i32 + i).rem_euclid(12)) as u8, root + 1),
            None => key.step(root, 6),
        },
        Tone::Second => key.step(root, 1),
        Tone::Fourth => key.step(root, 3),
        Tone::Sixth => key.step(root, 5),
        Tone::Approach | Tone::Walk | Tone::Dead => return None,
    })
}

/// a note of a line before its pitch is known
#[derive(Clone, Copy, Debug)]
struct Placed {
    t: f64,
    d: f64,
    tone: Tone,
    v: f32,
    slide: bool,
    /// played an eighth ahead of its bar
    pushed: bool,
    /// the chord it is played over
    span: usize,
}

/// The line `idiom` plays over `chords` (a cycle of a progression from beat 0) in `key`, within `register`: the take
/// `seed` draws, its cells from the seed's high half and its details from its low half and `salt`.
pub fn line(idiom: &Idiom, chords: &[ChordSpan], key: &Key, register: (i32, i32), seed: u64, salt: u64) -> Vec<Note> {
    let read = |cells: &[&str]| -> Vec<(Vec<Event>, usize)> { cells.iter().filter_map(|c| parse(c).ok()).collect() };
    let (cells, turns) = (read(idiom.cells), read(idiom.turns));
    if cells.is_empty() || chords.is_empty() {
        return Vec::new();
    }
    let mut take = Rng::new((seed >> 32) ^ hash_str(idiom.name));
    let main = take.below(cells.len());
    let answer =
        if cells.len() > 1 && take.chance(0.5) { (main + 1 + take.below(cells.len() - 1)) % cells.len() } else { main };
    let turn = (!turns.is_empty()).then(|| take.below(turns.len()));
    let mut detail = Rng::new((seed & 0xffff_ffff) ^ salt ^ hash_str(idiom.name));
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
        // the chord after this bar is another (or the cycle comes round): an approach leads into it
        let changes = bar0 + 4.0 >= beats - 1e-9 || span_at(bar0 + 4.0) != here;
        let starts = b > 0 && span_at(bar0 - 1e-6) != here;
        let mut events: Vec<Event> =
            cell.iter().copied().filter(|e| e.step.is_multiple_of(steps / 4) || !detail.chance(v.drop)).collect();
        for e in events.iter_mut().filter(|e| e.step > 0) {
            if detail.chance(v.octave) {
                e.tone = match e.tone {
                    Tone::Root => Tone::High,
                    Tone::High => Tone::Root,
                    t => t,
                };
            }
            if e.tone != Tone::Dead && detail.chance(v.slide) {
                e.slide = true;
            }
        }
        // dead notes in the empty steps right after a note, two in a bar at most
        let mut ghosts = Vec::new();
        for s in 1..steps {
            let empty = !events.iter().any(|e| s >= e.step && s < e.step + e.len);
            let after = events.iter().any(|e| e.step + e.len == s && e.tone != Tone::Dead);
            if empty && after && ghosts.len() < 2 && detail.chance(v.ghost) {
                ghosts.push(Event { step: s, len: 1, tone: Tone::Dead, accent: false, soft: true, slide: false });
            }
        }
        events.extend(ghosts);
        events.sort_by_key(|e| e.step);
        if changes && detail.chance(v.approach) {
            let notes = events.iter().filter(|e| e.tone != Tone::Dead).count();
            if let Some(e) = events.iter_mut().rev().find(|e| e.tone != Tone::Dead && e.step >= steps / 2)
                && notes > 1
            {
                e.tone = Tone::Approach;
            }
        }
        let pushed = starts && detail.chance(v.push);
        for e in events {
            let (mut t, mut d) = (bar0 + e.step as f64 * unit, e.len as f64 * unit);
            if t >= beats - 1e-9 {
                continue;
            }
            let ahead = pushed && e.step == 0 && e.tone != Tone::Dead;
            if ahead {
                (t, d) = (t - 0.5, d + 0.5);
            }
            let mut vel = weight(e.step, steps) * if e.soft { 0.7 } else { 1.0 } + if e.accent { 0.12 } else { 0.0 };
            if e.tone == Tone::Dead {
                (d, vel) = ((unit * 0.6).min(0.1), 0.32);
            } else if !e.slide {
                d = (d * idiom.legato).max(0.08);
            }
            placed.push(Placed {
                t,
                d,
                tone: e.tone,
                v: vel.min(1.0),
                slide: e.slide,
                pushed: ahead,
                span: span_at(bar0 + e.step as f64 * unit),
            });
        }
    }
    // a note pushed ahead of its bar takes the place of what the bar before played there
    let pushes: Vec<f64> = placed.iter().filter(|p| p.pushed).map(|p| p.t).collect();
    placed.retain(|p| p.pushed || !pushes.iter().any(|&t| p.t >= t - 1e-9 && p.t < t + 0.5 - 1e-9));
    placed.sort_by(|a, b| a.t.total_cmp(&b.t));
    placed.dedup_by(|b, a| (a.t - b.t).abs() < 1e-9);
    pitched(&placed, chords, key, register)
}

/// The notes of `placed`, each tone resolved over its chord: each chord's root the one nearest the note the line comes
/// to it from, low enough in the register to leave an octave above it, approaches and walks from the notes either
/// side, and every note sounding until the next starts at most, or sliding into it.
fn pitched(placed: &[Placed], chords: &[ChordSpan], key: &Key, register: (i32, i32)) -> Vec<Note> {
    let (low, high) = (register.0, if register.1 - 9 >= register.0 + 5 { register.1 - 9 } else { register.1 });
    let center = (low + high) / 2;
    let root_from = |span: usize, from: i32| {
        let pc = chords[span].chord.bass_pc();
        let mut r = nearest(pc, from);
        if r > high {
            r -= 12;
        }
        if r < low {
            r += 12;
        }
        if r > high { nearest(pc, center) } else { r }
    };
    let mut roots: Vec<Option<i32>> = vec![None; chords.len()];
    let mut out: Vec<Note> = Vec::with_capacity(placed.len());
    let mut last = center;
    for (i, p) in placed.iter().enumerate() {
        let root = *roots[p.span].get_or_insert_with(|| root_from(p.span, last));
        // the next note with a pitch of its own, round to the start of the cycle, over the root its chord will have
        let next = || {
            (1..=placed.len()).map(|k| (i + k) % placed.len()).find_map(|j| {
                let q = placed[j];
                let r = roots[q.span].unwrap_or_else(|| root_from(q.span, last));
                pitch(q.tone, &chords[q.span].chord, key, r, register)
            })
        };
        let mut note = match (p.tone, pitch(p.tone, &chords[p.span].chord, key, root, register)) {
            (_, Some(n)) => n,
            (Tone::Dead, None) => last,
            // from above coming down, from below going up; from the other side when that is the note before it
            (Tone::Approach, None) => {
                let to = next().unwrap_or(root);
                let from = if last > to { to + 1 } else { to - 1 };
                if from == last { 2 * to - from } else { from }
            }
            // a walk: the key's step towards the next note, or the semitone between when that step is the note itself
            (_, None) => {
                let to = next().unwrap_or(root);
                let up = to >= last;
                let step = key.step(last, if up { 1 } else { -1 });
                if (up && step >= to) || (!up && step <= to) { (last + to) / 2 } else { step }
            }
        };
        while note > register.1 + 3 {
            note -= 12;
        }
        while note < register.0 - 3 {
            note += 12;
        }
        last = note;
        out.push(Note { t: p.t, d: p.d, p: note, v: p.v, lock: None });
    }
    // one note at a time: each ends where the next starts, unless it slides into it
    for i in 0..out.len().saturating_sub(1) {
        let gap = out[i + 1].t - out[i].t;
        out[i].d = if placed[i].slide && out[i + 1].p != out[i].p { gap + 0.06 } else { out[i].d.min(gap) };
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theory::scale::Mode;

    const TEST: Idiom = Idiom {
        name: "test",
        about: "",
        cells: &["R--. 5-.8 R-.. 3-A."],
        turns: &["R--- 5--- 7--- A---"],
        phrase: 4,
        legato: 0.9,
        vary: Vary { ghost: 0.0, octave: 0.0, approach: 0.0, push: 0.0, drop: 0.0, slide: 0.0 },
    };

    fn bars(chords: &[&str]) -> Vec<ChordSpan> {
        chords
            .iter()
            .enumerate()
            .map(|(i, c)| ChordSpan { start: i as f64 * 4.0, len: 4.0, chord: Chord::parse(c).unwrap() })
            .collect()
    }

    #[test]
    fn a_cell_reads_a_step_at_a_time() {
        let (events, steps) = parse("'R--x .8.x R.8~5 .R7A").unwrap();
        assert_eq!(steps, 16);
        let at: Vec<(usize, usize)> = events.iter().map(|e| (e.step, e.len)).collect();
        assert_eq!(at, [(0, 3), (3, 1), (5, 1), (7, 1), (8, 1), (10, 1), (11, 1), (13, 1), (14, 1), (15, 1)]);
        assert!(events[0].accent && events[5].slide && events[3].tone == Tone::Dead);
        assert_eq!(parse("R... R... R...").unwrap().1, 12, "triplets");
        assert!(parse("R... R...").unwrap_err().contains("8 steps"));
        assert!(parse("R... R... R... R..Q").unwrap_err().contains("no step 'Q'"));
        assert!(parse("~R... R... R... R...").unwrap_err().contains("comes after a note"));
    }

    #[test]
    fn a_line_follows_the_changes_and_ends_its_phrase_on_the_turn() {
        let key = Key::new(9, Mode::Minor);
        let chords = bars(&["Am", "F", "C", "G"]);
        let notes = line(&TEST, &chords, &key, (28, 52), 7, 0);
        // the cell over the first three bars: the root on each downbeat, its fifth, octave and third
        for (b, root) in [(0, 9), (1, 5), (2, 0)] {
            let at = |beat: f64| notes.iter().find(|n| (n.t - (b as f64 * 4.0 + beat)).abs() < 1e-9).map(|n| n.p);
            assert_eq!(at(0.0).map(|p| p.rem_euclid(12)), Some(root), "bar {b}");
            let third = chords[b].chord.third_pc() as i32;
            assert_eq!(at(3.0).map(|p| p.rem_euclid(12)), Some(third), "bar {b}");
        }
        // the turn over G: root, fifth, the key's seventh over it (F), and an approach from F down onto the A minor
        // it comes round to: a semitone over it
        let turn: Vec<i32> = notes.iter().filter(|n| n.t >= 12.0).map(|n| n.p).collect();
        assert_eq!(turn.iter().map(|p| p.rem_euclid(12)).collect::<Vec<_>>(), [7, 2, 5, 10], "{turn:?}");
        // one note at a time, in the register
        assert!(notes.windows(2).all(|w| w[0].t + w[0].d <= w[1].t + 1e-9));
        assert!(notes.iter().all(|n| (25..=55).contains(&n.p)), "{notes:?}");
        // the same take, the same line; another seed's low half varies only the details
        assert_eq!(line(&TEST, &chords, &key, (28, 52), 7, 0), notes);
    }

    #[test]
    fn a_variation_keeps_the_takes_cells_and_another_take_may_change_them() {
        const TWO: Idiom = Idiom {
            name: "two",
            cells: &["R... R... R... R...", "R.R. R.R. R.R. R.R."],
            turns: &[],
            vary: Vary { ghost: 0.5, octave: 0.3, approach: 0.0, push: 0.0, drop: 0.0, slide: 0.0 },
            ..TEST
        };
        let key = Key::new(0, Mode::Major);
        let chords = bars(&["C", "C", "C", "C"]);
        let onsets = |seed: u64| -> Vec<f64> {
            line(&TWO, &chords, &key, (28, 52), seed, 0).iter().filter(|n| n.v > 0.5).map(|n| n.t).collect()
        };
        let take = 0x1234_5678_0000_0001u64;
        for low in 2..12u64 {
            assert_eq!(onsets(take & !0xffff_ffff | low), onsets(take), "the same cells, whatever the details");
        }
        assert!((1..40u64).any(|high| onsets(high << 32 | 1) != onsets(take)), "another take plays the other cell");
    }

    #[test]
    fn a_push_plays_the_next_chords_root_an_eighth_early_and_takes_the_place_of_what_was_there() {
        const PUSHED: Idiom = Idiom { vary: Vary { push: 1.0, ..TEST.vary }, turns: &[], ..TEST };
        let key = Key::new(0, Mode::Major);
        let notes = line(&PUSHED, &bars(&["C", "F"]), &key, (28, 52), 3, 0);
        let early = notes.iter().find(|n| (n.t - 3.5).abs() < 1e-9).expect("an eighth before F's bar");
        assert_eq!(early.p.rem_euclid(12), 5, "F's root, early");
        assert!(!notes.iter().any(|n| (n.t - 4.0).abs() < 1e-9), "and not again on its downbeat");
        assert_eq!(notes.iter().filter(|n| n.t >= 3.5 - 1e-9 && n.t < 4.0).count(), 1, "the approach there gave way");
    }
}
