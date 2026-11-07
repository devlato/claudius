//! Basslines: the idioms of the genres' bass players (`IDIOMS`, written as `idiom` reads them: reggae's one drop,
//! funk's slap, disco's octaves, the dembow, a jazz walk ...), and the one-bar motifs of electronic music (sustained,
//! pulse, offbeat, rolling, syncopated, acid, reese) and the 808s that follow the kick, each written once and
//! transposed onto every chord.

use super::idiom::{self, Idiom, Vary};
use super::scale::Key;
use super::{ChordSpan, Note};
use crate::rng::Rng;

const STILL: Vary = Vary { ghost: 0.0, octave: 0.0, approach: 0.0, push: 0.0, drop: 0.0, slide: 0.0 };

/// The ways the genres' bass players play, each as its players would.
pub const IDIOMS: &[Idiom] = &[
    Idiom {
        name: "one_drop",
        about: "reggae's one drop: nothing on the one, roots, fifths and octaves around the drop on three, room between",
        cells: &[".... R-.R -.5- 8-.7", "..R- R-.5 .... 8-5-", ".R-. R-3- 5... 8-7-"],
        turns: &["..R- R-5- 3-.. 5-A-", ".... R-.5 -.8- 7-5-"],
        phrase: 4,
        legato: 0.85,
        vary: Vary { octave: 0.1, approach: 0.35, drop: 0.1, ..STILL },
    },
    Idiom {
        name: "steppers",
        about: "reggae's and dub's steppers: a note on every beat, roots, fifths and octaves",
        cells: &["R-.. R-.. 5-.. 8-..", "R-.. 5-.. R-.. 7-..", "R-.R -.R- 5-.. 8-.."],
        turns: &["R-.. 5-.. 3-.. A-..", "R-.. 8-.. 7-.. 5-.."],
        phrase: 4,
        legato: 0.8,
        vary: Vary { octave: 0.15, approach: 0.3, drop: 0.05, ..STILL },
    },
    Idiom {
        name: "slap",
        about: "funk's slap: the one hit hard, octaves popped on the offbeat sixteenths, dead notes between",
        cells: &["'R--x .8.x R.8x .R7A", "'R.xR x8.R .x8. 5x7."],
        turns: &["'R.x8 .R.8 R.5. 3.A."],
        phrase: 4,
        legato: 0.55,
        vary: Vary { ghost: 0.3, octave: 0.25, approach: 0.4, push: 0.1, drop: 0.1, slide: 0.05 },
    },
    Idiom {
        name: "funk",
        about: "funk's fingerstyle sixteenths: syncopated roots, fifths and octaves, ghosted, pushed into the changes",
        cells: &["R-xR .xR- 5.x8 -R.7", "R.RR x.R. 5-x5 .8x7"],
        turns: &["R-xR .x5- 3.x5 .6A."],
        phrase: 4,
        legato: 0.65,
        vary: Vary { ghost: 0.25, octave: 0.15, approach: 0.4, push: 0.15, drop: 0.1, ..STILL },
    },
    Idiom {
        name: "disco",
        about: "disco's octaves: root and octave in eighths, walking up into the next chord",
        cells: &["R.8. R.8. R.8. R.8.", "R.8. R.8. 5.8. 3.8."],
        turns: &["R.8. R.8. 5.6. 7.A."],
        phrase: 4,
        legato: 0.7,
        vary: Vary { ghost: 0.05, approach: 0.5, push: 0.05, drop: 0.05, ..STILL },
    },
    Idiom {
        name: "garage",
        about: "UK garage's skipping bass: long and short notes off the beat, octave jumps, swung",
        cells: &["R-.R ..R- .8.. R-5.", "R-.. R.8- ..R- .5-.", "R..8 ..R. .R.. 8-.."],
        turns: &["R-.R ..8- .R5. 3.A."],
        phrase: 4,
        legato: 0.75,
        vary: Vary { ghost: 0.1, octave: 0.25, approach: 0.3, push: 0.15, drop: 0.1, slide: 0.05 },
    },
    Idiom {
        name: "dembow",
        about: "reggaeton's dembow: the root on the three-three-two of the tresillo, held under the kick",
        cells: &["R--R --R- R--R --R-", "R--R --R- R--5 --R-"],
        turns: &["R--R --R- 5--3 --A-"],
        phrase: 4,
        legato: 0.9,
        vary: Vary { octave: 0.1, approach: 0.25, drop: 0.05, slide: 0.1, ..STILL },
    },
    Idiom {
        name: "afro",
        about: "afrobeats' bass: smooth and syncopated, on the tresillo's pushes, gliding into the next chord early",
        cells: &["R--R --.5 -.R- ..8-", "R--. 5-.R --.8 -.A.", "R-.. R--5 -.R. .8-."],
        turns: &["R--R --5- 3--. 5-A-"],
        phrase: 4,
        legato: 0.85,
        vary: Vary { ghost: 0.05, octave: 0.15, approach: 0.25, push: 0.3, drop: 0.1, slide: 0.05 },
    },
    Idiom {
        name: "log_drum",
        about: "amapiano's log drum: short pitched hits off the beat, jumping octaves and fifths, bending now and then",
        cells: &["..R. .R.. R..8 ..5.", ".R.. R.5. .R.. 8.R.", "R..R ..8. .R.. 5.R."],
        turns: &["..R. .R.5 .8.. 5.R."],
        phrase: 4,
        legato: 0.5,
        vary: Vary { ghost: 0.05, octave: 0.25, approach: 0.1, push: 0.1, drop: 0.1, slide: 0.15 },
    },
    Idiom {
        name: "walking",
        about: "jazz's walking bass: a note on every beat, chord tones and walks, a chromatic approach into each change",
        cells: &["R--- W--- W--- A---", "R--- 3--- 5--- A---", "R--- 5--- W--- A---", "R--- 3--- W--- A---"],
        turns: &["R--- 5--- 3--- A---", "R--- W--- 5--- A---"],
        phrase: 4,
        legato: 0.92,
        vary: Vary { ghost: 0.1, ..STILL },
    },
    Idiom {
        name: "neo_soul",
        about: "neo-soul's bass: behind the beat and melodic, passing tones and ghosts, sliding into the chords",
        cells: &["R--. .x5- 3-.W 5-.A", "R-.x R5-. .7-. 8-5-"],
        turns: &["R--. 3-5- 7-.5 6-A."],
        phrase: 4,
        legato: 0.85,
        vary: Vary { ghost: 0.25, octave: 0.1, approach: 0.4, push: 0.15, drop: 0.1, slide: 0.15 },
    },
    Idiom {
        name: "soul",
        about: "soul's melodic bass: eighths through the chord tones, walks and pickups into the changes",
        cells: &["R.R5 .8.R .5.3 .2.A", "R-.5 -.R. 3-.5 .6.A"],
        turns: &["R.3. 5.6. 7.5. 3.A."],
        phrase: 4,
        legato: 0.75,
        vary: Vary { ghost: 0.15, octave: 0.1, approach: 0.5, push: 0.15, drop: 0.1, ..STILL },
    },
    Idiom {
        name: "rnb",
        about: "90s R&B's bass: smooth syncopated sixteenths, sevenths and octaves, slides into the next note",
        cells: &["R--. R-.5 -.R- 8~7-.", "R-.R -.5- .3-. 5~R-."],
        turns: &["R--. 3-5- 7-.5 -.A."],
        phrase: 4,
        legato: 0.85,
        vary: Vary { ghost: 0.1, octave: 0.1, approach: 0.35, push: 0.2, drop: 0.1, slide: 0.2 },
    },
    Idiom {
        name: "gfunk",
        about: "G-funk's Moog bass: laid back and syncopated, sliding up to the octave and back",
        cells: &["R--8~ R-.. R-5- 8-..", "R-.R --5. 8~R-. .7-."],
        turns: &["R--8~ R-5- 3-.5 -.A."],
        phrase: 4,
        legato: 0.85,
        vary: Vary { ghost: 0.1, octave: 0.2, approach: 0.3, push: 0.15, drop: 0.1, slide: 0.25 },
    },
    Idiom {
        name: "boom_bap",
        about: "boom bap's bass: the root held from the one, a pickup into beat three, long round notes",
        cells: &["R--- ---R --5- ----", "R--- --.R -.8- R---", "R--- ---. R--5 --.."],
        turns: &["R--- --.5 -.R- 7-A-"],
        phrase: 4,
        legato: 0.92,
        vary: Vary { octave: 0.1, approach: 0.25, push: 0.1, drop: 0.05, slide: 0.05, ..STILL },
    },
    Idiom {
        name: "lofi",
        about: "lo-fi's upright bass: sparse and behind the beat, a fifth on the and of two, an approach into the change",
        cells: &["R--- -.5- R--- -.A-", "R--- ---- 5--- -.W-", "R--. -.R- 3--- 5---"],
        turns: &["R--- 5--- 3--- -.A-"],
        phrase: 4,
        legato: 0.9,
        vary: Vary { ghost: 0.08, octave: 0.05, approach: 0.4, push: 0.1, drop: 0.1, slide: 0.05 },
    },
    Idiom {
        name: "two_step",
        about: "drum and bass's two-step bass: long notes on the kicks of the two-step, the one and the and of three",
        cells: &["R--- ---- --R- ----", "R--- ---- --5- ----", "R--- ---- --R- -8--"],
        turns: &["R--- ---- --R- -.A-"],
        phrase: 4,
        legato: 0.98,
        vary: Vary { octave: 0.1, approach: 0.2, slide: 0.15, ..STILL },
    },
    Idiom {
        name: "jungle_dub",
        about: "jungle's dub bass: a deep sub moving in half notes under the break, reggae's roots and fifths",
        cells: &["R--- ---- 5--- ----", "R--- ---- R--- 5---", "R--- --.. R--- 8---"],
        turns: &["R--- ---- 5--- 3-A-"],
        phrase: 4,
        legato: 0.98,
        vary: Vary { octave: 0.1, approach: 0.3, slide: 0.1, ..STILL },
    },
    Idiom {
        name: "halftime",
        about: "dubstep's half-time bass: long notes and short restarts around the snare on three",
        cells: &["R--- --R- R--- R-R-", "R--R --.. R--- ----"],
        turns: &["R--- --R- 5--- 8-R-"],
        phrase: 4,
        legato: 0.95,
        vary: Vary { octave: 0.2, drop: 0.1, slide: 0.15, ..STILL },
    },
    Idiom {
        name: "dub",
        about: "dub techno's sub: deep, held, with room around it",
        cells: &["R--- --.. R--- --..", "R--- ---. ..R- ----"],
        turns: &["R--- --.. 5--- --A."],
        phrase: 4,
        legato: 0.95,
        vary: Vary { approach: 0.2, ..STILL },
    },
    Idiom {
        name: "house_organ",
        about: "house's organ bass: short syncopated hits, three, three and two, around the four on the floor",
        cells: &["R..R ..R. .R.. R.R.", "..R. ..R. .R.. R...", "R..R ..R. .8.. R.5."],
        turns: &["R..R ..R. .5.. 3.A."],
        phrase: 4,
        legato: 0.6,
        vary: Vary { ghost: 0.05, octave: 0.2, approach: 0.3, push: 0.1, drop: 0.1, ..STILL },
    },
    Idiom {
        name: "deep",
        about: "deep house's bass: warm and longer, off the beat, the fifth at the end of the bar",
        cells: &["R-.. R-.- ..R- ..5-", "..R- ..R- ..R- .5-."],
        turns: &["R-.. R-.- ..5- ..A-"],
        phrase: 4,
        legato: 0.85,
        vary: Vary { octave: 0.15, approach: 0.3, push: 0.1, drop: 0.1, slide: 0.05, ..STILL },
    },
    Idiom {
        name: "tech",
        about: "tech house's rubbery bass: short sixteenths between the kicks, a groove more than a line",
        cells: &["..R. R..R ..R. R..R", ".RR. ..R. .RR. ..R.", "R..R .R.. R..R .R.8"],
        turns: &["..R. R..R ..5. 3..A"],
        phrase: 4,
        legato: 0.5,
        vary: Vary { ghost: 0.05, octave: 0.2, approach: 0.2, drop: 0.1, slide: 0.1, ..STILL },
    },
    Idiom {
        name: "new_jack",
        about: "new jack swing's synth bass: punchy swung sixteenths, octaves and fifths",
        cells: &["R.8. R..R .8.5 ..R.", "R.R8 ..R. 5.R. 8.R."],
        turns: &["R.8. R..5 .3.5 ..A."],
        phrase: 4,
        legato: 0.55,
        vary: Vary { ghost: 0.15, octave: 0.2, approach: 0.3, push: 0.1, drop: 0.1, ..STILL },
    },
    Idiom {
        name: "grime",
        about: "grime's eski bass: sparse square-wave stabs, a lot of room",
        cells: &["R... ..R. ..R. ....", "R... R... ..R. .R..", "R-.. ...R ..R. 8..."],
        turns: &["R... ..R. .8.. 5.A."],
        phrase: 4,
        legato: 0.5,
        vary: Vary { octave: 0.2, approach: 0.2, drop: 0.1, ..STILL },
    },
    Idiom {
        name: "trip_hop",
        about: "trip hop's bass: slow and heavy, the root held, a late second note",
        cells: &["R--- ---- .R-- 5---", "R--- --.R --.- 5-.."],
        turns: &["R--- ---- .5-- 3-A-"],
        phrase: 4,
        legato: 0.95,
        vary: Vary { octave: 0.1, approach: 0.3, push: 0.05, slide: 0.1, ..STILL },
    },
    Idiom {
        name: "big_beat",
        about: "big beat's riff: distorted, syncopated eighths and sixteenths with the octave",
        cells: &["R--. R-.8 .R-. 5-..", "R-.R -.8- R-.5 -..."],
        turns: &["R--. R-.8 .5-. 3-A."],
        phrase: 4,
        legato: 0.7,
        vary: Vary { ghost: 0.05, octave: 0.2, approach: 0.3, drop: 0.1, slide: 0.05, ..STILL },
    },
    Idiom {
        name: "rock",
        about: "rock's bass: eighths on the root, a fill up through the fifth into the next chord",
        cells: &["R.R. R.R. R.R. R.R.", "R.R. R.R. 5.5. 8.R."],
        turns: &["R.R. R.R. 5.6. 7.A."],
        phrase: 4,
        legato: 0.7,
        vary: Vary { octave: 0.05, approach: 0.4, drop: 0.05, ..STILL },
    },
    Idiom {
        name: "drive",
        about: "synthwave's driving bass: sixteenths on the root, accents on the beats, up to the octave",
        cells: &["'RRRR 'RRRR 'RRRR 'RRR8", "'R.RR 'R.RR 'R.RR 'R.8R"],
        turns: &["'RRRR 'RRRR '5555 '8888"],
        phrase: 4,
        legato: 0.8,
        vary: Vary { octave: 0.1, approach: 0.2, ..STILL },
    },
];

