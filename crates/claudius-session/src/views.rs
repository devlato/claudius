//! The engine's documents as the wire types of `claudius-api`.

use std::collections::BTreeMap;

use claudius::judge::{EarsReport, Verdict};
use claudius::library::prepared::Role;
use claudius::model::{Blueprint, Sound, TrackMix};
use claudius::palette;
use claudius::render::Rendered;
use claudius::synth::presets;
use claudius::{compose, genres, script};
use claudius_api::*;

/// what `State.exports` lists: every format claudius writes, with why this machine cannot write it when it cannot
pub fn export_formats() -> Vec<ExportFormat> {
    static FORMATS: std::sync::OnceLock<Vec<ExportFormat>> = std::sync::OnceLock::new();
    FORMATS
        .get_or_init(|| {
            claudius::export::presets()
                .iter()
                .map(|p| ExportFormat {
                    spec: p.to_string(),
                    label: p.label(claudius::config::SR),
                    extension: p.extension().to_string(),
                    lossless: p.lossless(),
                    unavailable: p.unavailable(),
                })
                .collect()
        })
        .clone()
}

pub fn genre_infos() -> Vec<GenreInfo> {
    genres::all()
        .unwrap_or_default()
        .into_iter()
        .map(|g| {
            let sampling = g.sampling.as_ref().map(|s| GenreSampling {
                by_default: s.by_default,
                roles: [Role::Phrase, Role::Break, Role::Texture, Role::Vocal, Role::Bass]
                    .into_iter()
                    .filter_map(|r| {
                        let n = claudius::sampling::need(&g, r)?;
                        Some(SampleRoleInfo { role: r.name().into(), search: n.search.clone(), seconds: n.seconds })
                    })
                    .collect(),
            });
            GenreInfo {
                name: g.name,
                family: g.family,
                aliases: g.aliases,
                description: g.description,
                bpm: g.bpm,
                sampling,
                complexity: Some(g.complexity),
            }
        })
        .collect()
}

/// a chain of effects as the API has it: each as the song keeps it
fn chain(fx: &[claudius::fx::Effect]) -> Vec<serde_json::Value> {
    fx.iter().map(|e| serde_json::to_value(e).unwrap_or_default()).collect()
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
        drive_db: m.drive_db,
        muted: m.mute,
        solo: m.solo,
        loudness: loudness.get(name).copied(),
        fx: chain(&m.fx),
        bus: m.bus.clone(),
        ..Default::default()
    }
}

/// `t` with the layers of the rack it plays, when `sound` is one
fn with_layers(mut t: TrackView, sound: &claudius::model::Sound) -> TrackView {
    if let claudius::model::Sound::Rack { layers } = sound {
        t.layers = layers.iter().map(claudius::script::layer_value).collect();
    }
    t
}

