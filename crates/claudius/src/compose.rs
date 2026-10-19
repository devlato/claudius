//! The composer. `generate` makes the creative choices (a blueprint from a genre profile and a seed); `arrange`
//! expands a blueprint into a project the way a producer lays out a session: progressions on a chord track, one
//! loop of every part per progression tiled across its sections, drum variants per section, fills at phrase ends,
//! crashes on phrase starts, builds with snare rolls, risers and a highpass sweep, a gap before the drop, impacts,
//! and filter automation in intros and outros.

use std::collections::BTreeMap;

use crate::audio::midi_to_hz;
use crate::dsp::env::Param;
use crate::dsp::filter::FilterMode;
use crate::fx::Effect;
use crate::genres::{GenreProfile, LoopSpec, PartSpec};
use crate::model::*;
use crate::pattern::notes;
use crate::rng::{Rng, hash_str};
use crate::synth::drums::{DrumParams, DrumVoice};
use crate::synth::presets;
use crate::theory::rhythm::{self, Hit};
use crate::theory::scale::{Key, Mode};
use crate::theory::{ChordSpan, Note, bass, melody, progression};

mod arrange;

pub use arrange::{arrange, cycle_spans, part_cycle};

/// a loop family picked for a song: loops produced to play together (one tempo, one key), with the chords of the
/// harmony loop read bar by bar
#[derive(Clone, Debug)]
pub struct FamilyPick {
    pub name: String,
    pub bpm: f64,
    pub key: Key,
    /// Roman numerals of the harmony loop, one per bar
    pub progression: Vec<String>,
    pub chords: Vec<Sound>,
    pub bass: Option<Sound>,
    pub beat: Option<Sound>,
}

/// where recorded sounds come from (the sample library); the composer works without one, synthesizing everything
pub trait Library: Sync {
    fn one_shot(&self, lane: &str, subtypes: &[String], genre: &GenreProfile, rng: &mut Rng) -> Option<Sound>;
    /// a loop for `spec` near `bpm`, plus the transpose that puts it in `key`
    fn find_loop(
        &self,
        spec: &LoopSpec,
        genre: &GenreProfile,
        bpm: f64,
        key: &Key,
        rng: &mut Rng,
    ) -> Option<(Sound, i32)>;
    /// a loop family for the genre (GarageBand-style composition)
    fn family(&self, _genre: &GenreProfile, _rng: &mut Rng) -> Option<FamilyPick> {
        None
    }
}

#[derive(Clone, Debug)]
pub struct Options {
    pub bpm: Option<f64>,
    pub key: Option<Key>,
    pub form: Option<usize>,
    /// halve every section (quick sketches, evolution previews)
    pub short: bool,
    /// chance that a drum lane uses a recorded one-shot when the library has one
    pub samples: f64,
    pub loops: bool,
    /// build the song around a loop family (its chords, bass and beat loops), GarageBand style
    pub family: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { bpm: None, key: None, form: None, short: false, samples: 0.75, loops: true, family: false }
    }
}

const ADJ: &[&str] = &[
    "Midnight", "Neon", "Velvet", "Static", "Golden", "Hollow", "Electric", "Paper", "Glass", "Silent", "Crimson",
    "Lunar", "Distant", "Burning", "Faded", "Liquid",
];
const NOUN: &[&str] = &[
    "Signal",
    "Avenue",
    "Bloom",
    "Drift",
    "Echo",
    "Tide",
    "Circuit",
    "Haze",
    "Pulse",
    "Mirage",
    "Orbit",
    "Harbor",
    "Engine",
    "Garden",
    "Satellite",
    "Rain",
];

fn title(rng: &mut Rng) -> String {
    format!("{} {}", rng.pick(ADJ), rng.pick(NOUN))
}

fn pick_weighted<'a, T>(items: &'a [(T, f64)], rng: &mut Rng) -> &'a T {
    rng.pick_weighted(items)
}

