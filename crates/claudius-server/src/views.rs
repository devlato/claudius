//! The engine's documents as the wire types of `claudius-api`.

use std::collections::BTreeMap;

use claudius::judge::{EarsReport, Verdict};
use claudius::model::{Blueprint, Sound, TrackMix};
use claudius::render::Rendered;
use claudius::synth::presets;
use claudius::{genres, script};
use claudius_api::*;

pub fn genre_infos() -> Vec<GenreInfo> {
    genres::all()
        .unwrap_or_default()
        .into_iter()
        .map(|g| GenreInfo { name: g.name, description: g.description, bpm: g.bpm })
        .collect()
}

fn track(
    name: &str,
    kind: &str,
    role: &str,
    sound: String,
    m: &TrackMix,
    loudness: &BTreeMap<String, f64>,
) -> TrackView {
    TrackView {
        name: name.to_string(),
        kind: kind.to_string(),
        role: role.to_string(),
        sound,
        level_db: m.level_db,
        pan: m.pan,
        reverb: m.reverb,
        delay: m.delay,
        duck: m.duck,
        muted: m.mute,
        solo: m.solo,
        loudness: loudness.get(name).copied(),
    }
}

pub fn song(bp: &Blueprint, loudness: &BTreeMap<String, f64>) -> SongView {
    let g = genres::get(&bp.genre).ok();
    let mut start = 0;
    let sections = bp
        .form
        .iter()
        .map(|s| {
            let v = SectionView {
                name: s.name.clone(),
                kind: s.kind.name().to_string(),
                start_bar: start,
                bars: s.bars,
                energy: s.energy,
                layers: s.layers.clone(),
                prog: s.prog.clone(),
                variant: s.variant.clone(),
                filter_in: s.filter_in,
                build: s.build,
                gap: s.gap,
                impact: s.impact,
                fills: s.fills,
            };
            start += s.bars;
            v
        })
        .collect();
    let mut tracks = Vec::new();
    for d in &bp.drums {
        tracks.push(track(&d.name, "drum", &d.name, d.sound.describe(), &d.mix, loudness));
    }
    for p in &bp.parts {
        tracks.push(track(&p.name, "part", p.role.name(), p.sound.describe(), &p.mix, loudness));
    }
    for l in &bp.loops {
        tracks.push(track(&l.name, "loop", &l.role, l.sound.describe(), &l.mix, loudness));
    }
    for p in &bp.patterns {
        let sounds = p.sounds.iter().map(|(k, s)| format!("{k}: {}", s.describe())).collect::<Vec<_>>().join(", ");
        let mut t =
            track(&p.name, "pattern", if p.role.is_empty() { "mixed" } else { &p.role }, sounds, &p.mix, loudness);
        if t.loudness.is_none() {
            // a pattern with several sounds renders as name.sound tracks: report the loudest
            t.loudness = loudness
                .iter()
                .filter(|(k, _)| k.starts_with(&format!("{}.", p.name)))
                .map(|(_, v)| *v)
                .fold(None, |a: Option<f64>, b| Some(a.map_or(b, |a| a.max(b))));
        }
        tracks.push(t);
    }
    tracks.push(track("fx", "fx", "fx", "risers, impacts, crashes".into(), &bp.transitions.mix, loudness));
    let drums = bp
        .drums
        .iter()
        .map(|d| DrumView {
            lane: d.name.clone(),
            pattern_bars: d.pattern_bars,
            variants: d
                .patterns
                .iter()
                .map(|(v, hits)| {
                    let mut steps = vec![0.0f32; (d.pattern_bars * 16) as usize];
                    for h in hits {
                        let i = (h.t * 4.0).round() as usize;
                        if let Some(s) = steps.get_mut(i) {
                            *s = s.max(h.v);
                        }
                    }
                    (v.clone(), steps)
                })
                .collect(),
        })
        .collect();
    let parts = bp
        .parts
        .iter()
        .map(|p| {
            let spec = g.as_ref().and_then(|g| g.parts.iter().find(|s| s.name == p.name));
            let mut styles = spec.map(|s| s.styles.clone()).unwrap_or_default();
            if !styles.contains(&p.style) {
                styles.insert(0, p.style.clone());
            }
            // the genre's choices (an "a|b" entry offers both), the synths for the role, then installed instruments
            let mut presets_for: Vec<String> = Vec::new();
            let mut offer = |n: &str| {
                if !n.is_empty() && !presets_for.iter().any(|x| x == n) {
                    presets_for.push(n.to_string());
                }
            };
            for entry in spec.map(|s| s.presets.clone()).unwrap_or_default() {
                for alt in entry.split('|') {
                    if presets::get(alt).is_some() || claudius_packs::resolve(alt, None).is_some() {
                        offer(alt);
                    }
                }
            }
            for n in presets::for_role(p.role.name()) {
                offer(n);
            }
            for n in instruments_for(p.role.name()) {
                if claudius_packs::resolve(n, None).is_some() {
                    offer(n);
                }
            }
            PartView {
                name: p.name.clone(),
                role: p.role.name().to_string(),
                style: p.style.clone(),
                styles,
                preset: match &p.sound {
                    Sound::Synth { preset, .. } => preset.clone(),
                    Sound::Instrument { label, name } => {
                        if label.is_empty() {
                            name.clone()
                        } else {
                            label.clone()
                        }
                    }
                    other => other.describe(),
                },
                presets: presets_for,
                register: p.register,
                density: p.density,
            }
        })
        .collect();
    SongView {
        title: bp.title.clone(),
        genre: bp.genre.clone(),
        seed: bp.seed,
        bpm: bp.bpm,
        key: bp.key.name(),
        swing: bp.swing,
        bars: bp.bars(),
        seconds: bp.secs(),
        progressions: bp.harmony.progressions.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        sections,
        layers: bp.layer_names(),
        tracks,
        drums,
        parts,
        script: script::to_script(bp),
        lineage: bp.lineage.clone(),
    }
}

