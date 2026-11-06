//! The composer. `generate` makes the creative choices (a blueprint from a genre profile and a seed); `arrange`
//! expands a blueprint into a project the way a producer lays out a session: progressions on a chord track, one
//! loop of every part per progression tiled across its sections, drum variants per section, fills at phrase ends,
//! crashes on phrase starts, builds with snare rolls, risers and a highpass sweep, a gap before the drop, impacts,
//! and filter automation in intros and outros.

use std::collections::{BTreeMap, BTreeSet};

use crate::audio::midi_to_hz;
use crate::dsp::env::Param;
use crate::dsp::filter::FilterMode;
use crate::fx::Effect;
use crate::genres::{GenreProfile, LoopSpec, PartSpec};
use crate::model::*;
use crate::palette;
use crate::pattern::notes;
use crate::rng::{Rng, hash_str};
use crate::synth::drums::{DrumParams, DrumVoice};
use crate::synth::presets;
use crate::theory::rhythm::{self, Hit};
use crate::theory::scale::{Key, Mode};
use crate::theory::{ChordSpan, Note, bass, melody, progression};

mod arrange;
mod layers;

pub use arrange::{arrange, cycle_spans, cycles, part_cycle};
pub use layers::{chance_at, left_out_at, reach};

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
    /// a sample prepared for the genre to play as `role` (`library::prepared`), or gathered for the words `asked`, when
    /// one is at hand
    fn prepared(
        &self,
        _role: crate::library::prepared::Role,
        _genre: &GenreProfile,
        _asked: Option<&str>,
        _rng: &mut Rng,
    ) -> Option<crate::library::prepared::Prepared> {
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
    /// build it on samples from free libraries (`crate::sampling`); none: as the genre does
    pub sampled: Option<bool>,
    /// what its phrase is searched for ("70s soul"); none: what the genre's producers sample
    pub sample_search: Option<String>,
    /// how many layers it plays, from 0 (its genre's core, sparse) through 0.5 (the genre as its profile has it) to
    /// 1 (instruments doubled on instruments, a counter-melody, more percussion, samples on samples); none: the
    /// genre's own (`complexity` in its profile)
    pub complexity: Option<f64>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            bpm: None,
            key: None,
            form: None,
            short: false,
            samples: 0.75,
            loops: true,
            family: false,
            sampled: None,
            sample_search: None,
            complexity: None,
        }
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