/// the idiom of that name
pub fn idiom(name: &str) -> Option<&'static Idiom> {
    IDIOMS.iter().find(|i| i.name == name)
}

/// every style a bassline is written in: the motifs below, then the idioms
pub fn styles() -> Vec<&'static str> {
    STYLES.iter().copied().chain(IDIOMS.iter().map(|i| i.name)).collect()
}

/// A bassline in `style` over `chords` (one cycle of a progression from beat 0) in `key`, within `register`: an
/// idiom's take from `seed` (with `salt` for its details), or a motif; `kicks` are the kick's beats over the cycle,
/// which the 808s follow.
pub fn write(
    style: &str,
    chords: &[ChordSpan],
    key: &Key,
    kicks: &[f64],
    register: (i32, i32),
    seed: u64,
    salt: u64,
) -> Vec<Note> {
    match idiom(style) {
        Some(i) => idiom::line(i, chords, key, register, seed, salt),
        None => bassline(style, chords, key, kicks, (register.0 + register.1) / 2, &mut Rng::new(seed ^ salt)),
    }
}

/// the pitch of class `pc` nearest to `center`
pub fn nearest(pc: u8, center: i32) -> i32 {
    let d = (pc as i32 - center).rem_euclid(12);
    center + if d > 6 { d - 12 } else { d }
}

