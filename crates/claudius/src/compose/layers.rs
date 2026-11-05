//! `layer`: a song as layered as its complexity says (`Options::complexity`). At 0.5 it is the song its genre's profile
//! writes. Above that, up to 1, more plays on top of it:
//!
//! - a pad holding the chords, in a genre whose songs have none;
//! - a counter-melody under the lead, answering it in the verses;
//! - an arpeggio sparkling over the hooks, in a genre whose songs have none;
//! - its harmony and its lead doubled on second instruments, an octave up or in unison on another sound, and its bass an
//!   octave up through a high-pass, so that its harmonics carry it;
//! - a clap on the snare's backbeats, a shaker, and a percussion line;
//! - the drums a sampled break stands in for, played over the break in the hooks.
//!
//! Each comes in where the song lifts, so that its sections build. Below 0.5, down to 0, its sections are thinned to
//! their core: the parts and drum lanes that only colour them are left out, of its quieter sections most. Each layer
//! draws from a fork of its own, after everything else, so that a song's seed makes the same song under them.

use super::*;

/// how far above (`more`) and below (`less`) the genre as written `complexity` is, each from 0 to 1
pub fn reach(complexity: f64) -> (f64, f64) {
    (((complexity - 0.5) / 0.5).clamp(0.0, 1.0), ((0.5 - complexity) / 0.5).clamp(0.0, 1.0))
}

/// The chance a genre leaves out a part (or a loop) its songs may play, `optional`, at `complexity`: as the genre has
/// it at 0.5, never at 1, always at 0.
pub fn left_out_at(optional: f64, complexity: f64) -> f64 {
    let (more, less) = reach(complexity);
    optional * (1.0 - more) + (1.0 - optional) * less
}

/// The chance a genre gives something that colours its songs (a sampled break or texture, a part it may play), at
/// `complexity`: as the genre has it at 0.5, sure at 1, none at 0.
pub fn chance_at(chance: f64, complexity: f64) -> f64 {
    let (more, less) = reach(complexity);
    (chance + (1.0 - chance) * more) * (1.0 - less)
}

fn peak(s: &SectionSpec) -> bool {
    matches!(s.kind, SectionKind::Hook | SectionKind::Drop)
}

/// whether a section lifts enough for a layer that comes in from energy `from` (a hook or a drop always does)
fn lifts(s: &SectionSpec, from: f32) -> bool {
    peak(s) || s.energy >= from
}

/// what a sound is called in the song's history
fn called(sound: &Sound) -> String {
    match sound {
        Sound::Synth { preset, .. } => preset.replace('_', " "),
        Sound::Instrument { label, .. } => label.replace('_', " "),
        Sound::Drum { voice, .. } => voice.role().to_string(),
        _ => "a sample".into(),
    }
}

/// `bp` layered or thinned to `complexity`, as the module's doc says
pub fn layer(bp: &mut Blueprint, g: &GenreProfile, complexity: f64, lib: Option<&dyn Library>, rng: &Rng) {
    let (more, less) = reach(complexity);
    if more == 0.0 && less == 0.0 {
        return;
    }
    let before: Vec<Vec<String>> = bp.form.iter().map(|s| s.layers.clone()).collect();
    let mut said: Vec<String> = Vec::new();
    if more > 0.0 {
        let available = palette::instruments();
        let mut used: BTreeSet<String> = bp.parts.iter().filter_map(|p| palette::identity(&p.sound)).collect();
        let mut draw = |bp: &mut Blueprint, spec: PartSpec, rng: &mut Rng| -> Part {
            let part = new_part(&spec, g, &available, &mut used, rng);
            bp.parts.push(part.clone());
            part
        };
        if let Some(said_pad) = pad(bp, g, more, &mut draw, &mut rng.fork("pad")) {
            said.push(said_pad);
        }
        said.extend(counter(bp, g, more, &mut draw, &mut rng.fork("counter")));
        said.extend(sparkle(bp, g, more, &mut draw, &mut rng.fork("sparkle")));
        said.extend(doubles(bp, g, more, &mut draw, &mut rng.fork("doubles")));
        said.extend(percussion(bp, g, more, lib, &rng.fork("percussion")));
        said.extend(over_the_break(bp, g, more, &mut rng.fork("over the break")));
    }
    if less > 0.0 {
        said.extend(thin(bp, g, less, &mut rng.fork("thin")));
    }
    for (s, was) in bp.form.iter_mut().zip(&before).filter(|(s, was)| s.layers != **was) {
        crate::form::retune(s, was, g);
    }
    if !said.is_empty() {
        bp.lineage.push(format!("layered at complexity {complexity:.2}: {}", said.join("; ")));
    }
}