fn drum_params(voice: DrumVoice, ranges: &BTreeMap<String, (f32, f32)>, rng: &mut Rng) -> DrumParams {
    let mut p = DrumParams::default_for(voice);
    let space = DrumParams::space(voice);
    let names = ["tune", "decay", "tone", "punch", "snap", "drive", "pitch_env", "pitch_time", "room"];
    for (i, name) in names.iter().enumerate() {
        let v = if let Some((lo, hi)) = ranges.get(*name) {
            rng.range(*lo as f64, *hi as f64) as f32
        } else {
            let crate::synth::drums::Range(lo, hi, log) = space[i];
            let d = p.get(i);
            // stay near the voice default: a third of the way towards a random point in its range
            let target = if log && lo > 0.0 {
                rng.log_range(lo as f64, hi as f64) as f32
            } else {
                rng.range(lo as f64, hi as f64) as f32
            };
            d + (target - d) * 0.33
        };
        p.set(i, v);
    }
    p
}

fn pick_kit(kits: &[(String, f64)], rng: &mut Rng) -> Option<String> {
    let mut x = rng.f64();
    for (name, w) in kits {
        if x < *w {
            return claudius_packs::installed().iter().any(|p| p.name == *name).then(|| name.clone());
        }
        x -= w;
    }
    None
}

/// a drum lane played on a kit from the sample packs
pub fn kit_sound(lane: &str, kit: &str) -> Option<Sound> {
    let name = match lane {
        "kick" => "bd",
        "snare" => "sd",
        "clap" => "cp",
        "hat" => "hh",
        "ohat" => "oh",
        "ride" => "rd",
        "crash" => "cr",
        "tom" => "mt",
        "perc" => "rim",
        "shaker" => "sh",
        _ => return None,
    };
    claudius_packs::resolve(name, Some(kit)).map(|r| Sound::Instrument { name: r.to_string(), label: name.to_string() })
}

/// which recordings a drum lane's one-shots may be cut from (Apple Loops subtypes, or "name:" + a word in the
/// loop's name)
pub fn default_hints(lane: &str) -> Vec<String> {
    let h: &[&str] = match lane {
        "kick" => &["Kick"],
        "snare" => &["Snare"],
        "clap" => &["name:Clap"],
        "hat" | "ohat" => &["Hi-hat"],
        "shaker" => &["Shaker", "Tambourine"],
        "perc" => &["Clave", "Conga", "Bongo", "Cowbell"],
        "ride" | "crash" => &["Cymbal"],
        "tom" => &["Tom"],
        _ => &[],
    };
    h.iter().map(|s| s.to_string()).collect()
}

fn tune_to_key(tune: f32, lo: f32, hi: f32, key: &Key) -> f32 {
    // the kick's fundamental on the tonic (or the fifth) nearest the sampled tuning
    let mut best = (f32::MAX, tune);
    for pc in [key.tonic as i32, (key.tonic as i32 + 7) % 12] {
        for oct in 0..4 {
            let f = midi_to_hz((pc + 12 * oct + 12) as f64) as f32;
            if f >= lo * 0.9 && f <= hi * 1.1 && (f - tune).abs() < best.0 {
                best = ((f - tune).abs(), f);
            }
        }
    }
    best.1
}

/// the sound for a genre's preset entry: alternatives separated by `|`, the first that exists wins (a synth preset,
/// or an instrument from an installed sample pack: "piano|keys" plays the sampled piano when its pack is installed,
/// else the keys patch). The random draws are the same either way, so the rest of the song does not change.
pub fn part_sound(entry: &str, rng: &mut Rng) -> Sound {
    let synth = entry.split('|').rev().find(|a| presets::get(a).is_some()).unwrap_or("pluck");
    let fallback = vary_patch(synth, rng);
    for alt in entry.split('|') {
        if presets::get(alt).is_some() {
            return if alt == synth { fallback } else { vary_patch(alt, &mut rng.fork(alt)) };
        }
        if let Some(r) = claudius_packs::resolve(alt, None) {
            return Sound::Instrument { name: r.to_string(), label: alt.to_string() };
        }
    }
    fallback
}