/// `t` with the sounds its track of `bp` may swap to, the one it plays, and the next
fn with_swaps(mut t: TrackView, bp: &Blueprint) -> TrackView {
    let choice = |c: claudius::swap::Choice| SoundChoice { name: c.name, kind: c.kind, group: c.group.to_string() };
    t.swaps = claudius::swap::choices(bp, &t.name).unwrap_or_default().into_iter().map(choice).collect();
    t.playing = claudius::swap::playing(bp, &t.name);
    t.next_swap = claudius::swap::next(bp, &t.name, &Default::default()).map(|c| c.name);
    t
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
                build_style: s.build_style.name().to_string(),
                build_amount: s.build_amount,
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
        let t = track(&d.name, "drum", &d.name, d.sound.describe(), &d.mix, loudness);
        tracks.push(with_layers(with_swaps(t, bp), &d.sound));
    }
    for p in &bp.parts {
        let t = track(&p.name, "part", p.role.name(), p.sound.describe(), &p.mix, loudness);
        tracks.push(with_layers(with_swaps(t, bp), &p.sound));
    }
    for l in &bp.loops {
        let take = match (&l.from, &l.sound) {
            (Some(from), Sound::Loop { beats, .. }) => {
                Some(format!("a take of {:.1} bars from the start of {from}", beats / 4.0))
            }
            _ => None,
        };
        let (kind, sound) = match take {
            Some(words) => ("take", words),
            None => ("loop", l.sound.describe()),
        };
        let mut t = track(&l.name, kind, &l.role, sound, &l.mix, loudness);
        if let (Some(_), Sound::Loop { beats, bpm, .. }) = (&l.from, &l.sound) {
            t.seconds = Some(beats * 60.0 / bpm.max(1.0));
        }
        tracks.push(t);
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
        // a pattern of one sound shows that sound's layers, one of several each racked sound's under its label
        if let [one] = p.sounds.values().collect::<Vec<_>>().as_slice() {
            t = with_layers(t, one);
        } else {
            t.label_layers = p
                .sounds
                .iter()
                .filter_map(|(label, s)| match s {
                    Sound::Rack { layers } => {
                        Some((label.clone(), layers.iter().map(claudius::script::layer_value).collect()))
                    }
                    _ => None,
                })
                .collect();
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
            trigs: d
                .patterns
                .iter()
                .map(|(v, hits)| {
                    let on_steps = hits.iter().filter_map(|h| {
                        let step = h.t * 4.0;
                        let tr = h.trig.filter(|_| (step - step.round()).abs() < 1e-6)?;
                        // the engine's trig and the view are the same JSON
                        let view = serde_json::to_value(tr).ok().and_then(|j| serde_json::from_value(j).ok())?;
                        Some((step.round() as usize, view))
                    });
                    (v.clone(), on_steps.collect())
                })
                .filter(|(_, t): &(String, Vec<(usize, claudius_api::TrigView)>)| !t.is_empty())
                .collect(),
            length: d.length,
            speed: d.speed,
            swing: d.swing,
            groove: d.groove.clone(),
            groove_amount: d.groove_amount,
            late_ms: d.late_ms,
        })
        .collect();
    let available = palette::instruments();
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
            // every installed instrument that can play the part, from every pack free to publish with
            if let Sound::Instrument { name, .. } = &p.sound {
                offer(name);
            }
            for i in available.iter().filter(|i| palette::suits(i, p.role, p.register)) {
                offer(&i.name);
            }
            PartView {
                name: p.name.clone(),
                role: p.role.name().to_string(),
                style: p.style.clone(),
                styles,
                preset: match &p.sound {
                    Sound::Synth { preset, .. } => preset.clone(),
                    Sound::Instrument { name, .. } => name.clone(),
                    other => other.describe(),
                },
                presets: presets_for,
                register: p.register,
                density: p.density,
                late_ms: p.late_ms,
                cycles: compose::cycles(bp, p)
                    .into_iter()
                    .map(|(prog, notes, beats, written)| CycleView {
                        prog,
                        beats,
                        written,
                        notes: notes.iter().map(|n| NoteView { t: n.t, d: n.d, p: n.p, v: n.v }).collect(),
                    })
                    .collect(),
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
        grooves: bp.grooves.keys().cloned().collect(),
        parts,
        kits: if bp.drums.is_empty() { Vec::new() } else { claudius::swap::kits(bp) },
        kit: claudius::swap::kit_of(bp),
        next_kit: claudius::swap::next_kit(bp),
        mix: claudius_api::MixView {
            buses: bp.mix.buses.iter().map(|(name, fx)| (name.clone(), chain(fx))).collect(),
            master: chain(&bp.mix.master.fx),
            lufs: bp.mix.master.lufs,
            ceiling_db: bp.mix.master.ceiling_db,
            reverb: serde_json::to_value(&bp.mix.reverb).unwrap_or_default(),
            delay: serde_json::to_value(&bp.mix.delay).unwrap_or_default(),
        },
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

/// the verdict `v` of the song `bp`, with the quick fixes its issues have
pub fn verdict(v: &Verdict, bp: &Blueprint) -> VerdictView {
    let targets: Vec<&str> = v.issues.iter().map(|i| i.target.as_str()).collect();
    let fixes = claudius::fixes::of(&targets, &v.measures, bp)
        .into_iter()
        .map(|f| claudius_api::QuickFixView { target: f.target, label: f.label })
        .collect();
    VerdictView {
        total: v.total,
        summary: v.summary.clone(),
        scores: v.scores.iter().map(|(k, x)| (k.clone(), *x)).collect(),
        issues: v.issues.iter().map(|i| (i.severity, i.what.clone(), i.target.clone())).collect(),
        ears: v.ears.as_ref().map(ears),
        measures: v.measures.iter().map(|(k, x)| (k.clone(), *x)).collect(),
        fixes,
    }
}

/// peak envelope of the master, `n` points 0..1
pub fn waveform(r: &Rendered, n: usize) -> Vec<f32> {
    waveform_of(&r.master, n)
}

/// peak envelope of `master`, `n` points 0..1
pub fn waveform_of(master: &claudius::audio::Stereo, n: usize) -> Vec<f32> {
    let len = master.l.len();
    if len == 0 {
        return Vec::new();
    }
    let step = (len / n).max(1);
    (0..len.div_ceil(step))
        .map(|i| {
            let (a, b) = (i * step, ((i + 1) * step).min(len));
            master.l[a..b]
                .iter()
                .zip(&master.r[a..b])
                .map(|(l, r)| l.abs().max(r.abs()))
                .fold(0.0f32, f32::max)
                .min(1.0)
        })
        .collect()
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
            default: p.default,
            opt_in: p.opt_in,
        })
        .collect();
    let kit = claudius_packs::instruments().into_iter().filter(|i| i.kind == "drum").map(|i| i.name).collect();
    SoundList {
        drums: script::drum_names().into_iter().map(String::from).collect(),
        synths: presets::names().into_iter().map(String::from).collect(),
        instruments,
        packs,
        kit,
        synth_roles: presets::ROLES
            .iter()
            .map(|(name, roles)| (name.to_string(), roles.iter().map(|r| r.to_string()).collect()))
            .collect(),
        about: presets::names()
            .into_iter()
            .filter_map(|n| presets::about(n).map(|a| (n, a)))
            .chain(script::drum_names().into_iter().filter_map(|n| script::drum_about(n).map(|a| (n, a))))
            .map(|(n, a)| (n.to_string(), a.to_string()))
            .collect(),
        styles: claudius::model::Role::ALL
            .into_iter()
            .map(|role| {
                let styles = compose::styles_for(role)
                    .into_iter()
                    .filter_map(|s| compose::style_about(role, s).map(|a| (s.to_string(), a.to_string())))
                    .collect();
                (role.name().to_string(), styles)
            })
            .collect(),
    }
}