/// a part spec for a layer of `role` the genre has no part for: the genre's own part of that role if it has one, else
/// one sounding as the genre's palette for the role draws
fn spec_for(g: &GenreProfile, name: &str, role: Role, fallback: &str) -> PartSpec {
    let own = g.parts.iter().find(|p| p.role == role);
    PartSpec {
        name: name.into(),
        role,
        presets: own.map_or_else(|| vec![fallback.to_string()], |p| p.presets.clone()),
        styles: own.map_or_else(|| vec![styles_for(role)[0].to_string()], |p| p.styles.clone()),
        register: own.map_or((48, 84), |p| p.register),
        density: own.map_or((0.3, 0.5), |p| p.density),
        voices: own.map_or(3, |p| p.voices),
        voicing: own.map_or(Voicing::Open, |p| p.voicing),
        rates: own.map_or_else(Vec::new, |p| p.rates.clone()),
        optional: 0.0,
        core: None,
        mix: own.map(|p| p.mix.clone()).unwrap_or_default(),
    }
}

/// a pad holding the chords, in a song with none: in its intro, breaks and outro, and under its peaks
fn pad(
    bp: &mut Blueprint,
    g: &GenreProfile,
    more: f64,
    draw: &mut impl FnMut(&mut Blueprint, PartSpec, &mut Rng) -> Part,
    r: &mut Rng,
) -> Option<String> {
    if bp.parts.iter().any(|p| p.role == Role::Pad) || !r.chance(0.7 * more) {
        return None;
    }
    let mix = TrackMix {
        level_db: -3.0,
        reverb: 0.5,
        fx: vec![Effect::Width { width: 1.4.into(), mono_below_hz: 200.0 }],
        ..Default::default()
    };
    let spec = PartSpec {
        styles: vec!["sustain".into()],
        register: (48, 72),
        voices: 3,
        voicing: Voicing::Open,
        mix,
        ..spec_for(g, "pad", Role::Pad, "warm_pad")
    };
    let part = draw(bp, spec, r);
    let everywhere = more >= 0.8;
    for s in bp.form.iter_mut() {
        let quiet = matches!(s.kind, SectionKind::Intro | SectionKind::Break | SectionKind::Outro);
        if (quiet || peak(s) || everywhere) && !s.layers.is_empty() {
            s.layers.push(part.name.clone());
        }
    }
    Some(format!("a pad on {}", called(&part.sound)))
}

/// a counter-melody under the lead, sparser and lower: with it in the peaks, and in the sections that lift without it
fn counter(
    bp: &mut Blueprint,
    g: &GenreProfile,
    more: f64,
    draw: &mut impl FnMut(&mut Blueprint, PartSpec, &mut Rng) -> Part,
    r: &mut Rng,
) -> Option<String> {
    let lead = bp.parts.iter().find(|p| p.role == Role::Lead && p.doubles.is_none())?;
    let (lead_name, lead_register, lead_mix) = (lead.name.clone(), lead.register, lead.mix.clone());
    if more < 0.3 || bp.parts.iter().any(|p| p.name == "counter") || !r.chance(0.65 * more) {
        return None;
    }
    let low = (lead_register.0 - 10).max(48);
    let spec = PartSpec {
        styles: vec!["motif".into()],
        register: (low, low + 14),
        density: (0.15, 0.3),
        mix: TrackMix {
            level_db: lead_mix.level_db - 5.0,
            pan: if lead_mix.pan.abs() > 0.1 {
                -lead_mix.pan
            } else if r.chance(0.5) {
                0.3
            } else {
                -0.3
            },
            reverb: (lead_mix.reverb + 0.1).min(1.0),
            ..lead_mix.clone()
        },
        ..spec_for(g, "counter", Role::Lead, "fm_lead")
    };
    let part = draw(bp, spec, r);
    for s in bp.form.iter_mut() {
        let with_lead = s.layers.contains(&lead_name);
        if (with_lead && peak(s)) || (!with_lead && s.energy >= 0.5 && s.layers.len() >= 3) {
            s.layers.push(part.name.clone());
        }
    }
    Some(format!("a counter-melody on {}", called(&part.sound)))
}