fn vary_patch(name: &str, rng: &mut Rng) -> Sound {
    let mut patch = presets::get(name).unwrap_or_else(|| presets::get("pluck").unwrap());
    if let Some(f) = patch.filter.as_mut() {
        f.cutoff *= rng.range(0.8, 1.25) as f32;
    }
    for o in patch.osc.iter_mut() {
        o.detune *= rng.range(0.85, 1.2) as f32;
    }
    Sound::Synth { preset: name.to_string(), patch }
}

/// the creative step: a blueprint from a genre profile and a seed
pub fn generate(g: &GenreProfile, seed: u64, opts: &Options, lib: Option<&dyn Library>) -> Blueprint {
    let family = if opts.family { lib.and_then(|l| l.family(g, &mut Rng::new(seed ^ 0xfa))) } else { None };
    let Some(f) = family else { return generate_inner(g, seed, opts, lib) };
    let o = Options { bpm: Some(f.bpm), key: Some(f.key), ..opts.clone() };
    let mut bp = generate_inner(g, seed, &o, lib);
    let prog = f.progression.clone();
    bp.harmony.progressions.insert("A".into(), prog.clone());
    bp.harmony.progressions.insert("B".into(), prog.clone());
    bp.harmony.bars_per_chord = 1;
    let add = |bp: &mut Blueprint, name: &str, role: &str, sound: Sound, mix: TrackMix| {
        bp.parts.retain(|p| p.name != name);
        bp.loops.retain(|l| l.name != name);
        bp.loops.push(LoopPart {
            name: name.into(),
            role: role.into(),
            sound,
            transpose: 0,
            chop: Vec::new(),
            slice_beats: 0.5,
            mix,
        });
    };
    let chord_mix =
        bp.parts.iter().find(|p| p.name == "chords" || p.name == "keys").map(|p| p.mix.clone()).unwrap_or_default();
    let harmony_name = if g.parts.iter().any(|p| p.name == "chords") {
        "chords"
    } else if g.parts.iter().any(|p| p.name == "keys") {
        "keys"
    } else {
        "chords"
    };
    if let Some(c) = f.chords.first() {
        add(&mut bp, harmony_name, "chords", c.clone(), TrackMix { fx: Vec::new(), ..chord_mix });
    }
    if let Some(b) = &f.bass {
        let bass_mix = bp.parts.iter().find(|p| p.name == "bass").map(|p| p.mix.clone()).unwrap_or_default();
        add(&mut bp, "bass", "bass", b.clone(), bass_mix);
    }
    if let Some(beat) = &f.beat {
        add(
            &mut bp,
            "beat",
            "drums",
            beat.clone(),
            TrackMix { bus: "drums".into(), level_db: -4.0, ..Default::default() },
        );
        for s in bp.form.iter_mut() {
            if s.layers.iter().any(|l| l == "hat" || l == "kick") && s.energy >= 0.6 {
                s.layers.push("beat".into());
            }
        }
    }
    for s in bp.form.iter_mut() {
        if !s.layers.iter().any(|l| l == harmony_name)
            && s.energy >= 0.5
            && matches!(s.kind, SectionKind::Drop | SectionKind::Hook | SectionKind::Groove | SectionKind::Verse)
        {
            s.layers.push(harmony_name.into());
        }
    }
    bp.lineage = vec![format!(
        "generated: {} seed {seed} around the Apple Loops family '{}' ({:.0} BPM, {}, chords {})",
        g.name,
        f.name,
        f.bpm,
        f.key.name(),
        prog.join(" ")
    )];
    bp
}

