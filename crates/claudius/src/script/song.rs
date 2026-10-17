//! Starting songs and the song-wide settings scripts call: `song`, `blank`, `tempo`, `key`, `swing`, `prog`,
//! `bars_per_chord`, `drum` and `track`. Parts and the mix are in `mix`, the form in `form`.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use rhai::{Dynamic, Engine, INT, Map};

use crate::compose::{self, Options};
use crate::dsp::env::Param;
use crate::dsp::filter::FilterMode;
use crate::fx::{Effect, SatKind};
use crate::genres::{self, GenreProfile};
use crate::model::*;
use crate::pattern::{Pattern, notes};
use crate::synth::drums::DrumParams;
use crate::theory::scale::Key;

use super::sounds::{drum_name, resolve_sound, role_of};
use super::steps::{hits_to_steps, steps_to_hits, tile_hits};
use super::{Ctx, MAX_NOTES_PER_BAR, R, constant, fail, num, text};

/// a diatonic chord moved to another mode on the same degree (I maj7 in major becomes i7 in minor); borrowed and
/// applied chords stay as written
pub fn remode(rn: &str, from: &Key, to: &Key) -> String {
    use crate::theory::{chord, progression};
    if rn.starts_with(['b', '#', '♭', '♯']) || rn.contains('/') {
        return rn.to_string();
    }
    let Some(c) = chord::roman(rn, from) else { return rn.to_string() };
    let Some(d) = from.pcs().iter().position(|&p| p == c.root) else { return rn.to_string() };
    let Some(ext) = (0..=2u8).find(|e| progression::diatonic(from, d as i32, *e).quality == c.quality) else {
        return rn.to_string();
    };
    chord::roman_of(&progression::diatonic(to, d as i32, ext), to)
}

/// a progression chord in a new key: numerals are relative to the key already, a slash bass (a note name) moves with
/// it, and a change of mode moves diatonic chords to the new mode
fn rekey(rn: &str, from: &Key, to: &Key, shift: i32) -> String {
    let remoded = |r: &str| if from.mode != to.mode { remode(r, from, to) } else { r.to_string() };
    match crate::theory::chord::slash_bass(rn) {
        Some((head, bass)) => {
            let bass = crate::theory::scale::pc_name((bass as i32 + shift).rem_euclid(12) as u8, to.flats());
            format!("{}/{bass}", remoded(head))
        }
        None => remoded(rn),
    }
}

pub(super) fn genre_of(bp: &Blueprint) -> Option<GenreProfile> {
    genres::get(&bp.genre).ok()
}

/// an empty song for scripts that build everything themselves
pub fn blank(genre: &str) -> Result<Blueprint, String> {
    let g = genres::get(genre).map_err(|e| e.to_string())?;
    let mut bp =
        compose::generate(&g, 1, &Options { short: true, samples: 0.0, loops: false, ..Default::default() }, None);
    bp.drums.clear();
    bp.parts.clear();
    bp.loops.clear();
    // nothing borrowed from the generator: the tonic chord until the script writes a progression
    let tonic = if bp.key.mode.is_minor() { "i" } else { "I" };
    bp.harmony.progressions = BTreeMap::from([("A".to_string(), vec![tonic.to_string()])]);
    bp.harmony.bars_per_chord = 1;
    bp.title = "Untitled".into();
    bp.form = vec![SectionSpec {
        name: "loop".into(),
        kind: SectionKind::Groove,
        bars: 8,
        energy: 0.8,
        prog: "A".into(),
        layers: Vec::new(),
        variant: "main".into(),
        filter_in: false,
        build: false,
        gap: false,
        impact: false,
        fills: false,
    }];
    bp.lineage = vec![format!("blank {genre} song")];
    Ok(bp)
}

/// option maps may only hold the options a function knows: a typo should not be silently ignored
pub(super) fn check_keys(m: &Map, what: &str, known: &[&str]) -> R<()> {
    match m.keys().find(|k| !known.contains(&k.as_str())) {
        Some(k) => fail(format!("{what} has no option '{k}' (options: {})", known.join(" "))),
        None => Ok(()),
    }
}

pub(super) fn map_f32(m: &Map, k: &str) -> Option<f32> {
    m.get(k).and_then(num).map(|v| v as f32)
}

pub(super) fn map_str(m: &Map, k: &str) -> Option<String> {
    m.get(k).and_then(text)
}

pub(super) fn register_song(engine: &mut Engine, ctx: Rc<RefCell<Ctx>>) {
    register_start(engine, &ctx);
    register_settings(engine, &ctx);
    register_drums(engine, &ctx);
    super::mix::register_mix(engine, &ctx);
    register_tracks(engine, &ctx);
    super::form::register_form(engine, &ctx);
}

