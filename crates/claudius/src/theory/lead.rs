//! Leads: the ways genres' lead players and producers write a top line, as idioms (`idiom::Idiom`) in the notes a
//! bass line's are written in, played up in the lead's register: G-funk's portamento whine, trap's and plugg's
//! bells, drill's triplets, afrobeats' flute, the dembow hook, synthwave's leaps, trance's riff, house's organ,
//! disco's octaves, horn riffs, reggae's horn lines, a synth riff, garage's chopped vocal, lo-fi's lazy sevenths,
//! R&B's runs, a rock lead, an ambient line, techno's stab, phonk's cowbell and a dark piano riff. A lead follows the
//! chords: its notes are each chord's tones and the key's steps above its root, and its phrases end on a turn.

use super::idiom::{Idiom, Vary};

const STILL: Vary = Vary { ghost: 0.0, octave: 0.0, approach: 0.0, push: 0.0, drop: 0.0, slide: 0.0 };

/// The ways the genres' leads go, each as its players would write it.
pub const IDIOMS: &[Idiom] = &[
    Idiom {
        name: "gfunk_whine",
        about: "G-funk's whine: a high portamento synth gliding between long notes, the fifth up to the octave and down",
        cells: &["5~--- ---- 8~--- ----", "8--- 5~--- ---- 3~---"],
        turns: &["3~--- 5~--- 8~--- 7-5-"],
        phrase: 4,
        legato: 1.0,
        vary: Vary { slide: 0.3, approach: 0.1, push: 0.1, ..STILL },
    },
    Idiom {
        name: "trap_bells",
        about: "trap's bells: a dark motif in eighths, the fifth and the third around the root, an octave now and then",
        cells: &["5.5. 3.5. R.3. 5...", "R.3. 5.3. R... 5.3.", "5..5 ..3. R..3 ..R."],
        turns: &["5.5. 3.5. 8.7. 5.3."],
        phrase: 4,
        legato: 0.5,
        vary: Vary { octave: 0.1, drop: 0.08, ..STILL },
    },
    Idiom {
        name: "drill_melody",
        about: "drill's melodies: a piano or a flute in triplets, dark and sliding, a note held over the bar now and then",
        cells: &["5.35.3 R..3..", "R..3.5 ..6.5.", "8.75.. 3.5R--"],
        turns: &["5.6.5. 3.R..."],
        phrase: 4,
        legato: 0.75,
        vary: Vary { slide: 0.25, drop: 0.1, ..STILL },
    },
    Idiom {
        name: "afro_flute",
        about: "afrobeats' flute: a bright call pushed ahead of the beat and answered, room between",
        cells: &["5.3. R..5 .6.5 ..3.", "..5. 6.5. 3.R. ....", "R.3. 5..6 5..3 R..."],
        turns: &["5.6. 5.3. R.2. R---"],
        phrase: 4,
        legato: 0.6,
        vary: Vary { push: 0.2, drop: 0.1, approach: 0.05, ..STILL },
    },
    Idiom {
        name: "dembow_hook",
        about: "reggaeton's hook: the melody on the dembow's tresillo, three, three and two",
        cells: &["R..3 ..5. R..3 ..5.", "5..5 ..3. 5..6 ..5.", "8..7 ..5. 3..5 ..R."],
        turns: &["5..6 ..5. 3..2 ..R."],
        phrase: 4,
        legato: 0.7,
        vary: Vary { push: 0.1, drop: 0.05, ..STILL },
    },
    Idiom {
        name: "synth_lead",
        about: "synthwave's lead: long notes leaping an octave, falling home at the phrase's end",
        cells: &["R--- 5--- 8--- 7-5-", "3--- ---- 5--- 3-2-", "8--- --7- 5--- ----"],
        turns: &["5--- 3--- 2--- R---"],
        phrase: 4,
        legato: 0.95,
        vary: Vary { octave: 0.15, slide: 0.1, ..STILL },
    },
    Idiom {
        name: "trance_lead",
        about: "trance's riff: gated eighths through the chord up to the octave and back, a falling run to end the phrase",
        cells: &["R.5. 8.5. R.5. 8.7.", "3.5. 8.5. 3.5. 8.5."],
        turns: &["8.7. 5.3. 5.3. 2.R."],
        phrase: 4,
        legato: 0.5,
        vary: Vary { octave: 0.1, drop: 0.05, ..STILL },
    },
    Idiom {
        name: "organ_riff",
        about: "house's organ riff: short notes off the beat, syncopated around the kick",
        cells: &["R..R ..5. .R.. 3...", "..R. .R.. 5..3 ..R."],
        turns: &["R..R ..5. .3.5 .8.."],
        phrase: 4,
        legato: 0.4,
        vary: Vary { push: 0.15, drop: 0.1, ..STILL },
    },
    Idiom {
        name: "disco_hook",
        about: "disco's and dance pop's hook: bright octaves and chord tones in eighths, a run down to end it",
        cells: &["8.5. 3.5. 8.5. 6.5.", "R.3. 5.8. 7.5. 3..."],
        turns: &["8.7. 6.5. 3.2. R..."],
        phrase: 4,
        legato: 0.6,
        vary: Vary { octave: 0.1, push: 0.1, ..STILL },
    },
    Idiom {
        name: "horn_riff",
        about: "a horn section's riff: short and punchy, the root and the fifth with rests between, answered",
        cells: &["'R..R .... '5.3. ....", "..'5. '3.R. .... ....", "'R.'R. ..5. '3... ...."],
        turns: &["'5.'5. '3.'R. 2.R. ...."],
        phrase: 4,
        legato: 0.45,
        vary: Vary { push: 0.15, drop: 0.1, ..STILL },
    },
    Idiom {
        name: "reggae_horns",
        about: "reggae's horns and melodica: a long, lazy line answering the singer, from the fifth down to the root",
        cells: &[".... 5-3- R--- ....", "3-5- 6-5- 3--- ...."],
        turns: &["5-6- 5-3- 2--- R---"],
        phrase: 4,
        legato: 0.9,
        vary: Vary { approach: 0.1, push: 0.1, ..STILL },
    },
    Idiom {
        name: "riff",
        about: "a synth riff: eighths and sixteenths round the root, leaping an octave, as grime's, electro's and rage's go",
        cells: &["R.R. 8.R. 5.R. 3.R.", "R..R ..8. R..7 ..5.", "RR8R .R5R .R3R .R5."],
        turns: &["8.7. 5.3. R.L. R..."],
        phrase: 4,
        legato: 0.4,
        vary: Vary { octave: 0.2, drop: 0.05, ..STILL },
    },
    Idiom {
        name: "garage_chop",
        about: "UK garage's chopped vocal: short stabs skipping off the beat",
        cells: &["5..3 ..R. .5.. 3...", "..5. 3..R ..5. ..3."],
        turns: &["5..6 ..5. 3..R ...."],
        phrase: 4,
        legato: 0.35,
        vary: Vary { push: 0.1, drop: 0.15, ..STILL },
    },
    Idiom {
        name: "lofi_melody",
        about: "lo-fi's and neo-soul's melody: lazy and sparse, on the sevenths and ninths, slipping into the next chord",
        cells: &["3--. .5-. ---- 7---", "..5- --3- ---- 2-R-", "7--- 5--- 3--- ----"],
        turns: &["3-2- R--- 6--- 5---"],
        phrase: 4,
        legato: 0.9,
        vary: Vary { approach: 0.2, push: 0.1, drop: 0.1, ..STILL },
    },
    Idiom {
        name: "rnb_runs",
        about: "R&B's melody: long smooth notes, and a run down the scale at the phrase's end",
        cells: &["5--- -6-5 3--- ----", "8-7- 5-3- 5--- ----"],
        turns: &["6543 2-R- ---- ----"],
        phrase: 4,
        legato: 0.95,
        vary: Vary { approach: 0.15, slide: 0.1, ..STILL },
    },
    Idiom {
        name: "rock_lead",
        about: "a rock guitar's lead: eighths up the chord and back, a bend into the root",
        cells: &["R.3.5.8. 7.5.3.R.", "5.5.6.5. 3~--R ...."],
        turns: &["8.7.5.3. 5~--- ----"],
        phrase: 4,
        legato: 0.8,
        vary: Vary { slide: 0.2, approach: 0.1, ..STILL },
    },
    Idiom {
        name: "ambient_line",
        about: "an ambient line: a note now and then, held long, an octave away",
        cells: &["5--- ---- ---- ----", "---- 3--- ---- ----"],
        turns: &["8--- ---- 7--- ----"],
        phrase: 4,
        legato: 1.0,
        vary: Vary { octave: 0.2, ..STILL },
    },
    Idiom {
        name: "techno_stab",
        about: "techno's stab: one note, hypnotic, off the beat or every three sixteenths against it, a fifth now and then",
        cells: &["..R. ..R. ..R. ..5.", "R..R ..R. .R.. R..."],
        turns: &["..R. ..R. ..5. .8.."],
        phrase: 4,
        legato: 0.3,
        vary: Vary { drop: 0.1, octave: 0.1, ..STILL },
    },
    Idiom {
        name: "phonk_cowbell",
        about: "phonk's cowbell: a Memphis melody in sixteenths on a pitched cowbell",
        cells: &["R.R5 .R.R 5.R. 3.5.", "R..R .5.R ..R. 5.3."],
        turns: &["R.R5 .R.R 8.7. 5.3."],
        phrase: 4,
        legato: 0.3,
        vary: Vary { drop: 0.05, ..STILL },
    },
    Idiom {
        name: "piano_riff",
        about: "the 90s' dark piano riff: the fifth hammered, falling to the third and the root, room between",
        cells: &["5..5 ..3. 5... ..3.", "R... 3..5 ..3. R..."],
        turns: &["5..5 ..6. 5..3 ..R."],
        phrase: 4,
        legato: 0.6,
        vary: Vary { drop: 0.05, push: 0.05, ..STILL },
    },
];