/// interval markers resolved against the sounding chord (its third / fifth) rather than fixed semitones
const THIRD: i32 = 100;
const FIFTH: i32 = 101;

#[derive(Clone, Copy, Debug)]
struct Step {
    t: f64,
    d: f64,
    /// semitones over the chord's bass note
    iv: i32,
    v: f32,
    slide: bool,
}

/// the motifs of electronic music, and the 808s that follow the kick (`kick_follow`; `glide`, trap's hook 808 sliding
/// up into the end of its phrases; `drill`, sliding from note to note)
pub const STYLES: &[&str] = &[
    "sustain",
    "pulse",
    "root8",
    "offbeat",
    "octave",
    "rolling",
    "syncopated",
    "kick_follow",
    "glide",
    "drill",
    "acid",
    "reese",
];

fn motif(style: &str, kicks_in_bar: &[f64], minor: bool, rng: &mut Rng) -> Vec<Step> {
    let s = |t: f64, d: f64, iv: i32, v: f32| Step { t, d, iv, v, slide: false };
    match style {
        "sustain" | "reese" => vec![s(0.0, 4.0, 0, 0.9)],
        "pulse" => (0..4).map(|b| s(b as f64, 0.5, 0, if b == 0 { 1.0 } else { 0.8 })).collect(),
        "root8" => (0..8)
            .map(|k| {
                s(
                    k as f64 * 0.5,
                    0.42,
                    if k == 7 && rng.chance(0.5) { 12 } else { 0 },
                    if k % 2 == 0 { 0.95 } else { 0.75 },
                )
            })
            .collect(),
        "octave" => (0..8)
            .map(|k| s(k as f64 * 0.5, 0.4, if k % 2 == 1 { 12 } else { 0 }, if k % 2 == 0 { 0.95 } else { 0.8 }))
            .collect(),
        "offbeat" => {
            let mut v: Vec<Step> = (0..4).map(|b| s(b as f64 + 0.5, 0.42, 0, 0.95)).collect();
            if rng.chance(0.4) {
                v.push(s(3.75, 0.2, if rng.chance(0.5) { 12 } else { 7 }, 0.6));
            }
            v
        }
        "rolling" => (0..4)
            .flat_map(|b| (1..4).map(move |k| (b, k)))
            .map(|(b, k)| s(b as f64 + k as f64 * 0.25, 0.22, 0, if k == 1 { 0.95 } else { 0.7 }))
            .collect(),
        "syncopated" => {
            let grooves: [&[usize]; 5] = [
                &[0, 3, 6, 10, 11, 14],
                &[0, 6, 7, 10, 14],
                &[0, 3, 8, 11, 14],
                &[2, 6, 10, 13],
                &[0, 4, 7, 10, 12, 15],
            ];
            let g = grooves[rng.below(grooves.len())];
            g.iter()
                .enumerate()
                .map(|(i, &st)| {
                    let next = g.get(i + 1).copied().unwrap_or(16);
                    let iv = match rng.below(6) {
                        0 => 12,
                        1 => FIFTH,
                        2 if i > 0 => THIRD,
                        _ => 0,
                    };
                    s(
                        st as f64 * 0.25,
                        ((next - st) as f64 * 0.25 * 0.7).min(0.75),
                        if i == 0 { 0 } else { iv },
                        if st % 4 == 0 { 1.0 } else { 0.78 },
                    )
                })
                .collect()
        }
        "kick_follow" => {
            let mut ks: Vec<f64> = kicks_in_bar.to_vec();
            if ks.is_empty() {
                ks = vec![0.0, 2.5];
            }
            ks.sort_by(|a, b| a.partial_cmp(b).unwrap());
            ks.iter()
                .enumerate()
                .map(|(i, &t)| {
                    let next = ks.get(i + 1).copied().unwrap_or(4.0);
                    let iv = if i > 0 && rng.chance(0.25) { [12, 7, -5, 10][rng.below(4)] } else { 0 };
                    Step { t, d: (next - t).max(0.25), iv, v: 1.0, slide: i > 0 && rng.chance(0.3) }
                })
                .collect()
        }
        "acid" => {
            let pent = if minor { [0, 3, 5, 7, 10, 12] } else { [0, 2, 4, 7, 9, 12] };
            (0..16)
                .filter_map(|k| {
                    if k > 0 && rng.chance(0.3) {
                        return None;
                    }
                    let iv = if k == 0 || rng.chance(0.45) { 0 } else { pent[rng.below(pent.len())] }
                        + if rng.chance(0.15) { 12 } else { 0 };
                    Some(Step {
                        t: k as f64 * 0.25,
                        d: 0.2,
                        iv,
                        v: if rng.chance(0.3) { 1.0 } else { 0.62 },
                        slide: rng.chance(0.2),
                    })
                })
                .collect()
        }
        _ => vec![s(0.0, 4.0, 0, 0.9)],
    }
}