/// a lane's own kit some of the time (`mix`, its genre's `kit_mix`): another of the installed kits that has its sound,
/// or None to keep the song's (or its synthesized voice)
fn lane_kit(lane: &str, mix: f64, kits: &[String], rng: &mut Rng) -> Option<Sound> {
    if !rng.chance(mix) {
        return None;
    }
    let have: Vec<Sound> = kits.iter().filter_map(|k| kit_sound(lane, k)).collect();
    have.get(rng.below(have.len() + 1)).cloned()
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
        "perc" | "rim" => "rim",
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
        "rim" => &["Clave", "name:Rim"],
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

/// how far under a drums track's level an Apple Loops family's beat plays, under the song's own drums
pub const FAMILY_BEAT_DB: f32 = -4.0;

/// The level (dB) a song of genre `g` gives a track of its own over its role's as it is made: its drum lane's,
/// its part's, its loop's, its transitions' or its sample's `mix.level_db`, or what an Apple Loops family's beat
/// plays at; 0 for any other track.
pub fn made_level(g: &GenreProfile, track: &str) -> f32 {
    if let Some(l) = g.drums.iter().find(|l| l.name == track) {
        return l.mix.level_db;
    }
    if let Some(p) = g.parts.iter().find(|p| p.name == track) {
        return p.mix.level_db;
    }
    if let Some(l) = g.loops.iter().find(|l| l.name == track) {
        return l.mix.level_db;
    }
    let s = g.sampling.as_ref();
    let sampled = match track {
        crate::sampling::BREAK => s.and_then(|s| s.drum_break.as_ref()),
        crate::sampling::PHRASE | crate::sampling::SECOND => s.and_then(|s| s.phrase.as_ref()),
        crate::sampling::TEXTURE => s.and_then(|s| s.texture.as_ref()),
        crate::sampling::VOCAL => s.and_then(|s| s.vocal.as_ref()),
        "crash" | "riser" | "impact" | "downlifter" | "reverse_crash" => return g.transitions.mix.level_db,
        "beat" => return FAMILY_BEAT_DB,
        _ => None,
    };
    match sampled {
        Some(n) => n.mix.level_db,
        None => layers::made_level(g, track).unwrap_or(0.0),
    }
}

/// The synth presets that fit a genre's parts of `role`: those its own parts name (each entry's preset, or the synth
/// an instrument it names falls back to: `vibraphone|mallet`, the mallet), and the others of their families
/// (`presets::FAMILIES`), so that a genre that plays an electric piano may play the other one, and one that plays no
/// bell plays none. A pad, which holds its chords behind the song, may be any of the synth pads.
pub fn fitting_synths(g: &GenreProfile, role: Role) -> Vec<String> {
    let named = g
        .parts
        .iter()
        .filter(|p| p.role == role)
        .flat_map(|p| p.presets.iter())
        .filter_map(|entry| entry.split('|').rev().find(|a| presets::get(a).is_some()));
    let pads = if role == Role::Pad { presets::drawable("pad") } else { Vec::new() };
    let mut out: Vec<String> = Vec::new();
    for s in named.flat_map(presets::family).chain(pads) {
        if !out.iter().any(|o| o == s) {
            out.push(s.to_string());
        }
    }
    out
}

/// The instruments a genre's parts of `role` name, by alias (`vibraphone|mallet`, the vibraphone): installed, a draw
/// from the palette may play them for their kind, as the part itself would
pub fn named_instruments(g: &GenreProfile, role: Role) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for alias in g.parts.iter().filter(|p| p.role == role).flat_map(|p| p.presets.iter()).flat_map(|e| e.split('|')) {
        if presets::get(alias).is_none() && !out.iter().any(|o| o == alias) {
            out.push(alias.to_string());
        }
    }
    out
}

