//! From pattern events to notes on the song's timeline: what a value means depends on the pattern's kind (a sound
//! name, a note, a scale degree, a chord), chords are voiced with voice leading and optionally arpeggiated.

use std::collections::BTreeMap;

use super::{Kind, Pattern, Value};
use crate::theory::chord::{Chord, Voicing, roman, voice};
use crate::theory::progression;
use crate::theory::scale::{Key, Mode, parse_pc};
use crate::theory::{ChordSpan, Note, chord_at};

/// a note with the sound that plays it (sound names matter for multi-sound patterns like s("bd, hh*4"))
#[derive(Clone, Debug)]
pub struct Placed {
    pub sound: String,
    pub note: Note,
}

pub struct Context<'a> {
    pub key: Key,
    pub progressions: &'a BTreeMap<String, Vec<String>>,
    pub bars_per_chord: u32,
    /// the progression of the section being written
    pub prog: &'a str,
}

/// MIDI pitch of a note name ("c", "c#4", "eb3", "A2"); octave 3 when absent, like Strudel
pub fn note_name(w: &str, default_octave: i32) -> Option<i32> {
    let mut cs = w.trim().chars();
    let first = cs.next()?.to_ascii_uppercase();
    if !"ABCDEFG".contains(first) {
        return None;
    }
    let rest: String = cs.collect();
    let acc_len = rest.chars().take_while(|c| matches!(c, '#' | 'b' | 's' | 'f')).count();
    let acc: String = rest.chars().take(acc_len).map(|c| if c == 's' || c == '#' { '#' } else { 'b' }).collect();
    let oct_s: String = rest.chars().skip(acc_len).collect();
    let pc = parse_pc(&format!("{first}{acc}"))? as i32;
    let oct = if oct_s.is_empty() { default_octave } else { oct_s.parse::<i32>().ok()? };
    Some(pc + 12 * (oct + 1))
}

/// (tonic, intervals) of a scale like "A:minor", "C major", "D:dorian", "e:minor:pentatonic", "F#:blues"
pub fn scale(spec: &str) -> Option<(u8, Vec<i32>)> {
    let s = spec.replace(':', " ").to_lowercase();
    let mut parts = s.split_whitespace();
    let tonic = parse_pc(parts.next()?)?;
    let rest: Vec<&str> = parts.collect();
    let name = rest.join("_");
    let iv: Vec<i32> = match name.as_str() {
        "" | "major" | "ionian" => Mode::Major.intervals().to_vec(),
        "minor" | "aeolian" | "natural_minor" => Mode::Minor.intervals().to_vec(),
        "pentatonic" | "major_pentatonic" => vec![0, 2, 4, 7, 9],
        "minor_pentatonic" => vec![0, 3, 5, 7, 10],
        "blues" | "minor_blues" => vec![0, 3, 5, 6, 7, 10],
        "chromatic" => (0..12).collect(),
        other => Mode::parse(other)?.intervals().to_vec(),
    };
    Some((tonic, iv))
}

fn degree_pitch(d: i32, tonic: u8, iv: &[i32], octave: i32) -> i32 {
    let n = iv.len().max(1) as i32;
    tonic as i32 + 12 * (octave + 1) + 12 * d.div_euclid(n) + iv[d.rem_euclid(n) as usize]
}

fn chord_of(w: &str, key: &Key, ctx: &Context, bar_beat: f64) -> Option<Chord> {
    if let Some(name) = w.strip_prefix("prog").map(|r| r.trim_start_matches(':')) {
        let name = if name.is_empty() { ctx.prog } else { name };
        // a progression the song does not have plays progression A, as the generated parts do
        let prog = ctx.progressions.get(name).or_else(|| ctx.progressions.get("A")).cloned().unwrap_or_default();
        let len = ctx.bars_per_chord.max(1) as f64 * 4.0;
        let spans: Vec<ChordSpan> = progression::chords(&prog, key)
            .into_iter()
            .enumerate()
            .map(|(i, c)| ChordSpan { start: i as f64 * len, len, chord: c })
            .collect();
        let cycle = spans.last().map_or(16.0, |s| s.start + s.len);
        return chord_at(&spans, bar_beat.rem_euclid(cycle)).map(|s| s.chord);
    }
    Chord::parse(w).or_else(|| roman(w, key))
}