fn generate_inner(g: &GenreProfile, seed: u64, opts: &Options, lib: Option<&dyn Library>) -> Blueprint {
    let mut rng = Rng::new(seed);
    // drawn even when given, so that a tempo or a key of your own changes only that, not the rest of the song
    let drawn_bpm = rng.range(g.bpm.0, g.bpm.1).round();
    let bpm = opts.bpm.unwrap_or(drawn_bpm);
    let swing = rng.range(g.swing.0 as f64, g.swing.1 as f64) as f32;

    // harmony: a library progression in its style's mode, or a fresh Markov one in one of the genre's modes
    let mut hr = rng.fork("harmony");
    let (mode, prog_a, style_progs): (Mode, Vec<String>, Vec<String>) = if hr.chance(g.markov) || g.harmony.is_empty() {
        let m = *pick_weighted(&g.modes, &mut hr);
        let k = Key::new(0, m);
        let len = if hr.chance(0.8) { 4 } else { 2 };
        (m, progression::markov(&k, len, g.ext, &mut hr), Vec::new())
    } else {
        let names: Vec<(String, f64)> = g.harmony.clone();
        let sname = pick_weighted(&names, &mut hr).clone();
        let st = progression::style(&sname).unwrap_or(&progression::STYLES[0]);
        let p = progression::parse(hr.pick(st.progs));
        (st.mode, p, st.progs.iter().map(|s| s.to_string()).collect())
    };
    let drawn_tonic = hr.below(12) as u8;
    let tonic = opts.key.map_or(drawn_tonic, |k| k.tonic);
    // the progressions are written in the style's mode; a key of your own in another mode gets them as key() would
    let style_key = Key::new(tonic, mode);
    let key = opts.key.unwrap_or(style_key);
    let prog_b = {
        let others: Vec<&String> = style_progs.iter().filter(|p| progression::parse(p) != prog_a).collect();
        if !others.is_empty() && hr.chance(0.5) {
            progression::parse(hr.pick(&others))
        } else {
            let (b, _) = progression::substitute(&prog_a, &style_key, &mut hr);
            if b == prog_a { progression::substitute(&b, &style_key, &mut hr).0 } else { b }
        }
    };
    let in_key = |p: Vec<String>| -> Vec<String> {
        if key.mode == mode { p } else { p.iter().map(|rn| crate::script::remode(rn, &style_key, &key)).collect() }
    };
    let (prog_a, prog_b) = (in_key(prog_a), in_key(prog_b));
    let mut bpc = *pick_weighted(&g.bars_per_chord, &mut hr);
    if prog_a.len() == 1 {
        bpc = 4;
    }
    let mut progressions = BTreeMap::new();
    progressions.insert("A".to_string(), prog_a);
    progressions.insert("B".to_string(), prog_b);

    // form
    let mut fr = rng.fork("form");
    let fi = opts.form.unwrap_or_else(|| fr.below(g.forms.len().max(1))).min(g.forms.len().saturating_sub(1));
    let mut form = g.forms[fi].clone();
    if opts.short {
        for s in form.iter_mut() {
            s.bars = (s.bars / 2).max(4);
        }
    }

    // drums (on a drum machine from the sample packs, sometimes)
    let mut dr = rng.fork("drums");
    let kit = pick_kit(&g.kits, &mut rng.fork("kit"));
    let mut drums = Vec::new();
    for spec in &g.drums {
        let hints: Vec<String> = if spec.samples.is_empty() { default_hints(&spec.name) } else { spec.samples.clone() };
        // the same draws with or without a library, so a seed makes the same patterns on every machine
        let mut sr = dr.fork(&spec.name);
        let sampled = sr.chance(opts.samples);
        let mut p = drum_params(spec.voice, &spec.params, &mut dr);
        let sound = lib
            .filter(|_| !hints.is_empty() && sampled)
            .and_then(|l| l.one_shot(&spec.name, &hints, g, &mut sr))
            .unwrap_or_else(|| {
                if spec.tune_to_key && spec.voice == DrumVoice::Kick {
                    let (lo, hi) = spec.params.get("tune").copied().unwrap_or((40.0, 65.0));
                    p.tune = tune_to_key(p.tune, lo, hi, &key);
                }
                Sound::Drum { voice: spec.voice, params: p }
            });
        let pattern_bars = 4;
        let mut patterns = BTreeMap::new();
        if let Some((k, n)) = spec.euclid {
            let rot = dr.below(n.max(1));
            patterns.insert("main".to_string(), rhythm::euclid_hits(k, n, rot, pattern_bars, 0.75));
            patterns.insert("sparse".to_string(), rhythm::euclid_hits((k / 2).max(1), n, rot, pattern_bars, 0.65));
        } else if let Some(t) = &spec.template {
            patterns.insert("main".to_string(), rhythm::generate(t, pattern_bars, &mut dr));
        }
        for (name, t) in &spec.variants {
            patterns.insert(name.clone(), rhythm::generate(t, pattern_bars, &mut dr));
        }
        let sound = kit.as_deref().and_then(|k| kit_sound(&spec.name, k)).unwrap_or(sound);
        drums.push(DrumLane {
            name: spec.name.clone(),
            sound,
            patterns,
            pattern_bars,
            pitch: 0,
            choke: spec.choke.clone(),
            mix: spec.mix.clone(),
        });
    }

    // parts
    let mut pr = rng.fork("parts");
    let mut parts = Vec::new();
    for spec in &g.parts {
        if spec.optional > 0.0 && pr.chance(spec.optional) {
            continue;
        }
        parts.push(new_part(spec, &mut pr));
    }

    // loops from the library
    let mut lr = rng.fork("loops");
    let mut loops = Vec::new();
    if opts.loops
        && let Some(l) = lib
    {
        for spec in &g.loops {
            if spec.optional > 0.0 && lr.chance(spec.optional) {
                continue;
            }
            if let Some((sound, transpose)) = l.find_loop(spec, g, bpm, &key, &mut lr) {
                loops.push(LoopPart {
                    name: spec.name.clone(),
                    role: spec.role.clone(),
                    sound,
                    transpose,
                    chop: Vec::new(),
                    slice_beats: 0.5,
                    mix: spec.mix.clone(),
                });
            }
        }
    }

    let mut tr = rng.fork("transitions");
    let sampled = tr.chance(opts.samples);
    let params = drum_params(DrumVoice::Crash, &BTreeMap::new(), &mut tr);
    let crash = lib
        .filter(|_| sampled)
        .and_then(|l| l.one_shot("crash", &default_hints("crash"), g, &mut tr))
        .unwrap_or(Sound::Drum { voice: DrumVoice::Crash, params });
    let t = &g.transitions;
    let transitions = Transitions {
        crash,
        riser: if t.riser { Some(Sound::Fx { fx: "riser".into() }) } else { None },
        impact: if t.impact { Some(Sound::Fx { fx: "impact".into() }) } else { None },
        use_impact: t.impact,
        use_downlifter: t.downlifter,
        use_reverse_crash: t.reverse_crash,
        riser_bars: t.riser_bars,
        mix: t.mix.clone(),
    };
    let mix = MixPlan {
        reverb: g.mix.reverb.clone(),
        delay: g.mix.delay.clone(),
        buses: g.mix.buses.clone(),
        duck_release: g.mix.duck_release,
        levels: g.mix.levels.clone(),
        master: Master { fx: g.mix.master.clone(), lufs: g.mix.lufs, ceiling_db: -1.0 },
    };
    Blueprint {
        version: crate::model::BLUEPRINT_FORMAT,
        title: title(&mut rng.fork("title")),
        genre: g.name.clone(),
        seed,
        bpm,
        key,
        swing,
        humanize: g.humanize,
        harmony: Harmony { progressions, bars_per_chord: bpc },
        form,
        drums,
        parts,
        loops,
        patterns: Vec::new(),
        transitions,
        mix,
        lineage: vec![format!("generated: {} seed {seed}", g.name)],
    }
}