/// the lead idiom of that name
pub fn idiom(name: &str) -> Option<&'static Idiom> {
    IDIOMS.iter().find(|i| i.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theory::chord::Chord;
    use crate::theory::idiom::{check, line};
    use crate::theory::scale::{Key, Mode};
    use crate::theory::{ChordSpan, Note};

    fn spans(chords: &[&str]) -> Vec<ChordSpan> {
        chords
            .iter()
            .enumerate()
            .map(|(i, c)| ChordSpan { start: i as f64 * 4.0, len: 4.0, chord: Chord::parse(c).unwrap() })
            .collect()
    }

    fn take(name: &str, seed: u64) -> Vec<Note> {
        let key = Key::new(9, Mode::Minor);
        line(idiom(name).unwrap(), &spans(&["Am", "F", "C", "G"]), &key, (67, 88), seed, 0)
    }

    #[test]
    fn every_lead_idiom_reads_and_has_a_name_of_its_own_and_none_of_a_bass_or_comping_idiom() {
        for i in IDIOMS {
            check(i).unwrap();
            assert!(!i.about.is_empty(), "{}", i.name);
            let taken = super::super::bass::idiom(i.name).is_some() || super::super::comp::idiom(i.name).is_some();
            assert!(!taken, "{} is a bass line's or a chord part's idiom too", i.name);
        }
        let mut names: Vec<&str> = IDIOMS.iter().map(|i| i.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), IDIOMS.len());
    }

    #[test]
    fn a_lead_plays_in_its_register_and_in_the_key_on_the_beat() {
        // A minor: a downbeat is the chord's or the key's, and only an approach, late in a bar, is chromatic
        let in_key = |p: i32| [9, 11, 0, 2, 4, 5, 7].contains(&p.rem_euclid(12));
        for i in IDIOMS {
            for seed in [1u64 << 32 | 1, 9 << 32 | 4] {
                let notes = take(i.name, seed);
                assert!(!notes.is_empty(), "{}", i.name);
                let pitches: Vec<i32> = notes.iter().map(|n| n.p).collect();
                assert!(pitches.iter().all(|p| (64..=91).contains(p)), "{}: {pitches:?}", i.name);
                for n in notes.iter().filter(|n| n.t.rem_euclid(4.0) < 1e-9) {
                    assert!(in_key(n.p), "{}: {} on the downbeat at {}", i.name, n.p, n.t);
                }
            }
        }
        // the dembow hook on the tresillo, three, three and two sixteenths apart, pushed or not
        for seed in [1u64 << 32 | 1, 5 << 32 | 2, 8 << 32 | 3] {
            for n in take("dembow_hook", seed) {
                let step = ((n.t * 4.0).round() as i64).rem_euclid(8);
                assert!([0, 3, 6].contains(&step), "a note on step {step} of the tresillo");
            }
        }
        // drill's in triplets
        let triplet = |n: &Note| (n.t * 3.0 - (n.t * 3.0).round()).abs() < 1e-6 && (n.t * 4.0).fract().abs() > 1e-6;
        assert!(take("drill_melody", 1 << 32 | 1).iter().any(triplet));
        // G-funk's whine slides: a note held into the next, at another pitch
        let whine = take("gfunk_whine", 1 << 32 | 1);
        assert!(whine.windows(2).any(|w| w[0].t + w[0].d > w[1].t + 1e-9 && w[0].p != w[1].p));
    }
}