/// an arpeggio of the chords high over the peaks, in a song with none, quietly, through a delay
fn sparkle(
    bp: &mut Blueprint,
    g: &GenreProfile,
    more: f64,
    draw: &mut impl FnMut(&mut Blueprint, PartSpec, &mut Rng) -> Part,
    r: &mut Rng,
) -> Option<String> {
    if more < 0.2 || bp.parts.iter().any(|p| p.role == Role::Arp) || !r.chance(0.7 * more) {
        return None;
    }
    let style = *r.pick(&["up", "updown", "pinky", "random"]);
    let spec = PartSpec {
        styles: vec![style.into()],
        register: (72, 96),
        rates: vec![if bp.bpm < 110.0 { 0.25 } else { 0.5 }],
        mix: TrackMix {
            level_db: -3.0,
            pan: r.range(-0.4, 0.4) as f32,
            reverb: 0.35,
            delay: 0.25,
            ..Default::default()
        },
        ..spec_for(g, "sparkle", Role::Arp, "bell")
    };
    let part = draw(bp, spec, r);
    let last_verse = bp.form.iter().rposition(|s| s.kind == SectionKind::Verse);
    for (i, s) in bp.form.iter_mut().enumerate() {
        if peak(s) || (more >= 0.8 && Some(i) == last_verse) {
            s.layers.push(part.name.clone());
        }
    }
    Some(format!("an arpeggio on {}", called(&part.sound)))
}

/// The song's harmony, lead and bass parts, each doubled with the chance `more` gives it: the same notes on a second
/// sound drawn as the part's own would be (none the song plays already), an octave up or in unison, 6 dB under it
/// and to the other side; a bass an octave up, 9 dB under it, through a high-pass and some drive. A double comes in
/// where its part plays and the song lifts (the peaks, and at full complexity the verses too).
fn doubles(
    bp: &mut Blueprint,
    g: &GenreProfile,
    more: f64,
    draw: &mut impl FnMut(&mut Blueprint, PartSpec, &mut Rng) -> Part,
    r: &mut Rng,
) -> Vec<String> {
    let playable = |s: &Sound| matches!(s, Sound::Synth { .. } | Sound::Instrument { .. });
    let sources: Vec<Part> = bp
        .parts
        .iter()
        .filter(|p| p.doubles.is_none() && playable(&p.sound))
        .filter(|p| matches!(p.role, Role::Keys | Role::Chords | Role::Lead | Role::Bass))
        .filter(|p| !bp.parts.iter().any(|q| q.doubles.as_deref() == Some(&p.name)))
        .cloned()
        .collect();
    let from = if more >= 0.8 { 0.45 } else { 0.6 };
    let mut said = Vec::new();
    for src in sources {
        let mut r = r.fork(&src.name);
        let name = format!("{}_double", src.name);
        let chance = if src.role == Role::Bass { 0.45 } else { 0.8 };
        if bp.layer_names().contains(&name) || !r.chance(chance * more) {
            continue;
        }
        let octave = match src.role {
            Role::Bass => 1,
            Role::Lead if src.register.0 >= 60 && r.chance(0.5) => -1,
            _ if src.register.1 + 12 <= 100 && r.chance(0.6) => 1,
            _ => 0,
        };
        let register = (src.register.0 + 12 * octave, src.register.1 + 12 * octave);
        let mut mix = src.mix.clone();
        mix.mute = false;
        mix.solo = false;
        if src.role == Role::Bass {
            mix.level_db -= 9.0;
            mix.fx.push(Effect::Filter {
                mode: crate::dsp::filter::FilterMode::Hp,
                cutoff: 150.0.into(),
                res: 0.0.into(),
                drive: 1.0,
                mix: 1.0.into(),
            });
            mix.fx.push(Effect::Saturate { kind: crate::fx::SatKind::Tube, drive_db: 6.0.into(), mix: 0.5.into() });
        } else {
            mix.level_db -= 6.0;
            mix.pan = if src.mix.pan.abs() > 0.1 {
                -src.mix.pan
            } else if r.chance(0.5) {
                0.3
            } else {
                -0.3
            };
            mix.reverb = (mix.reverb + 0.1).min(1.0);
        }
        let own = g.parts.iter().find(|p| p.name == src.name && p.role == src.role).cloned();
        let spec = PartSpec {
            name: name.clone(),
            register,
            mix,
            ..own.unwrap_or_else(|| spec_for(g, &name, src.role, "pluck"))
        };
        let drawn = draw(bp, spec, &mut r);
        // it plays its part's notes, and were that part gone, notes of the same kind
        let part = bp.parts.iter_mut().find(|p| p.name == drawn.name).expect("drawn above");
        part.doubles = Some(src.name.clone());
        part.octave = octave;
        part.style = src.style.clone();
        part.voices = src.voices;
        part.voicing = src.voicing;
        part.density = src.density;
        part.rate = src.rate;
        part.seed = src.seed;
        for s in bp.form.iter_mut().filter(|s| s.layers.contains(&src.name) && lifts(s, from)) {
            s.layers.push(name.clone());
        }
        let way = match octave {
            1 => "an octave up",
            -1 => "an octave down",
            _ => "in unison",
        };
        said.push(format!("{} doubled {way} on {}", src.name, called(&drawn.sound)));
    }
    said
}

