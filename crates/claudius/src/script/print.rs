//! Printing a song as a script that runs back into the same song.

use crate::model::*;
use crate::pattern::quote;

use super::eval_pattern;
use super::form::default_energy;
use super::song::pattern_mix;
use super::sounds::{drum_label, drum_name, voice_word};
use super::steps::{hits_to_steps, shortest_steps};
use crate::synth::drums::DrumParams;

/// a number as the shortest text that reads back as the same f32
fn f32s(v: f32) -> String {
    if v == v.trunc() && v.abs() < 1e9 { format!("{}", v as i64) } else { format!("{v}") }
}

fn f64s(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 { format!("{}", v as i64) } else { format!("{v}") }
}

/// the mix settings that differ from what the track would get without a mix() line
fn mix_line(name: &str, m: &TrackMix, base: &TrackMix, out: &mut String) {
    let mut parts = Vec::new();
    for (k, v, b) in [
        ("level", m.level_db, base.level_db),
        ("pan", m.pan, base.pan),
        ("reverb", m.reverb, base.reverb),
        ("delay", m.delay, base.delay),
        ("duck", m.duck, base.duck),
    ] {
        if v != b {
            parts.push(format!("{k}: {}", f32s(v)));
        }
    }
    if m.mute != base.mute {
        parts.push(format!("mute: {}", m.mute));
    }
    if m.solo != base.solo {
        parts.push(format!("solo: {}", m.solo));
    }
    if !parts.is_empty() {
        out.push_str(&format!("mix({}, #{{ {} }});\n", quote(name), parts.join(", ")));
    }
}