/// every edit works on the current song (a blank house song when there is none yet)
pub(super) fn with_bp<T>(ctx: &Rc<RefCell<Ctx>>, f: impl FnOnce(&mut Blueprint, &mut Vec<String>) -> R<T>) -> R<T> {
    let mut cx = ctx.borrow_mut();
    if cx.bp.is_none() {
        cx.bp = Some(blank("house")?);
    }
    let Ctx { bp, log, .. } = &mut *cx;
    f(bp.as_mut().unwrap(), log)
}

/// `song(...)` and `blank(...)`: start a song
fn register_start(engine: &mut Engine, ctx: &Rc<RefCell<Ctx>>) {
    let c = ctx.clone();
    let start = move |genre: &str, seed: u64, extra: Option<Map>| -> R<()> {
        let g = genres::get(genre).map_err(|e| e.to_string())?;
        let mut cx = c.borrow_mut();
        let lib = cx.lib.clone();
        let mut opts = Options::default();
        if let Some(m) = &extra {
            check_keys(m, "song()", &["seed", "bpm", "key", "loops", "short", "samples"])?;
            opts.bpm = m.get("bpm").and_then(num);
            if let Some(b) = opts.bpm
                && !(40.0..=240.0).contains(&b)
            {
                return fail("tempo between 40 and 240 BPM");
            }
            if let Some(k) = map_str(m, "key") {
                opts.key = Some(Key::parse(&k).ok_or_else(|| {
                    format!("cannot read key '{k}' (try \"A minor\", \"F#m\", \"Eb\", \"D dorian\")")
                })?);
            }
            opts.family = m.get("loops").and_then(|d| d.as_bool().ok()).unwrap_or(false);
            opts.short = m.get("short").and_then(|d| d.as_bool().ok()).unwrap_or(false);
            if let Some(s) = map_f32(m, "samples") {
                opts.samples = s as f64;
            }
        }
        let bp = compose::generate(&g, seed, &opts, lib.as_deref().map(|l| l as &dyn compose::Library));
        cx.log.push(format!("song: {} — {} at {:.0} BPM in {}", bp.title, bp.genre, bp.bpm, bp.key.name()));
        cx.bp = Some(bp);
        Ok(())
    };
    let s1 = Rc::new(start);
    let s = s1.clone();
    engine.register_fn("song", move |genre: &str| {
        s(
            genre,
            1 + (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64)
                % 1_000_000),
            None,
        )
    });
    let s = s1.clone();
    engine.register_fn("song", move |genre: &str, seed: INT| s(genre, seed.max(0) as u64, None));
    let s = s1.clone();
    engine.register_fn("song", move |genre: &str, m: Map| {
        let seed = m.get("seed").and_then(num).map_or(1, |v| v as u64);
        s(genre, seed, Some(m))
    });
    let c = ctx.clone();
    engine.register_fn("blank", move |genre: &str| -> R<()> {
        c.borrow_mut().bp = Some(blank(genre)?);
        Ok(())
    });
}