/// a drum lane of the song's own, as a genre's are made: on a one-shot from the library now and then, else drawn
fn lane(
    name: &str,
    voice: DrumVoice,
    patterns: BTreeMap<String, Vec<Hit>>,
    mix: TrackMix,
    lib: Option<&dyn Library>,
    g: &GenreProfile,
    r: &mut Rng,
) -> DrumLane {
    let params = drum_params(voice, &BTreeMap::new(), r);
    let sampled = r.chance(0.75);
    let sound = lib
        .filter(|_| sampled)
        .and_then(|l| l.one_shot(name, &default_hints(name), g, r))
        .unwrap_or(Sound::Drum { voice, params });
    DrumLane {
        length: None,
        speed: 1.0,
        swing: None,
        groove: None,
        groove_amount: 1.0,
        name: name.into(),
        sound,
        patterns,
        pattern_bars: 4,
        pitch: 0,
        choke: None,
        mix,
    }
}

/// the voice a lane of the song plays, as its genre made it
fn voice_of(g: &GenreProfile, lane: &DrumLane) -> Option<DrumVoice> {
    g.drums.iter().find(|d| d.name == lane.name).map(|d| d.voice).or(match &lane.sound {
        Sound::Drum { voice, .. } => Some(*voice),
        _ => None,
    })
}