/// the song as a script; running it on the same song changes nothing
pub fn to_script(bp: &Blueprint) -> String {
    let mut s = String::new();
    s.push_str(&format!("// {} — {} (seed {})\n", bp.title, bp.genre, bp.seed));
    if bp.lineage.first().is_some_and(|l| *l == format!("blank {} song", bp.genre)) {
        s.push_str(&format!(
            "// blank({});   // uncomment to build this song from nothing (as it is, the script edits the song open)\n",
            quote(&bp.genre)
        ));
    } else {
        s.push_str(&format!(
            "// song({}, {});   // uncomment to start over from the generator\n",
            quote(&bp.genre),
            bp.seed as i64
        ));
    }
    s.push_str(&format!(
        "title({}); tempo({}); key({}); swing({});\n",
        quote(&bp.title),
        f64s(bp.bpm),
        quote(&bp.key.name()),
        f32s(bp.swing)
    ));
    if bp.humanize != 0.0 {
        s.push_str(&format!("humanize({});\n", f32s(bp.humanize)));
    }
    for (k, v) in &bp.harmony.progressions {
        s.push_str(&format!("prog({}, {});\n", quote(k), quote(&v.join(" "))));
    }
    s.push_str(&format!("bars_per_chord({});\n", bp.harmony.bars_per_chord));
    s.push_str(&format!(
        "master(#{{ lufs: {}, ceiling: {} }});\n",
        f32s(bp.mix.master.lufs),
        f32s(bp.mix.master.ceiling_db)
    ));
    if !bp.drums.is_empty() {
        s.push_str(
            "\n// drums: X accent, x hit, o ghost, . rest, | bar line; 16 steps per bar unless a number follows\n",
        );
    }
    for d in &bp.drums {
        for (v, hits) in &d.patterns {
            let (steps, res) = hits_to_steps(hits, d.pattern_bars);
            let steps = shortest_steps(&steps);
            if res == 16 {
                s.push_str(&format!("drum({}, {}, {});\n", quote(&d.name), quote(v), quote(&steps)));
            } else {
                s.push_str(&format!("drum({}, {}, {}, {res});\n", quote(&d.name), quote(v), quote(&steps)));
            }
        }
        match &d.sound {
            Sound::Drum { voice, params } => {
                // what drum() makes for this name needs no line; the generator's own settings have no word to print
                let made =
                    drum_name(&d.name).is_some_and(|(v, _)| v == *voice && *params == DrumParams::default_for(v));
                let label = drum_label(*voice, params).or_else(|| {
                    drum_name(&d.name).is_none_or(|(v, _)| v != *voice).then(|| voice_word(*voice).to_string())
                });
                if let Some(label) = label.filter(|_| !made) {
                    s.push_str(&format!("drum_sound({}, {});\n", quote(&d.name), quote(&label)));
                }
            }
            Sound::Instrument { name, .. } => {
                s.push_str(&format!("drum_sound({}, {});\n", quote(&d.name), quote(name)));
            }
            // a one-shot cut from your library lives in claudius' cache: its path means nothing on another machine
            Sound::Sample { path, label, .. } if std::path::Path::new(path).starts_with(crate::config::home()) => {
                s.push_str(&format!("// {}: {label}\n", d.name));
            }
            Sound::Sample { path, .. } => s.push_str(&format!("drum_sound({}, {});\n", quote(&d.name), quote(path))),
            _ => {}
        }
    }
    if !bp.parts.is_empty() {
        s.push_str("\n// parts written by the theory engine: change the style, preset, register, density or seed\n");
    }
    for p in &bp.parts {
        let preset = match &p.sound {
            Sound::Synth { preset, .. } => format!(", preset: {}", quote(preset)),
            Sound::Instrument { name, .. } => format!(", preset: {}", quote(name)),
            _ => String::new(),
        };
        let rate = if p.role == Role::Arp { format!(", rate: {}", f64s(p.rate)) } else { String::new() };
        s.push_str(&format!(
            "part({}, #{{ style: {}{preset}, register: [{}, {}], density: {}{rate}, seed: {} }});\n",
            quote(&p.name),
            quote(&p.style),
            p.register.0,
            p.register.1,
            f32s(p.density),
            p.seed as i64
        ));
    }
    if !bp.patterns.is_empty() {
        s.push_str("\n// pattern tracks (mini-notation, Strudel style)\n");
    }
    for p in &bp.patterns {
        s.push_str(&format!("track({}, {});\n", quote(&p.name), p.code));
    }
    s.push_str("\nform([\n");
    let all = bp.layer_names();
    for sec in &bp.form {
        let layers = if sec.layers == all { quote("*") } else { quote(&sec.layers.join(" ")) };
        let mut line = format!("  sec({}, {}, {layers})", quote(sec.kind.name()), sec.bars);
        if sec.name != sec.kind.name() {
            line.push_str(&format!(".name({})", quote(&sec.name)));
        }
        if sec.energy != default_energy(sec.kind) {
            line.push_str(&format!(".energy({})", f32s(sec.energy)));
        }
        let default_prog = if sec.kind == SectionKind::Break { "B" } else { "A" };
        if sec.prog != default_prog {
            line.push_str(&format!(".prog({})", quote(&sec.prog)));
        }
        let default_variant =
            if matches!(sec.kind, SectionKind::Intro | SectionKind::Outro) { "sparse" } else { "main" };
        if sec.variant != default_variant {
            line.push_str(&format!(".variant({})", quote(&sec.variant)));
        }
        let defaults = (
            sec.kind == SectionKind::Build,
            matches!(sec.kind, SectionKind::Drop | SectionKind::Groove | SectionKind::Hook),
        );
        if sec.build != defaults.0 {
            line.push_str(&format!(".build({})", sec.build));
        }
        if sec.fills != defaults.1 {
            line.push_str(&format!(".fills({})", sec.fills));
        }
        for (flag, name) in [(sec.gap, "gap"), (sec.impact, "impact"), (sec.filter_in, "filter_in")] {
            if flag {
                line.push_str(&format!(".{name}()"));
            }
        }
        s.push_str(&line);
        s.push_str(",\n");
    }
    s.push_str("]);\n\n");
    let plain = TrackMix::default();
    for d in &bp.drums {
        mix_line(&d.name, &d.mix, &plain, &mut s);
    }
    for p in &bp.parts {
        mix_line(&p.name, &p.mix, &plain, &mut s);
    }
    for l in &bp.loops {
        mix_line(&l.name, &l.mix, &plain, &mut s);
    }
    for p in &bp.patterns {
        let labels: Vec<String> = p.sounds.keys().cloned().collect();
        let base = eval_pattern(&p.code).map(|pat| pattern_mix(&pat.opts, &labels, &p.role)).unwrap_or_default();
        mix_line(&p.name, &p.mix, &base, &mut s);
    }
    mix_line("fx", &bp.transitions.mix, &plain, &mut s);
    s
}