/// a part as the genre describes it: its preset, style, density and rate drawn from the genre's choices
pub fn new_part(spec: &PartSpec, rng: &mut Rng) -> Part {
    let preset = rng.pick(&spec.presets).clone();
    let sound = part_sound(&preset, rng);
    let mut mix = spec.mix.clone();
    // harmony parts from single-voice patches would sit dead centre: a light chorus spreads them
    let single_voice = matches!(&sound, Sound::Synth { patch, .. } if patch.osc.iter().all(|o| o.voices <= 1));
    if single_voice
        && matches!(spec.role, Role::Chords | Role::Keys | Role::Pad)
        && !mix.fx.iter().any(|e| e.name() == "chorus")
    {
        mix.fx.push(Effect::Chorus { rate_hz: 0.45, depth_ms: 2.5, delay_ms: 11.0, mix: 0.4 });
    }
    Part {
        name: spec.name.clone(),
        role: spec.role,
        sound,
        style: rng.pick(&spec.styles).clone(),
        register: spec.register,
        density: rng.range(spec.density.0 as f64, spec.density.1 as f64) as f32,
        voices: if spec.voices > 0 { spec.voices } else { 4 },
        voicing: spec.voicing,
        rate: if spec.rates.is_empty() { 0.25 } else { *rng.pick(&spec.rates) },
        seed: rng.next_u64(),
        notes: BTreeMap::new(),
        mix,
    }
}