/// A clap on the snare's backbeats (its hits as loud as a backbeat, none of its ghosts), a shaker on the sixteenths
/// and a percussion line, each in a song without one, with the chance `more` gives it: the clap where the snare plays
/// and the song lifts, the shaker where the hats play, the percussion in the peaks.
fn percussion(bp: &mut Blueprint, g: &GenreProfile, more: f64, lib: Option<&dyn Library>, r: &Rng) -> Vec<String> {
    let has = |bp: &Blueprint, v: &[DrumVoice]| bp.drums.iter().any(|d| voice_of(g, d).is_some_and(|x| v.contains(&x)));
    let mut said = Vec::new();
    let mut cr = r.fork("clap");
    let snare = bp.drums.iter().find(|d| voice_of(g, d) == Some(DrumVoice::Snare)).cloned();
    if let Some(snare) = snare
        && !has(bp, &[DrumVoice::Clap])
        && !bp.layer_names().contains(&"clap".to_string())
        && cr.chance(0.7 * more)
    {
        let patterns = snare
            .patterns
            .iter()
            .map(|(k, hits)| {
                let backbeats = hits.iter().filter(|h| h.v >= 0.6).map(|h| Hit { v: h.v * 0.9, ..h.clone() });
                (k.clone(), backbeats.collect())
            })
            .collect();
        let mix = TrackMix { level_db: -5.0, reverb: 0.2, bus: snare.mix.bus.clone(), ..Default::default() };
        bp.drums.push(DrumLane {
            pattern_bars: snare.pattern_bars,
            ..lane("clap", DrumVoice::Clap, patterns, mix, lib, g, &mut cr)
        });
        for s in bp.form.iter_mut().filter(|s| s.layers.contains(&snare.name) && lifts(s, 0.6)) {
            s.layers.push("clap".into());
        }
        said.push("a clap on the snare".to_string());
    }
    let mut sr = r.fork("shaker");
    let hats: Vec<String> = bp
        .drums
        .iter()
        .filter(|d| matches!(voice_of(g, d), Some(DrumVoice::ClosedHat | DrumVoice::OpenHat | DrumVoice::Ride)))
        .map(|d| d.name.clone())
        .collect();
    if more >= 0.2
        && !hats.is_empty()
        && !has(bp, &[DrumVoice::Shaker])
        && !bp.layer_names().contains(&"shaker".to_string())
        && sr.chance(0.7 * more)
    {
        let t = crate::theory::rhythm::LaneTemplate {
            steps: [0.55, 0.25, 1.0, 0.3].repeat(4),
            vel: (0.35, 0.6),
            ghost: 0.15,
            rolls: 0.0,
            vary: 0.25,
        };
        let patterns = BTreeMap::from([("main".to_string(), crate::theory::rhythm::generate(&t, 4, &mut sr))]);
        let mix = TrackMix { level_db: -2.0, pan: 0.3, bus: "drums".into(), ..Default::default() };
        bp.drums.push(lane("shaker", DrumVoice::Shaker, patterns, mix, lib, g, &mut sr));
        for s in bp.form.iter_mut().filter(|s| s.layers.iter().any(|l| hats.contains(l)) && s.energy >= 0.5) {
            s.layers.push("shaker".into());
        }
        said.push("a shaker".to_string());
    }
    let mut pr = r.fork("perc");
    if more >= 0.4
        && !has(bp, &[DrumVoice::Perc, DrumVoice::Tom, DrumVoice::Cowbell])
        && !bp.layer_names().contains(&"perc".to_string())
        && pr.chance(0.55 * more)
    {
        let (k, n) = *pr.pick(&[(5, 16), (3, 8), (7, 16), (5, 12)]);
        let rot = pr.below(n);
        let patterns = BTreeMap::from([(
            "main".to_string(),
            crate::theory::rhythm::euclid_hits(k, n, rot, 4, pr.range(0.55, 0.75) as f32),
        )]);
        let mix = TrackMix { level_db: -3.0, pan: -0.35, reverb: 0.15, bus: "drums".into(), ..Default::default() };
        bp.drums.push(lane("perc", DrumVoice::Perc, patterns, mix, lib, g, &mut pr));
        for s in bp.form.iter_mut().filter(|s| peak(s) && !s.layers.is_empty()) {
            s.layers.push("perc".into());
        }
        said.push(format!("a percussion line of {k} in {n}"));
    }
    said
}

/// the drum lanes a sampled break plays instead of, played over it in the peaks, with the chance `more` gives it
fn over_the_break(bp: &mut Blueprint, g: &GenreProfile, more: f64, r: &mut Rng) -> Option<String> {
    let n = crate::sampling::need(g, crate::library::prepared::Role::Break)?;
    let lanes: Vec<String> = n.instead.iter().filter(|l| bp.drums.iter().any(|d| d.name == **l)).cloned().collect();
    if lanes.is_empty() || !bp.loops.iter().any(|l| l.name == crate::sampling::BREAK) || !r.chance(more) {
        return None;
    }
    for s in bp.form.iter_mut().filter(|s| peak(s) && s.layers.iter().any(|l| l == crate::sampling::BREAK)) {
        for l in &lanes {
            if !s.layers.contains(l) {
                s.layers.push(l.clone());
            }
        }
    }
    Some(format!("{} over the break in the peaks", lanes.join(" and ")))
}