/// the song in a few lines, for people and assistants
/// `items` as a sentence lists them: "a, b and c"
fn listed(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// "a" or "an", as the word after it is said: "an intro", "a UK drill song", "an IDM song", "an R&B song"
fn a(word: &str) -> &'static str {
    let first = word.split(' ').next().unwrap_or(word);
    // a word in capitals is said letter by letter, and the letters said with a vowel first take "an"
    let letters = first.len() > 1 && first.chars().all(|c| c.is_ascii_uppercase() || c == '&');
    let an = if letters {
        first.starts_with(['A', 'E', 'F', 'H', 'I', 'L', 'M', 'N', 'O', 'R', 'S', 'X'])
    } else {
        first.starts_with(['a', 'e', 'i', 'o', 'u', 'A', 'E', 'I', 'O', 'U'])
    };
    if an { "an" } else { "a" }
}

/// a genre as people write it: "deep house", "UK drill", "IDM", "R&B", "drum and bass", "trip hop fused with IDM"
fn genre_in_words(genre: &str) -> String {
    genre.split('+').map(claudius_api::words::name_in_words).collect::<Vec<_>>().join(" fused with ")
}

/// The song in a few sentences for a person, in the words the studio uses rather than a producer's: what it is, how
/// it goes, what plays in it, and what the critic makes of it. An assistant can pass it on as it is.
pub fn in_words(s: &SongView, v: Option<&VerdictView>) -> String {
    let genre = genre_in_words(&s.genre);
    // from the whole seconds, so that 3:59.6 is 4:00 rather than 3:60
    let whole = s.seconds.max(0.0).round() as u32;
    let (m, sec) = (whole / 60, whole % 60);
    let mut out = format!(
        "\"{}\" is {} {genre} song, {m}:{sec:02} long, at {:.0} beats a minute in {}.",
        s.title,
        a(&genre),
        s.bpm,
        s.key
    );
    let mut seen: Vec<&str> = Vec::new();
    let form: Vec<String> = s
        .sections
        .iter()
        .map(|x| {
            let kind = x.kind.as_str();
            let again = seen.contains(&kind);
            seen.push(kind);
            if again { format!("another {kind}") } else { format!("{} {kind}", a(kind)) }
        })
        .collect();
    if !form.is_empty() {
        out.push_str(&format!(" It goes {}.", listed(&form)));
    }
    let named = |pick: &dyn Fn(&TrackView) -> bool| -> Vec<String> {
        s.tracks.iter().filter(|t| !t.muted && pick(t)).map(|t| t.name.replace('_', " ")).collect()
    };
    let drums = named(&|t| t.kind == "drum");
    let rest = named(&|t| t.kind != "drum");
    match (drums.is_empty(), rest.is_empty()) {
        (false, false) => {
            out.push_str(&format!(" The drums are {}; over them play {}.", listed(&drums), listed(&rest)))
        }
        (false, true) => out.push_str(&format!(" Only drums play: {}.", listed(&drums))),
        (true, false) => out.push_str(&format!(" It plays {}, without drums.", listed(&rest))),
        (true, true) => {}
    }
    if let Some(v) = v {
        out.push_str(&format!(" The critic gives it {:.0} out of 100", v.total));
        match v.issues.first() {
            None => out.push_str(", with nothing to fix."),
            Some((severity, what, target)) => {
                let more = match v.issues.len() - 1 {
                    0 => String::new(),
                    1 => " (and one thing more)".to_string(),
                    n => format!(" (and {n} things more)"),
                };
                out.push_str(&format!(
                    ". What to fix first, {}: {what}{more}; the fix is {}.",
                    severity_in_words(*severity),
                    fix_in_words(target)
                ));
            }
        }
    }
    out
}

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
            "  {} [{} {}] {}; level {:+.1} dB, pan {:+.2}, reverb {:.2}, delay {:.2}, duck {:.2}{}{}{}\n",
            t.name,
            t.kind,
            t.role,
            t.sound,
            t.level_db,
            t.pan,
            t.reverb,
            t.delay,
            t.duck,
            if t.drive_db > 0.0 { format!(", drive {:.0} dB", t.drive_db) } else { String::new() },
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
    let measures: std::collections::BTreeMap<String, f64> = v.measures.iter().cloned().collect();
    if let [Some(ce), Some(cu), Some(pc), Some(pq)] = ["aes_ce", "aes_cu", "aes_pc", "aes_pq"].map(|k| measures.get(k))
    {
        out.push_str(&format!(
            "  aesthetics (of 10): enjoyment {ce:.1}, production quality {pq:.1}; held out: usefulness {cu:.1}, \
             complexity {pc:.1}\n"
        ));
    }
    let sections = claudius::judge::section_loudness(&measures);
    if !sections.is_empty() {
        let line: Vec<String> = sections.iter().map(|(name, lufs)| format!("{name} {lufs:.1}")).collect();
        out.push_str(&format!("  loudness by section (LUFS): {}\n", line.join(", ")));
    }
    for (sev, what, target) in v.issues.iter().take(8) {
        out.push_str(&format!("  issue ({sev:.2}): {what} — fix: {target}\n"));
    }
    for f in &v.fixes {
        out.push_str(&format!("  quick fix ({}): {}\n", f.target, f.label));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_song_in_words_says_what_it_is_how_it_goes_what_plays_and_what_to_fix() {
        let t = |name: &str, kind: &str, muted: bool| TrackView {
            name: name.into(),
            kind: kind.into(),
            muted,
            ..Default::default()
        };
        let sec = |kind: &str| SectionView { kind: kind.into(), ..Default::default() };
        let song = SongView {
            title: "Faded Engine".into(),
            genre: "deep_house".into(),
            bpm: 118.0,
            key: "E major".into(),
            seconds: 244.0,
            sections: vec![sec("intro"), sec("groove"), sec("drop"), sec("drop"), sec("outro")],
            tracks: vec![
                t("kick", "drum", false),
                t("hat", "drum", false),
                t("bass", "part", false),
                t("lead", "part", true),
            ],
            ..Default::default()
        };
        let verdict = VerdictView {
            total: 91.4,
            issues: vec![(0.1, "dull 4-8 kHz".into(), "band:presence-".into()), (0.05, "x".into(), "form".into())],
            ..Default::default()
        };
        let words = in_words(&song, Some(&verdict));
        assert!(
            words.starts_with("\"Faded Engine\" is a deep house song, 4:04 long, at 118 beats a minute in E major."),
            "{words}"
        );
        assert!(words.contains("It goes an intro, a groove, a drop, another drop and an outro."), "{words}");
        assert!(
            words.contains("The drums are kick and hat; over them play bass."),
            "the muted lead is left out: {words}"
        );
        assert!(
            words.contains("gives it 91 out of 100. What to fix first, minor: dull 4-8 kHz (and one thing more)"),
            "{words}"
        );
        assert!(words.ends_with("the fix is more presence in the mix."), "{words}");
        assert!(!in_words(&song, None).contains("critic"));
        let long = SongView { seconds: 239.6, ..song.clone() };
        assert!(in_words(&long, None).contains("4:00 long"), "{}", in_words(&long, None));
    }

    #[test]
    fn a_genre_reads_as_people_write_it_with_its_article_as_it_is_said() {
        let said = |g: &str| {
            let w = genre_in_words(g);
            format!("{} {w}", a(&w))
        };
        assert_eq!(said("uk_drill"), "a UK drill");
        assert_eq!(said("idm"), "an IDM");
        assert_eq!(said("rnb_90s"), "an R&B 90s");
        assert_eq!(said("ny_drill"), "an NY drill");
        assert_eq!(said("dnb"), "a drum and bass");
        assert_eq!(said("ambient"), "an ambient");
        assert_eq!(said("trip_hop+idm"), "a trip hop fused with IDM");
        assert_eq!(a("intro"), "an");
        assert_eq!(a("outro"), "an");
        assert_eq!(a("drop"), "a");
    }

    #[test]
    fn every_sound_and_style_a_part_may_take_says_what_it_is() {
        let list = sounds();
        for name in list.synths.iter().chain(&list.drums) {
            assert!(list.sound_about(name).is_some_and(|a| !a.is_empty()), "{name} says nothing of itself");
        }
        assert_eq!(list.sound_about("tr909:bd"), Some("kick"), "a kit's drum is its voice");
        assert_eq!(list.sound_about("vsco2:strings"), None, "an instrument is not the preset of its name");
        for role in claudius::model::Role::ALL {
            for style in compose::styles_for(role) {
                assert!(list.style_about(role.name(), style).is_some(), "{}: {style} says nothing", role.name());
            }
        }
        assert_ne!(list.style_about("bass", "sustain"), list.style_about("chords", "sustain"), "by role");
    }
}