/// write a bassline over `chords`; `kicks` = absolute kick beats (for kick-locked styles); `center` = register
pub fn bassline(style: &str, chords: &[ChordSpan], key: &Key, kicks: &[f64], center: i32, rng: &mut Rng) -> Vec<Note> {
    let minor = key.mode.is_minor();
    let bar_kicks: Vec<f64> = kicks.iter().filter(|k| **k < 4.0).copied().collect();
    let m = motif(style, &bar_kicks, minor, rng);
    let mut out: Vec<Note> = Vec::new();
    let mut prev = center;
    for span in chords {
        let root = {
            let r = nearest(span.chord.bass_pc(), prev);
            if (r - center).abs() > 7 { nearest(span.chord.bass_pc(), center) } else { r }
        };
        let bars = (span.len / 4.0).ceil().max(1.0) as usize;
        for b in 0..bars {
            for st in &m {
                let t = span.start + b as f64 * 4.0 + st.t;
                if t >= span.start + span.len - 1e-6 {
                    continue;
                }
                let mut p = match st.iv {
                    THIRD => nearest(span.chord.third_pc(), root + 4),
                    FIFTH => nearest(span.chord.fifth_pc(), root + 7),
                    iv => root + iv,
                };
                if style == "acid" {
                    p = key.snap(p);
                }
                let mut d = st.d.min(span.start + span.len - t);
                if style == "sustain" || style == "reese" {
                    d = (span.start + span.len - t).min(4.0 * bars as f64);
                }
                if st.slide {
                    d += 0.06;
                }
                out.push(Note { t, d, p, v: st.v, lock: None });
            }
            if style == "sustain" || style == "reese" {
                break;
            }
        }
        prev = root;
    }
    // kick-locked 808s also hit on every kick, not just the first bar's pattern
    if matches!(style, "kick_follow" | "glide" | "drill") && !kicks.is_empty() {
        out = follow(style, chords, key, kicks, center, rng);
    }
    out
}