/// The layers that only colour a section left out of it with the chance `less` gives each (most of a quiet section,
/// half that of a peak): the drum lanes other than the kick, snare, clap and hats, and the parts other than the bass
/// and the first harmony part. Samples stay, and so does a section's last melodic layer.
fn thin(bp: &mut Blueprint, g: &GenreProfile, less: f64, r: &mut Rng) -> Option<String> {
    let core_drums = [DrumVoice::Kick, DrumVoice::Snare, DrumVoice::Clap, DrumVoice::ClosedHat];
    let colour_drums: Vec<String> = bp
        .drums
        .iter()
        .filter(|d| voice_of(g, d).is_some_and(|v| !core_drums.contains(&v)))
        .map(|d| d.name.clone())
        .collect();
    let harmony = bp.parts.iter().find(|p| matches!(p.role, Role::Keys | Role::Chords)).map(|p| p.name.clone());
    let colour_parts: Vec<String> = bp
        .parts
        .iter()
        .filter(|p| p.role != Role::Bass && Some(&p.name) != harmony.as_ref())
        .map(|p| p.name.clone())
        .collect();
    let melodic: Vec<String> =
        bp.parts.iter().map(|p| p.name.clone()).chain(bp.loops.iter().map(|l| l.name.clone())).collect();
    let mut left = 0;
    for s in bp.form.iter_mut() {
        let chance = if peak(s) { 0.5 * less } else { 0.85 * less };
        let was = s.layers.clone();
        s.layers.retain(|l| !(colour_drums.contains(l) || colour_parts.contains(l)) || !r.chance(chance));
        if !s.layers.iter().any(|l| melodic.contains(l))
            && let Some(last) = was.iter().find(|l| melodic.contains(l))
        {
            s.layers.push(last.clone());
        }
        left += was.len() - s.layers.len();
    }
    (left > 0).then(|| format!("{left} colouring layers left out of its sections"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genres;

    fn song(genre: &str, seed: u64, complexity: f64) -> Blueprint {
        let g = genres::get(genre).unwrap();
        generate(&g, seed, &Options { complexity: Some(complexity), ..Default::default() }, None)
    }

    /// how many tracks a song's sections play, all told
    fn played(bp: &Blueprint) -> usize {
        bp.form.iter().map(|s| s.layers.len()).sum()
    }

    #[test]
    fn a_chance_is_the_genres_at_half_complexity_sure_at_one_and_none_at_naught() {
        assert_eq!((chance_at(0.4, 0.5), chance_at(0.4, 1.0), chance_at(0.4, 0.0)), (0.4, 1.0, 0.0));
        assert_eq!((left_out_at(0.3, 0.5), left_out_at(0.3, 1.0), left_out_at(0.3, 0.0)), (0.3, 0.0, 1.0));
        assert!(chance_at(0.4, 0.75) > 0.4 && chance_at(0.4, 0.25) < 0.4);
    }

    #[test]
    fn a_song_at_half_complexity_is_its_genre_as_written() {
        for genre in ["house", "neo_soul", "boom_bap", "techno"] {
            let g = genres::get(genre).unwrap();
            for seed in 0..4 {
                assert_eq!(song(genre, seed, 0.5), generate(&g, seed, &Options::default(), None), "{genre} {seed}");
            }
        }
    }

    #[test]
    fn a_layered_song_plays_more_at_once_and_a_sparse_one_less() {
        let (mut doubled, mut stacked, mut said) = (0, 0, 0);
        for genre in ["house", "neo_soul", "boom_bap", "techno", "trap", "lofi", "rnb", "dnb"] {
            let (mut sparse, mut written, mut layered) = (0, 0, 0);
            for seed in 0..6 {
                sparse += played(&song(genre, seed, 0.0));
                let as_written = song(genre, seed, 0.5);
                written += played(&as_written);
                let bp = song(genre, seed, 1.0);
                layered += played(&bp);
                doubled += bp.parts.iter().any(|p| p.doubles.is_some()) as usize;
                stacked += (bp.drums.len() > as_written.drums.len()) as usize;
                said += bp.lineage.iter().any(|l| l.starts_with("layered at complexity 1.00: ")) as usize;
            }
            assert!(sparse < written && written < layered, "{genre}: {sparse} < {written} < {layered}");
        }
        assert!(
            doubled >= 40 && stacked >= 30 && said >= 46,
            "of 48 songs, {doubled} doubled a part, {stacked} stacked drums, {said} said so"
        );
    }

    #[test]
    fn a_layered_song_is_its_seeds_song_under_its_layers() {
        for genre in ["house", "neo_soul", "boom_bap", "trap"] {
            for seed in 0..4 {
                let (a, b) = (song(genre, seed, 0.5), song(genre, seed, 1.0));
                assert_eq!(
                    (&a.title, a.bpm, &a.key, &a.harmony),
                    (&b.title, b.bpm, &b.key, &b.harmony),
                    "{genre} {seed}"
                );
                let shape = |bp: &Blueprint| -> Vec<(String, u32, String)> {
                    bp.form.iter().map(|s| (s.name.clone(), s.bars, s.prog.clone())).collect()
                };
                assert_eq!(shape(&a), shape(&b), "{genre} {seed}");
                for d in &a.drums {
                    let e = b.drums.iter().find(|e| e.name == d.name).expect("its lanes stay");
                    assert_eq!((&d.sound, &d.patterns), (&e.sound, &e.patterns), "{genre} {seed} {}", d.name);
                }
                for p in &a.parts {
                    let q = b.parts.iter().find(|q| q.name == p.name).expect("its parts stay");
                    assert_eq!(
                        (&p.sound, &p.style, p.seed, &p.mix),
                        (&q.sound, &q.style, q.seed, &q.mix),
                        "{genre} {seed}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_double_plays_its_parts_notes_an_octave_away_on_a_sound_of_its_own() {
        let bp = (0..30)
            .map(|seed| song("neo_soul", seed, 1.0))
            .find(|bp| bp.parts.iter().any(|p| p.doubles.is_some() && p.octave != 0))
            .expect("a double an octave away");
        let double = bp.parts.iter().find(|p| p.doubles.is_some() && p.octave != 0).unwrap();
        let source = bp.parts.iter().find(|p| Some(&p.name) == double.doubles.as_ref()).unwrap();
        assert_ne!(palette::identity(&double.sound), palette::identity(&source.sound));
        let theirs = cycles(&bp, source);
        let ours = cycles(&bp, double);
        assert!(!ours.is_empty());
        for (prog, notes, period, written) in &ours {
            let (_, notes_of, period_of, _) = theirs.iter().find(|c| c.0 == *prog).expect("its part plays there");
            assert_eq!(period, period_of);
            let at = |ns: &[Note], by: i32| ns.iter().map(|n| (n.t, n.d, n.p + by)).collect::<Vec<_>>();
            assert_eq!(at(notes, 0), at(notes_of, 12 * double.octave), "{prog}");
            assert!(!written);
        }
        // it comes in where its part plays and the song lifts
        for s in bp.form.iter().filter(|s| s.layers.contains(&double.name)) {
            assert!(s.layers.contains(&source.name) && (peak(s) || s.energy >= 0.45), "{}", s.name);
        }
        // a double whose part is gone plays notes of its own
        let mut gone = bp.clone();
        gone.parts.retain(|p| p.name != source.name);
        let own = gone.parts.iter().find(|p| p.name == double.name).unwrap();
        assert_eq!(own.source(&gone).0.name, double.name);
    }

    #[test]
    fn a_sparse_song_keeps_its_core() {
        for genre in ["house", "neo_soul", "boom_bap", "techno", "trap"] {
            for seed in 0..6 {
                let (a, b) = (song(genre, seed, 0.5), song(genre, seed, 0.0));
                let melodic = |bp: &Blueprint, l: &String| {
                    bp.parts.iter().any(|p| p.name == *l) || bp.loops.iter().any(|p| p.name == *l)
                };
                for (s, t) in a.form.iter().zip(&b.form) {
                    if s.layers.iter().any(|l| melodic(&a, l)) {
                        assert!(t.layers.iter().any(|l| melodic(&b, l)), "{genre} {seed} {}: {:?}", t.name, t.layers);
                    }
                    for core in ["kick", "snare"] {
                        let core = core.to_string();
                        assert_eq!(s.layers.contains(&core), t.layers.contains(&core), "{genre} {seed} {}", s.name);
                    }
                }
            }
        }
    }
}