/// tempo, key, swing, humanize, title, progressions
fn register_settings(engine: &mut Engine, ctx: &Rc<RefCell<Ctx>>) {
    for name in ["tempo", "bpm"] {
        let c = ctx.clone();
        engine.register_fn(name, move |x: Dynamic| -> R<()> {
            let v = constant(&x, "tempo")?;
            if !(40.0..=240.0).contains(&v) {
                return fail("tempo between 40 and 240 BPM");
            }
            with_bp(&c, |bp, _| {
                bp.bpm = v;
                Ok(())
            })
        });
    }
    let c = ctx.clone();
    engine.register_fn("key", move |k: &str| -> R<()> {
        let key = Key::parse(k)
            .ok_or_else(|| format!("cannot read key '{k}' (try \"A minor\", \"F#m\", \"Eb\", \"D dorian\")"))?;
        with_bp(&c, |bp, _| {
            let shift = key.transpose_from(bp.key.tonic);
            for prog in bp.harmony.progressions.values_mut() {
                for rn in prog.iter_mut() {
                    *rn = rekey(rn, &bp.key, &key, shift);
                }
            }
            for l in bp.loops.iter_mut() {
                if let Sound::Loop { tonal: true, .. } = l.sound {
                    let t = l.transpose + shift;
                    l.transpose = if t > 6 {
                        t - 12
                    } else if t < -6 {
                        t + 12
                    } else {
                        t
                    };
                }
            }
            bp.key = key;
            Ok(())
        })
    });
    let c = ctx.clone();
    engine.register_fn("swing", move |x: Dynamic| -> R<()> {
        let v = constant(&x, "swing")? as f32;
        with_bp(&c, |bp, _| {
            bp.swing = v.clamp(0.0, 0.8);
            Ok(())
        })
    });
    let c = ctx.clone();
    engine.register_fn("humanize", move |x: Dynamic| -> R<()> {
        let v = constant(&x, "humanize")? as f32;
        with_bp(&c, |bp, _| {
            bp.humanize = v.clamp(0.0, 1.0);
            Ok(())
        })
    });
    let c = ctx.clone();
    engine.register_fn("title", move |t: &str| -> R<()> {
        with_bp(&c, |bp, _| {
            bp.title = t.to_string();
            Ok(())
        })
    });
    let c = ctx.clone();
    engine.register_fn("prog", move |name: &str, chords: &str| -> R<()> {
        let rn: Vec<String> = chords.split_whitespace().map(String::from).collect();
        with_bp(&c, |bp, _| {
            for r in &rn {
                if crate::theory::chord::roman(r, &bp.key).is_none() && crate::theory::chord::Chord::parse(r).is_none()
                {
                    return fail(format!(
                        "cannot read chord '{r}' (Roman numerals like i7 VImaj7 bVII, or names like Am7)"
                    ));
                }
            }
            let rn: Vec<String> = rn
                .iter()
                .map(|r| match crate::theory::chord::Chord::parse(r) {
                    Some(ch) if crate::theory::chord::roman(r, &bp.key).is_none() => {
                        crate::theory::chord::roman_of(&ch, &bp.key)
                    }
                    _ => r.clone(),
                })
                .collect();
            bp.harmony.progressions.insert(name.to_string(), rn);
            Ok(())
        })
    });
    let c = ctx.clone();
    engine.register_fn("bars_per_chord", move |bars_per_chord: INT| -> R<()> {
        with_bp(&c, |bp, _| {
            bp.harmony.bars_per_chord = bars_per_chord.clamp(1, 8) as u32;
            Ok(())
        })
    });
}

/// `drum(lane, [variant,] steps[, steps per bar])`
fn register_drums(engine: &mut Engine, ctx: &Rc<RefCell<Ctx>>) {
    let drum = {
        let c = ctx.clone();
        move |lane: &str, variant: &str, steps: &str, per_bar: u32| -> R<()> {
            with_bp(&c, |bp, _| {
                let (hits, bars) = steps_to_hits(steps, per_bar);
                if let Some(l) = bp.drums.iter_mut().find(|d| d.name == lane) {
                    // every variant of a lane has the same length: repeat the shorter ones up to the longest
                    let len = if l.pattern_bars % bars == 0 {
                        l.pattern_bars
                    } else if bars % l.pattern_bars == 0 {
                        bars
                    } else {
                        l.pattern_bars * bars
                    };
                    let hits = tile_hits(&hits, bars, len);
                    if len != l.pattern_bars {
                        for v in l.patterns.values_mut() {
                            *v = tile_hits(v, l.pattern_bars, len);
                        }
                        l.pattern_bars = len;
                    }
                    // the same steps as now: keep the exact hits (step strings round velocities)
                    if l.patterns.get(variant).is_some_and(|old| hits_to_steps(old, len) == hits_to_steps(&hits, len)) {
                        return Ok(());
                    }
                    l.patterns.insert(variant.to_string(), hits);
                    return Ok(());
                }
                let Some((voice, _)) = drum_name(lane).or_else(|| match lane {
                    "hats" => drum_name("hh"),
                    _ => None,
                }) else {
                    let lanes: Vec<&str> = bp.drums.iter().map(|d| d.name.as_str()).collect();
                    return fail(format!(
                        "no drum lane '{lane}', and drum() makes lanes only for drum names ({}); the song's lanes: {}. \
                         For other sounds, track(\"{lane}\", s(\"...\"))",
                        super::drum_names().join(" "),
                        if lanes.is_empty() { "none yet".to_string() } else { lanes.join(" ") }
                    ));
                };
                let mut patterns = BTreeMap::new();
                patterns.insert(variant.to_string(), hits);
                bp.drums.push(DrumLane {
                    name: lane.to_string(),
                    sound: Sound::Drum { voice, params: DrumParams::default_for(voice) },
                    patterns,
                    pattern_bars: bars,
                    pitch: 0,
                    choke: None,
                    mix: TrackMix { bus: "drums".into(), ..Default::default() },
                });
                for s in bp.form.iter_mut().filter(|s| s.energy >= 0.6) {
                    s.layers.push(lane.to_string());
                }
                Ok(())
            })
        }
    };
    let d = Rc::new(drum);
    let d1 = d.clone();
    engine.register_fn("drum", move |lane: &str, steps: &str| d1(lane, "main", steps, 16));
    let d1 = d.clone();
    engine.register_fn("drum", move |lane: &str, variant: &str, steps: &str| d1(lane, variant, steps, 16));
    let d1 = d.clone();
    engine.register_fn("drum", move |lane: &str, variant: &str, steps: &str, per_bar: INT| {
        d1(lane, variant, steps, per_bar.clamp(1, 96) as u32)
    });
}