/// Whether `sound` is one a genre has a place for in a part of `role`, as its draws judge (`palette::draw`): a synth
/// that fits its parts or that its palette draws, an installed instrument its parts name or of a kind its palette
/// would draw. Other sounds (recordings, samples, loops) are not judged.
pub fn has_place(g: &GenreProfile, role: Role, sound: &Sound) -> bool {
    let own = fitting_synths(g, role);
    let mut rp = g.palette.get(role.name()).cloned().unwrap_or_else(|| palette::default_for(role));
    if rp.synths.is_empty() {
        rp.synths = own.clone();
    }
    if rp.instruments.is_empty() {
        rp.instruments = named_instruments(g, role);
    }
    match sound {
        Sound::Synth { preset, .. } => {
            own.contains(preset) || palette::synths_drawn(role, &rp).contains(&preset.as_str())
        }
        Sound::Instrument { name, .. } => palette::instrument_has_place(role, &rp, name),
        _ => true,
    }
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

/// A synth sound varied by `amount` (0 to 1): its filter, envelopes, detune, vibrato, FM, drive and glide each
/// moved up or down (by half again at most), now and then a voice more or less in its unison or a soft octave above
/// it, so that the same preset in two songs sounds like two patches of one family. Instruments stay as they are.
pub fn widen(sound: Sound, amount: f64, rng: &mut Rng) -> Sound {
    let Sound::Synth { preset, mut patch } = sound else { return sound };
    let a = amount.clamp(0.0, 1.0);
    let mut by = |x: f64, spread: f64| x * (rng.range(-1.0, 1.0) * spread.ln() * a).exp();
    if let Some(f) = patch.filter.as_mut() {
        f.cutoff = by(f.cutoff as f64, 1.5).clamp(80.0, 16_000.0) as f32;
        f.env = by(f.env as f64, 1.3) as f32;
    }
    // to a tenth of a millisecond: a time with no more digits than that reads back from JSON as it was
    let ms = |x: f64| (x * 1e4).round() / 1e4;
    for env in [&mut patch.fenv, &mut patch.aenv] {
        env.a = ms(by(env.a, 1.8).max(0.001));
        env.d = ms(by(env.d, 1.5).max(0.005));
        env.r = ms(by(env.r, 1.5).max(0.005));
    }
    for o in patch.osc.iter_mut() {
        o.detune = by(o.detune as f64, 1.4) as f32;
    }
    if let Some(l) = patch.lfo.as_mut() {
        l.rate = by(l.rate as f64, 1.3) as f32;
        l.vibrato = by(l.vibrato as f64, 1.5) as f32;
    }
    if let Some(fm) = patch.fm.as_mut() {
        fm.index = by(fm.index as f64, 1.35) as f32;
        fm.decay = by(fm.decay as f64, 1.4) as f32;
    }
    patch.drive = by(patch.drive as f64, 1.5) as f32;
    patch.glide = by(patch.glide as f64, 1.5) as f32;
    let (more, bass) = (rng.chance(0.25 * a), patch.mono && patch.osc.iter().all(|o| o.octave <= 0));
    if more {
        if let Some(o) = patch.osc.iter_mut().find(|o| o.voices > 1) {
            o.voices = if rng.chance(0.5) { (o.voices + 1).min(9) } else { (o.voices - 1).max(2) };
        } else if bass {
            patch.sub = (patch.sub + 0.2).min(0.8);
        } else {
            patch.osc.push(crate::synth::voice::OscSpec::new(crate::dsp::osc::Wave::Tri).octave(1).level(0.12));
        }
    }
    Sound::Synth { preset, patch }
}

/// The sound of a part as its genre draws it: one of the genre's own (the part's `presets`), varied a little, or, by
/// the genre's palette for the role, a synth preset made for the role, varied more, or an installed instrument
/// (`palette`), none of them one the song's other parts play (`used`, by `palette::identity`). The genre's own is
/// drawn from `rng` either way, so the rest of the part comes out the same.
pub fn draw_sound(
    spec: &PartSpec,
    g: &GenreProfile,
    available: &[palette::Instrument],
    used: &BTreeSet<String>,
    rng: &mut Rng,
) -> Sound {
    let preset = rng.pick(&spec.presets).clone();
    let own = part_sound(&preset, rng);
    let mut rp = g.palette.get(spec.role.name()).cloned().unwrap_or_else(|| palette::default_for(spec.role));
    rp.core = spec.core.unwrap_or(rp.core);
    if rp.synths.is_empty() {
        rp.synths = fitting_synths(g, spec.role);
    }
    if rp.instruments.is_empty() {
        rp.instruments = named_instruments(g, spec.role);
    }
    // its own is another part's already: one from the palette instead, unless it must keep its own
    if palette::identity(&own).is_some_and(|k| used.contains(&k)) && spec.core != Some(1.0) {
        rp.core = 0.0;
    }
    let mut sr = rng.fork("palette");
    match palette::draw(spec.role, spec.register, &rp, available, used, &mut sr) {
        None => widen(own, 0.5, &mut sr),
        Some(palette::Drawn::Synth(name)) => {
            let patch = presets::get(name).unwrap_or_else(|| presets::get("pluck").unwrap());
            widen(Sound::Synth { preset: name.to_string(), patch }, 1.0, &mut sr)
        }
        Some(palette::Drawn::Instrument(name)) => {
            let label = name.rsplit(':').next().unwrap_or(&name).to_string();
            Sound::Instrument { name, label }
        }
    }
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

/// Whether a song plays a part (or a loop) its genre leaves out with the chance `optional`, as written (at 0.5) and at
/// `complexity`, from one draw of `rng` when it is optional
fn kept(optional: f64, complexity: f64, rng: &mut Rng) -> (bool, bool) {
    if optional <= 0.0 {
        return (true, true);
    }
    let u = rng.f64();
    (u >= optional, u >= left_out_at(optional, complexity))
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
    // the family's loops play its own chords everywhere, choruses too
    if bp.harmony.progressions.remove("C").is_some() {
        for s in bp.form.iter_mut().filter(|s| s.prog == "C") {
            s.prog = "A".into();
        }
    }
    let add = |bp: &mut Blueprint, name: &str, role: &str, sound: Sound, mix: TrackMix| {
        // the part the loop takes the place of goes, and its doubles with it, from the sections too
        let gone: Vec<String> = bp
            .parts
            .iter()
            .filter(|p| p.name == name || p.doubles.as_deref() == Some(name))
            .map(|p| p.name.clone())
            .collect();
        bp.parts.retain(|p| !gone.contains(&p.name));
        for s in bp.form.iter_mut() {
            s.layers.retain(|l| l == name || !gone.contains(l));
        }
        bp.loops.retain(|l| l.name != name);
        bp.loops.push(LoopPart {
            name: name.into(),
            role: role.into(),
            sound,
            transpose: 0,
            chop: Vec::new(),
            slice_beats: 0.5,
            retime: false,
            mix,
            from: None,
            sampled: false,
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
            TrackMix { bus: "drums".into(), level_db: FAMILY_BEAT_DB, ..Default::default() },
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
    // what was generated, said again; what else it says (how it was layered, what it sampled) stays
    let generated = format!(
        "generated: {} seed {seed} around the Apple Loops family '{}' ({:.0} BPM, {}, chords {})",
        g.name,
        f.name,
        f.bpm,
        f.key.name(),
        prog.join(" ")
    );
    match bp.lineage.first_mut() {
        Some(first) => *first = generated,
        None => bp.lineage.push(generated),
    }
    bp
}

fn generate_inner(g: &GenreProfile, seed: u64, opts: &Options, lib: Option<&dyn Library>) -> Blueprint {
    let mut rng = Rng::new(seed);
    let complexity = opts.complexity.unwrap_or(g.complexity).clamp(0.0, 1.0);
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
    let mut bpc = *pick_weighted(&g.bars_per_chord, &mut hr);
    if prog_a.len() == 1 {
        bpc = 4;
    }
    // eight chords loop in sixteen bars at most, so that a section hears the whole of them
    if prog_a.len().max(prog_b.len()) >= 8 {
        bpc = bpc.min(2);
    }
    let (drawn_a, drawn_b) = (prog_a.clone(), prog_b.clone());
    // a loop answered, the second time round ending another way, when the eight chords still fit (in the style's
    // key, like everything here, so that a key of your own gets what key() would make of it)
    let answered = |p: Vec<String>, chance: f64, label: &str| -> Vec<String> {
        let mut r = hr.fork(label);
        if bpc <= 2 && r.chance(chance) { progression::answer(&p, &style_key, &mut r).map_or(p, |(a, _)| a) } else { p }
    };
    let (style_a, style_b) = (answered(prog_a, 0.4, "answer A"), answered(prog_b, 0.25, "answer B"));
    let (prog_a, prog_b) = (in_key(style_a.clone()), in_key(style_b.clone()));
    let mut progressions = BTreeMap::new();
    progressions.insert("A".to_string(), prog_a);
    progressions.insert("B".to_string(), prog_b);

    // form
    let mut fr = rng.fork("form");
    let fi = opts.form.unwrap_or_else(|| fr.below(g.forms.len().max(1))).min(g.forms.len().saturating_sub(1));
    // a form of its own, unless one was asked for
    let mut form = if opts.form.is_some() {
        g.forms[fi].clone()
    } else {
        crate::form::vary(&g.forms[fi], g, bpm, &fr.fork("vary"))
    };
    if opts.short {
        for s in form.iter_mut() {
            s.bars = (s.bars / 2).max(4);
        }
    }
    // a chorus of its own: the hooks on a progression the verses do not play
    if form.iter().any(|s| s.kind == SectionKind::Hook)
        && form.iter().any(|s| s.kind == SectionKind::Verse)
        && let mut cr = hr.fork("chorus")
        && cr.chance(0.35)
    {
        // not what the verses or the breaks play (as drawn or answered, or the first half of eight chords), and no
        // longer a loop than the verses'
        let played = [&drawn_a, &drawn_b, &style_a, &style_b];
        let halves: Vec<&[String]> = played.iter().filter(|p| p.len() == 8).map(|p| &p[..4]).collect();
        let heard = |p: &Vec<String>| played.contains(&p) || halves.contains(&p.as_slice());
        let longest = (style_a.len() as u32 * bpc).max(16);
        let others: Vec<Vec<String>> = style_progs
            .iter()
            .map(|p| progression::parse(p))
            .filter(|p| !heard(p) && p.len() as u32 * bpc <= longest)
            .collect();
        let c = if !others.is_empty() && cr.chance(0.5) {
            cr.pick(&others).clone()
        } else {
            let (c, _) = progression::substitute(&style_a, &style_key, &mut cr);
            if heard(&c) { progression::substitute(&c, &style_key, &mut cr).0 } else { c }
        };
        if !heard(&c) {
            progressions.insert("C".to_string(), in_key(c));
            for s in form.iter_mut().filter(|s| s.kind == SectionKind::Hook && s.prog == "A") {
                s.prog = "C".into();
            }
        }
    }

    // drums (on a drum machine from the sample packs, sometimes; some lanes on a kit of their own)
    let mut dr = rng.fork("drums");
    let kit = pick_kit(&g.kits, &mut rng.fork("kit"));
    let kits = palette::kits();
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
        let sound =
            lane_kit(&spec.name, g.kit_mix, &kits, &mut dr.fork(&format!("{}-kit", spec.name))).unwrap_or(sound);
        let feel = &mut dr.fork(&format!("{}-feel", spec.name));
        drums.push(DrumLane {
            late_ms: spec.late_ms.map_or(0.0, |(lo, hi)| feel.range(lo as f64, hi as f64).round() as f32),
            length: None,
            speed: 1.0,
            swing: None,
            groove: None,
            groove_amount: 1.0,
            name: spec.name.clone(),
            sound,
            patterns,
            pattern_bars,
            pitch: 0,
            choke: spec.choke.clone(),
            mix: spec.mix.clone(),
        });
    }

    // parts: each the genre plays as written drawn (and coloured, below) as it would be, kept or not, so that the
    // others come out the same at any complexity; one only a more layered song plays, after them from its own fork
    let mut pr = rng.fork("parts");
    let mut written = Vec::new();
    let available = palette::instruments();
    let mut used = BTreeSet::new();
    let mut late = Vec::new();
    for spec in &g.parts {
        let (as_written, kept) = kept(spec.optional, complexity, &mut pr);
        if as_written {
            written.push((new_part(spec, g, &available, &mut used, &mut pr), kept));
        } else if kept {
            late.push((written.iter().filter(|w| w.1).count(), spec));
        }
    }
    let late: Vec<(usize, Part)> = late
        .into_iter()
        .map(|(at, spec)| (at, new_part(spec, g, &available, &mut used, &mut pr.fork(&spec.name))))
        .collect();

    // loops from the library
    let mut lr = rng.fork("loops");
    let mut loops = Vec::new();
    if opts.loops
        && let Some(l) = lib
    {
        for spec in &g.loops {
            let (as_written, kept) = kept(spec.optional, complexity, &mut lr);
            let found = match as_written {
                true => l.find_loop(spec, g, bpm, &key, &mut lr),
                false if kept => l.find_loop(spec, g, bpm, &key, &mut lr.fork(&spec.name)),
                false => None,
            };
            if kept && let Some((sound, transpose)) = found {
                loops.push(LoopPart {
                    name: spec.name.clone(),
                    role: spec.role.clone(),
                    sound,
                    transpose,
                    chop: Vec::new(),
                    slice_beats: 0.5,
                    retime: false,
                    mix: spec.mix.clone(),
                    from: None,
                    sampled: false,
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
    let mut mix = MixPlan {
        reverb: g.mix.reverb.clone(),
        delay: g.mix.delay.clone(),
        buses: g.mix.buses.clone(),
        duck_release: g.mix.duck_release,
        levels: g.mix.levels.clone(),
        bands: g.mix.bands.clone(),
        master: Master { fx: g.mix.master.clone(), lufs: g.mix.lufs, ceiling_db: -1.0 },
    };
    // a mix of its own
    let (mut parts, keep): (Vec<Part>, Vec<bool>) = written.into_iter().unzip();
    crate::colour::vary(&mut mix, &mut parts, &rng.fork("colour"));
    let mut parts: Vec<Part> = parts.into_iter().zip(keep).filter_map(|(p, k)| k.then_some(p)).collect();
    for (at, mut part) in late.into_iter().rev() {
        let own = rng.fork("colour").fork(&part.name);
        crate::colour::vary_part(&mut part, &own);
        parts.insert(at, part);
    }
    let mut bp = Blueprint {
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
        grooves: BTreeMap::new(),
    };
    // the samples a sampling genre is built on
    crate::sampling::apply(&mut bp, g, opts, lib, &rng.fork("sampling"));
    layers::layer(&mut bp, g, complexity, lib, &rng.fork("layers"));
    bp
}

/// a part as the genre describes it: its sound drawn as `draw_sound` does (and added to `used`), its style, density
/// and rate drawn from the genre's choices
pub fn new_part(
    spec: &PartSpec,
    g: &GenreProfile,
    available: &[palette::Instrument],
    used: &mut BTreeSet<String>,
    rng: &mut Rng,
) -> Part {
    let sound = draw_sound(spec, g, available, used, rng);
    used.extend(palette::identity(&sound));
    let mut mix = spec.mix.clone();
    // harmony parts from single-voice patches or mono recordings would sit dead centre: a light chorus spreads them
    let harmony = matches!(spec.role, Role::Chords | Role::Keys | Role::Pad);
    let centred = harmony
        && match &sound {
            Sound::Synth { patch, .. } => patch.osc.iter().all(|o| o.voices <= 1),
            Sound::Instrument { name, .. } => claudius_packs::instrument(name, None).is_ok_and(|i| i.recorded_mono()),
            _ => false,
        };
    if centred && !mix.fx.iter().any(|e| e.name() == "chorus") {
        mix.fx.push(Effect::Chorus { rate_hz: 0.45, depth_ms: 2.5, delay_ms: 11.0, mix: 0.4.into() });
    }
    Part {
        // from a stream of its own, so that a genre's feel leaves the rest of its parts as they were; a whole ms
        late_ms: spec.late_ms.map_or(0.0, |(lo, hi)| rng.fork("feel").range(lo as f64, hi as f64).round() as f32),
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
        doubles: spec.doubles.clone(),
        octave: spec.octave,
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

/// The semitones that move a recording in `from` (its tonic, and whether it is minor) into `key` by the shortest way:
/// onto its tonic when both are minor or both major, else onto its relative minor or major.
pub fn transpose_into(key: &Key, (tonic, minor): (u8, bool)) -> i32 {
    let target = if minor == key.mode.is_minor() {
        key.tonic
    } else if minor {
        (key.tonic + 9) % 12
    } else {
        (key.tonic + 3) % 12
    };
    Key::new(target, key.mode).transpose_from(tonic)
}

/// flip a recording into the song: warped to the tempo, transposed to the key, re-sliced in half-beat pieces (or
/// played straight), placed in the high-energy sections
pub fn add_sample(bp: &mut Blueprint, path: &str, bpm: f64, beats: f64, key: Option<(u8, bool)>, rng: &mut Rng) {
    let transpose = key.map_or(0, |from| transpose_into(&bp.key, from));
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
        retime: false,
        mix: TrackMix { duck: 0.4, reverb: 0.15, bus: "music".into(), ..Default::default() },
        from: None,
        sampled: false,
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