pub fn ears(e: &EarsReport) -> EarsView {
    EarsView {
        genre_query: e.genre_query.clone(),
        genre_z: e.genre_z,
        best_alt: e.best_alt.clone(),
        best_alt_z: e.best_alt_z,
        production_z: e.production_z,
        worst_flaw: e.worst_flaw.clone(),
        worst_flaw_z: e.worst_flaw_z,
        sounds_like: e.sounds_like.clone(),
        p_real: e.p_real,
    }
}

pub fn verdict(v: &Verdict) -> VerdictView {
    VerdictView {
        total: v.total,
        summary: v.summary.clone(),
        scores: v.scores.iter().map(|(k, x)| (k.clone(), *x)).collect(),
        issues: v.issues.iter().map(|i| (i.severity, i.what.clone(), i.target.clone())).collect(),
        ears: v.ears.as_ref().map(ears),
        measures: v.measures.iter().map(|(k, x)| (k.clone(), *x)).collect(),
    }
}

/// peak envelope of the master, `n` points 0..1
pub fn waveform(r: &Rendered, n: usize) -> Vec<f32> {
    let len = r.master.l.len();
    if len == 0 {
        return Vec::new();
    }
    let step = (len / n).max(1);
    (0..len.div_ceil(step))
        .map(|i| {
            let (a, b) = (i * step, ((i + 1) * step).min(len));
            r.master.l[a..b]
                .iter()
                .zip(&r.master.r[a..b])
                .map(|(l, r)| l.abs().max(r.abs()))
                .fold(0.0f32, f32::max)
                .min(1.0)
        })
        .collect()
}

/// sampled instruments that suit a part's role (offered when their pack is installed)
pub fn instruments_for(role: &str) -> &'static [&'static str] {
    match role {
        "bass" => &["upright_bass", "electric_bass", "synth_bass", "contrabass"],
        "chords" | "keys" => &[
            "piano",
            "upright",
            "rhodes",
            "wurlitzer",
            "cp80",
            "hammond",
            "church_organ",
            "nylon_guitar",
            "electric_guitar",
        ],
        "pad" => &["string_ensemble", "choir", "violins", "cellos", "brass", "woodwinds"],
        "lead" => &[
            "piano",
            "rhodes",
            "vibes",
            "marimba",
            "kalimba",
            "flute",
            "clarinet",
            "sax",
            "trumpet",
            "violins",
            "nylon_guitar",
            "harmonica",
        ],
        "arp" => &["harp", "marimba", "glock", "pizzicato", "kalimba", "vibes"],
        _ => &[],
    }
}