/// `track(name, pattern)`: patterns as tracks
fn register_tracks(engine: &mut Engine, ctx: &Rc<RefCell<Ctx>>) {
    let c = ctx.clone();
    engine.register_fn("track", move |name: &str, p: Pattern| -> R<()> {
        let (lib, roots) = {
            let cx = c.borrow();
            (cx.lib.clone(), cx.roots.clone())
        };
        with_bp(&c, |bp, log| {
            let g = genre_of(bp);
            let nctx = notes::Context {
                key: bp.key,
                progressions: &bp.harmony.progressions,
                bars_per_chord: bp.harmony.bars_per_chord,
                prog: "A",
            };
            let placed = notes::place(&p, 0, 64, 0.0, &nctx).map_err(|e| format!("track '{name}': {e}"))?;
            if placed.len() > 64 * MAX_NOTES_PER_BAR {
                return fail(format!(
                    "track '{name}' plays {} notes per bar; the limit is {MAX_NOTES_PER_BAR}",
                    placed.len() / 64
                ));
            }
            let mut labels: Vec<String> = placed.iter().map(|x| x.sound.clone()).collect();
            labels.sort();
            labels.dedup();
            if labels.is_empty() {
                labels.extend(p.opts.sound.clone());
            }
            let mut sounds = BTreeMap::new();
            for l in &labels {
                let s = resolve_sound(l, p.opts.bank.as_deref(), lib.as_deref(), g.as_ref(), bp.seed, roots.as_deref())
                    .map_err(|e| format!("track '{name}': {e}"))?;
                sounds.insert(l.clone(), s);
            }
            let mut pitches: Vec<i32> = placed.iter().map(|x| x.note.p).collect();
            pitches.sort();
            let median = pitches.get(pitches.len() / 2).copied().unwrap_or(60) as f64;
            let role =
                p.opts.role.clone().unwrap_or_else(|| {
                    if labels.len() == 1 { role_of(&labels[0], p.kind, median) } else { String::new() }
                });
            let mix = pattern_mix(&p.opts, &labels, &role);
            let existed = bp.layer_names().iter().any(|l| l == name);
            bp.drums.retain(|d| d.name != name);
            bp.parts.retain(|x| x.name != name);
            bp.loops.retain(|l| l.name != name);
            bp.patterns.retain(|x| x.name != name);
            bp.patterns.push(PatternTrack { name: name.to_string(), code: p.repr.clone(), sounds, role, mix });
            if !existed {
                let any = bp.form.iter().any(|s| !s.layers.is_empty());
                for s in bp.form.iter_mut() {
                    if !any || s.energy >= 0.6 {
                        s.layers.push(name.to_string());
                    }
                }
            }
            log.push(format!("track {name}: {}", labels.join(", ")));
            Ok(())
        })
    });
}

/// the mix a pattern's own settings give its track
pub(super) fn pattern_mix(o: &crate::pattern::TrackOpts, labels: &[String], role: &str) -> TrackMix {
    let drum_like = !labels.is_empty() && labels.iter().all(|l| super::sounds::drum_role(l).is_some());
    let mut fx = Vec::new();
    if let Some(f) = o.hpf {
        fx.push(Effect::Filter { mode: FilterMode::Hp, cutoff: Param::Const(f), res: 0.1, drive: 1.0, mix: 1.0 });
    }
    if let Some(f) = o.lpf {
        fx.push(Effect::Filter { mode: FilterMode::Lp, cutoff: Param::Const(f), res: 0.15, drive: 1.0, mix: 1.0 });
    }
    if let Some(b) = o.crush {
        fx.push(Effect::Bitcrush { bits: b, downsample: 1.0, mix: 1.0 });
    }
    if let Some(d) = o.drive {
        fx.push(Effect::Saturate { kind: SatKind::Tanh, drive_db: d, mix: 1.0 });
    }
    TrackMix {
        level_db: o.level_db.unwrap_or(0.0),
        pan: o.pan.unwrap_or(0.0),
        fx,
        reverb: o.reverb.unwrap_or(0.0),
        delay: o.delay.unwrap_or(0.0),
        duck: o.duck.unwrap_or(if drum_like || role == "kick" { 0.0 } else { 0.3 }),
        bus: if drum_like {
            "drums"
        } else if role == "bass" {
            "bass"
        } else {
            "music"
        }
        .into(),
        mute: false,
        solo: false,
    }
}