/// An 808 on every kick in `kicks`, held to the next: now and then an octave or a fifth that slides back
/// (`kick_follow`); long, gliding up an octave or to the fifth into the end of each two-bar phrase (`glide`); or
/// moving through the key's tones over the root, sliding from note to note (`drill`).
fn follow(style: &str, chords: &[ChordSpan], key: &Key, kicks: &[f64], center: i32, rng: &mut Rng) -> Vec<Note> {
    let mut ks = kicks.to_vec();
    ks.sort_by(|a, b| a.total_cmp(b));
    let mut notes: Vec<(f64, f64, i32)> = Vec::new();
    let mut slides: Vec<bool> = Vec::new();
    for (i, &t) in ks.iter().enumerate() {
        let Some(span) = super::chord_at(chords, t) else { continue };
        let root = nearest(span.chord.bass_pc(), center);
        let next = ks.get(i + 1).copied().unwrap_or(span.start + span.len);
        let (iv, slide) = match style {
            "glide" => {
                let ends = ks.get(i + 1).is_none_or(|n| (n / 8.0).floor() != (t / 8.0).floor());
                if ends && rng.chance(0.6) {
                    ([12, 7][rng.below(2)], false)
                } else {
                    (if rng.chance(0.12) { [7, 10][rng.below(2)] } else { 0 }, rng.chance(0.35))
                }
            }
            "drill" => {
                let iv = if i > 0 && rng.chance(0.55) { [3, 5, 7, 10, 12, -2][rng.below(6)] } else { 0 };
                (iv, rng.chance(0.8))
            }
            _ => {
                let iv = if i % 4 == 3 && rng.chance(0.35) { [12, 7, 10][rng.below(3)] } else { 0 };
                (iv, iv != 0 && rng.chance(0.5))
            }
        };
        let p = if style == "drill" { key.snap(root + iv) } else { root + iv };
        notes.push((t, (next - t).clamp(0.25, 4.0), p));
        slides.push(slide);
    }
    // a glide bends into the note after it, where that one is another
    if style != "kick_follow" {
        for i in 0..notes.len() {
            let into = notes.get(i + 1).is_some_and(|n| n.2 != notes[i].2);
            slides[i] = into && (slides[i] || style == "glide" && notes[i + 1].2 > notes[i].2);
        }
    }
    notes
        .iter()
        .zip(slides)
        .map(|(&(t, d, p), s)| Note { t, d: d + if s { 0.06 } else { 0.0 }, p, v: 1.0, lock: None })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theory::chord::Chord;
    use crate::theory::scale::Mode;

    fn spans(chords: &[&str]) -> Vec<ChordSpan> {
        chords
            .iter()
            .enumerate()
            .map(|(i, c)| ChordSpan { start: i as f64 * 4.0, len: 4.0, chord: Chord::parse(c).unwrap() })
            .collect()
    }

    fn a_minor() -> Key {
        Key::new(9, Mode::Minor)
    }

    /// the notes played firmly (no dead notes) of a take of `style` over A minor's i iv i v
    fn firm(style: &str, seed: u64) -> Vec<Note> {
        let notes = write(style, &spans(&["Am", "Dm", "Am", "Em"]), &a_minor(), &[], (28, 50), seed, 0);
        notes.into_iter().filter(|n| n.v > 0.5).collect()
    }

    const TAKES: [u64; 4] = [1 << 32 | 1, 7 << 32 | 3, 21 << 32 | 9, 99 << 32 | 4];

    #[test]
    fn every_idiom_reads_and_every_genre_plays_styles_its_parts_have() {
        for i in IDIOMS {
            idiom::check(i).unwrap();
            assert!(!i.about.is_empty(), "{}", i.name);
        }
        let mut names = styles();
        let all = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), all, "no two styles of one name");
        for g in crate::genres::all().unwrap() {
            for p in &g.parts {
                let known = crate::compose::styles_for(p.role);
                for s in &p.styles {
                    assert!(
                        known.contains(&s.as_str()),
                        "{}: {} plays '{s}', no {} style",
                        g.name,
                        p.name,
                        p.role.name()
                    );
                }
            }
        }
    }

    #[test]
    fn reggaes_one_drop_leaves_the_one_empty() {
        for seed in TAKES {
            let notes = firm("one_drop", seed);
            assert!(!notes.is_empty());
            for bar in 0..4 {
                assert!(!notes.iter().any(|n| (n.t - bar as f64 * 4.0).abs() < 1e-6), "a note on the one of bar {bar}");
            }
        }
    }

    #[test]
    fn the_dembow_plays_the_tresillo_from_every_downbeat() {
        let tresillo = [0.0, 0.75, 1.5, 2.0, 2.75, 3.5];
        for seed in TAKES {
            let notes = firm("dembow", seed);
            assert!(notes.iter().all(|n| tresillo.iter().any(|t| (n.t.rem_euclid(4.0) - t).abs() < 1e-6)), "{notes:?}");
            assert!((0..4).all(|bar| notes.iter().any(|n| (n.t - bar as f64 * 4.0).abs() < 1e-6)));
        }
    }

    #[test]
    fn a_walking_bass_plays_every_beat_and_approaches_each_change_by_a_semitone() {
        let chords = spans(&["Am", "Dm", "Am", "Em"]);
        for seed in TAKES {
            let notes = firm("walking", seed);
            for beat in 0..16 {
                assert!(notes.iter().any(|n| (n.t - beat as f64).abs() < 1e-6), "beat {beat}");
            }
            for bar in 0..4 {
                let four = notes.iter().find(|n| (n.t - (bar as f64 * 4.0 + 3.0)).abs() < 1e-6).unwrap();
                let next = chords[(bar + 1) % 4].chord.bass_pc() as i32;
                assert!(matches!((four.p - next).rem_euclid(12), 1 | 11), "bar {bar}: {} into {next}", four.p);
            }
        }
    }

    #[test]
    fn funks_slap_ghosts_its_dead_notes_and_pops_the_octave() {
        for seed in TAKES {
            let notes = write("slap", &spans(&["Am", "Dm", "Am", "Em"]), &a_minor(), &[], (28, 52), seed, 0);
            assert!(notes.iter().any(|n| n.v < 0.4 && n.d <= 0.1 + 1e-9), "dead notes");
            let firm: Vec<&Note> = notes.iter().filter(|n| n.v > 0.5).collect();
            assert!(firm.windows(2).any(|w| (w[0].p - w[1].p).abs() == 12), "an octave popped");
        }
    }

    #[test]
    fn drum_and_bass_holds_its_notes_on_the_two_step() {
        for seed in TAKES {
            let notes = firm("two_step", seed);
            for bar in 0..3 {
                let at = |beat: f64| notes.iter().find(|n| (n.t - (bar as f64 * 4.0 + beat)).abs() < 1e-6);
                assert!(at(0.0).is_some_and(|n| n.d >= 2.0) && at(2.5).is_some(), "bar {bar}: {notes:?}");
            }
        }
    }

    #[test]
    fn drills_808_slides_from_note_to_note_in_the_key_and_traps_glides_into_its_phrase_ends() {
        let chords = spans(&["Am", "F", "Dm", "E"]);
        // drill's kick: the one and the and of three, then two late kicks in the bar after
        let kicks: Vec<f64> = (0..2).flat_map(|k| [0.0, 2.5, 5.5, 6.75].map(|t| t + 8.0 * k as f64)).collect();
        let (mut slides, mut changes) = (0, 0);
        for seed in 1..=8 {
            let drill = write("drill", &chords, &a_minor(), &kicks, (24, 40), seed, 0);
            assert_eq!(drill.len(), kicks.len());
            assert!(drill.iter().all(|n| a_minor().contains(n.p)), "{drill:?}");
            changes += drill.windows(2).filter(|w| w[0].p != w[1].p).count();
            slides += drill.windows(2).filter(|w| w[0].t + w[0].d > w[1].t + 1e-9).count();
        }
        assert!(changes >= 24 && slides * 10 >= changes * 6, "{slides} slides in {changes} changes of note");
        let glide = write("glide", &chords, &a_minor(), &kicks, (24, 40), 5, 0);
        let jumps = glide.windows(2).filter(|w| w[1].p - w[0].p >= 7 && w[0].t + w[0].d > w[1].t).count();
        assert!(jumps >= 1, "a glide up into a phrase's end: {glide:?}");
    }
}