/// notes of `p` for the cycles [from, to) (one cycle = one bar), at `offset_beats` on the timeline
pub fn place(p: &Pattern, from: u32, to: u32, offset_beats: f64, ctx: &Context) -> Result<Vec<Placed>, String> {
    let evs = p.try_events(from as f64, to as f64)?;
    let sound = p.opts.sound.clone().unwrap_or_else(|| match p.kind {
        Kind::Chord => "epiano".into(),
        _ => "pluck".into(),
    });
    let octave = p.opts.octave.unwrap_or(3);
    let scale_spec = p.opts.scale.clone();
    let (tonic, iv) =
        scale_spec.as_deref().and_then(scale).unwrap_or((ctx.key.tonic, ctx.key.mode.intervals().to_vec()));
    let (vlo, vhi) = p.opts.voicing.unwrap_or((52, 74));
    let chord_key = scale_spec.as_deref().and_then(|s| Key::parse(&s.replace(':', " "))).unwrap_or(ctx.key);
    let mut prev_voicing: Option<Vec<i32>> = None;
    let mut out = Vec::new();
    for ev in evs {
        let t = offset_beats + (ev.b - from as f64) * 4.0;
        let len = (ev.e - ev.b) * 4.0;
        let d = (len * ev.ctrl.legato.unwrap_or(1.0)).max(0.02);
        let v =
            ev.ctrl.velocity.or(ev.ctrl.gain).unwrap_or(if p.kind == Kind::Sound { 0.9 } else { 0.8 }).clamp(0.0, 1.5)
                as f32;
        let add = ev.ctrl.add.round() as i32;
        let semis = ev.ctrl.transpose.round() as i32;
        match p.kind {
            Kind::Sound => {
                let name = match &ev.v {
                    Value::Word(w) => w.clone(),
                    Value::Num(n) => super::fmt_num(*n),
                };
                // only a number after the last colon picks a variation: "bd:3", but "tr808:sd" is one name
                let (base, idx) = match name.rsplit_once(':') {
                    Some((b, i)) if i.parse::<i64>().is_ok() => (b.to_string(), i.parse::<i64>().ok()),
                    _ => (name.clone(), None),
                };
                let idx = ev.ctrl.index.or(idx);
                let label = match idx {
                    Some(i) if i != 0 => format!("{base}:{i}"),
                    _ => base,
                };
                out.push(Placed { sound: label, note: Note { t, d, p: 60 + add + semis, v } });
            }
            Kind::Note | Kind::Plain => {
                let pitch = match &ev.v {
                    Value::Num(n) => n.round() as i32,
                    Value::Word(w) => note_name(w, octave).ok_or_else(|| {
                        format!("cannot read '{w}' as a note (names with an octave like c3, f#4, bb2, or MIDI numbers)")
                    })?,
                };
                out.push(Placed { sound: sound.clone(), note: Note { t, d, p: pitch + add + semis, v } });
            }
            Kind::Degree => {
                let Some(deg) = ev.v.num() else {
                    return Err(format!("n() takes scale degrees (0 1 2 -1 ...), not '{}'", super::mini_word(&ev.v)));
                };
                let pitch = degree_pitch(deg.round() as i32 + add, tonic, &iv, octave);
                out.push(Placed { sound: sound.clone(), note: Note { t, d, p: pitch + semis, v } });
            }
            Kind::Chord => {
                let w = super::mini_word(&ev.v);
                let Some(ch) = chord_of(&w, &chord_key, ctx, t - offset_beats + from as f64 * 4.0) else {
                    if w.starts_with("prog") {
                        continue;
                    }
                    return Err(format!(
                        "cannot read '{w}' as a chord (names like Am7, Fmaj7, Bb, numerals like ii7, bVII, V7/V, or prog)"
                    ));
                };
                if p.opts.root {
                    // the bass note of the chord, in the octave asked for (2 by default: bass register)
                    let oct = p.opts.octave.unwrap_or(2);
                    let pitch = ch.bass_pc() as i32 + 12 * (oct + 1) + add + semis;
                    out.push(Placed { sound: sound.clone(), note: Note { t, d, p: pitch, v } });
                    continue;
                }
                let pitches = voice(
                    &ch,
                    prev_voicing.as_deref(),
                    vlo + add,
                    vhi + add,
                    ch.pcs().len().clamp(3, 4),
                    Voicing::Close,
                    None,
                );
                prev_voicing = Some(pitches.clone());
                let pitches: Vec<i32> = pitches.into_iter().map(|p| p + semis).collect();
                match p.opts.arp.as_deref() {
                    Some(mode) if !pitches.is_empty() => {
                        let mut order: Vec<i32> = pitches.clone();
                        match mode {
                            "down" => order.reverse(),
                            "updown" => {
                                let mut back: Vec<i32> = pitches.iter().rev().skip(1).copied().collect();
                                back.pop();
                                order.extend(back);
                            }
                            _ => {}
                        }
                        let step = len / order.len() as f64;
                        for (k, pch) in order.iter().enumerate() {
                            out.push(Placed {
                                sound: sound.clone(),
                                note: Note {
                                    t: t + k as f64 * step,
                                    d: (step * ev.ctrl.legato.unwrap_or(0.9)).max(0.02),
                                    p: *pch,
                                    v,
                                },
                            });
                        }
                    }
                    _ => {
                        for pch in pitches {
                            out.push(Placed { sound: sound.clone(), note: Note { t, d, p: pch, v } });
                        }
                    }
                }
            }
        }
    }
    Ok(out)
}