/// the styles a part with this role can play
pub fn styles_for(role: Role) -> &'static [&'static str] {
    match role {
        Role::Bass => bass::STYLES,
        Role::Chords | Role::Pad | Role::Keys => melody::CHORD_STYLES,
        Role::Arp => melody::ARP_STYLES,
        Role::Lead => melody::LEAD_STYLES,
    }
}

/// flip a recording into the song: warped to the tempo, transposed to the key, re-sliced in half-beat pieces (or
/// played straight), placed in the high-energy sections
pub fn add_sample(bp: &mut Blueprint, path: &str, bpm: f64, beats: f64, key: Option<(u8, bool)>, rng: &mut Rng) {
    let transpose = key.map_or(0, |(k, minor)| {
        let target = if minor == bp.key.mode.is_minor() {
            bp.key.tonic
        } else if minor {
            (bp.key.tonic + 9) % 12
        } else {
            (bp.key.tonic + 3) % 12
        };
        Key::new(target, bp.key.mode).transpose_from(k)
    });
    let slices = (beats / 0.5).round().max(1.0) as i32;
    let chop: Vec<i32> = if rng.chance(0.6) && slices >= 4 {
        let motif: Vec<i32> =
            (0..8).map(|i| if i % 4 == 0 { i % slices } else { rng.below(slices as usize) as i32 }).collect();
        motif
    } else {
        Vec::new()
    };
    let name = format!("sample{}", bp.loops.iter().filter(|l| l.role == "sample").count() + 1);
    let label =
        std::path::Path::new(path).file_name().map_or_else(|| path.to_string(), |f| f.to_string_lossy().into_owned());
    bp.loops.push(LoopPart {
        name: name.clone(),
        role: "sample".into(),
        sound: Sound::Loop {
            path: path.to_string(),
            bpm,
            beats,
            key: key.map(|k| k.0),
            tonal: key.is_some(),
            label: format!("sample '{label}'"),
        },
        transpose,
        chop: chop.clone(),
        slice_beats: 0.5,
        mix: TrackMix { duck: 0.4, reverb: 0.15, bus: "music".into(), ..Default::default() },
    });
    for s in bp.form.iter_mut() {
        if matches!(s.kind, SectionKind::Drop | SectionKind::Hook | SectionKind::Groove)
            || (s.kind == SectionKind::Break && rng.chance(0.4))
        {
            s.layers.push(name.clone());
        }
    }
    bp.lineage.push(format!(
        "flipped '{label}' ({bpm:.0} BPM, {beats} beats, transposed {transpose:+}){}",
        if chop.is_empty() { "" } else { ", re-chopped" }
    ));
}