pub fn sounds() -> SoundList {
    let mut instruments: Vec<String> = claudius_packs::aliases().into_iter().map(|(name, _)| name).collect();
    instruments.extend(claudius_packs::instruments().into_iter().map(|i| i.name));
    instruments.dedup();
    let packs = claudius_packs::packs()
        .into_iter()
        .map(|p| PackInfo {
            name: p.name,
            description: if p.description.is_empty() { p.title } else { p.description },
            license: p.licence,
            installed: p.installed,
            size_mb: p.size_mb as f64,
        })
        .collect();
    SoundList {
        drums: script::drum_names().into_iter().map(String::from).collect(),
        synths: presets::names().into_iter().map(String::from).collect(),
        instruments,
        packs,
    }
}

/// the song in a few lines, for people and assistants
pub fn summary(s: &SongView, v: Option<&VerdictView>, r: Option<&RenderView>) -> String {
    let mut out = format!(
        "{} — {} at {:.0} BPM in {}, swing {:.2}, {} bars ({}:{:02})\n",
        s.title,
        s.genre,
        s.bpm,
        s.key,
        s.swing,
        s.bars,
        (s.seconds / 60.0).floor() as u32,
        (s.seconds % 60.0).round() as u32
    );
    for (k, p) in &s.progressions {
        out.push_str(&format!("progression {k}: {}\n", p.join(" ")));
    }
    out.push_str("form:\n");
    for (i, sec) in s.sections.iter().enumerate() {
        let mut flags = Vec::new();
        for (on, f) in [
            (sec.build, "build"),
            (sec.gap, "gap"),
            (sec.impact, "impact"),
            (sec.filter_in, "filter_in"),
            (sec.fills, "fills"),
        ] {
            if on {
                flags.push(f);
            }
        }
        out.push_str(&format!(
            "  {i}. {} ({}) bar {} +{}, energy {:.2}, drums '{}'{}: {}\n",
            sec.name,
            sec.kind,
            sec.start_bar,
            sec.bars,
            sec.energy,
            sec.variant,
            if flags.is_empty() { String::new() } else { format!(" [{}]", flags.join(", ")) },
            sec.layers.join(" ")
        ));
    }
    out.push_str("tracks:\n");
    for t in &s.tracks {
        out.push_str(&format!(
            "  {} [{} {}] {}; level {:+.1} dB, pan {:+.2}, reverb {:.2}, delay {:.2}, duck {:.2}{}{}\n",
            t.name,
            t.kind,
            t.role,
            t.sound,
            t.level_db,
            t.pan,
            t.reverb,
            t.delay,
            t.duck,
            if t.muted { ", MUTED" } else { "" },
            t.loudness.map(|l| format!(", {l:.1} LUFS")).unwrap_or_default()
        ));
    }
    if let Some(r) = r {
        out.push_str(&format!(
            "render: {:.1} LUFS, true peak {:.1} dBTP, {:.0} s\n",
            r.lufs, r.true_peak_db, r.seconds
        ));
    }
    if let Some(v) = v {
        out.push_str(&verdict_text(v));
    }
    out
}

pub fn verdict_text(v: &VerdictView) -> String {
    let mut out = format!("verdict {:.1}/100: {}\n", v.total, v.summary);
    out.push_str(&format!(
        "  scores: {}\n",
        v.scores.iter().map(|(k, x)| format!("{k} {x:.2}")).collect::<Vec<_>>().join(", ")
    ));
    if let Some(e) = &v.ears {
        out.push_str(&format!(
            "  ears: '{}' z {:+.1} (closest confusion '{}' z {:+.1}); production z {:+.1} vs '{}' z {:+.1}; sounds like {}\n",
            e.genre_query,
            e.genre_z,
            e.best_alt,
            e.best_alt_z,
            e.production_z,
            e.worst_flaw,
            e.worst_flaw_z,
            e.sounds_like.join(", ")
        ));
    }
    for (sev, what, target) in v.issues.iter().take(8) {
        out.push_str(&format!("  issue ({sev:.2}): {what} — fix: {target}\n"));
    }
    out
}
