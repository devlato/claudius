//! Printing a song as a script that runs back into the same song.

use crate::fx::Effect;
use crate::model::*;
use crate::pattern::quote;

use super::eval_pattern;
use super::form::default_energy;
use super::song::pattern_mix;
use super::sounds::{drum_label, drum_name, voice_word};
use super::steps::{hits_to_steps, shortest_steps};
use crate::synth::drums::DrumParams;

/// a number as the shortest text that reads back as the same f32
pub(super) fn f32s(v: f32) -> String {
    if v == v.trunc() && v.abs() < 1e9 { format!("{}", v as i64) } else { format!("{v}") }
}

fn f64s(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 { format!("{}", v as i64) } else { format!("{v}") }
}

/// a setting as a script writes it: whole numbers whole, the rest to four places at most
pub(super) fn number(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v:.4}").trim_end_matches('0').trim_end_matches('.').to_string()
    }
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
    if m.bus != base.bus {
        parts.push(format!("bus: {}", quote(&m.bus)));
    }
    if !parts.is_empty() {
        out.push_str(&format!("mix({}, #{{ {} }});\n", quote(name), parts.join(", ")));
    }
}

/// a map key as a script writes it: bare when it is a name, quoted when it is not or when Rhai keeps the word
fn key_text(k: &str) -> String {
    const KEYWORDS: &[&str] = &[
        "as",
        "break",
        "call",
        "case",
        "catch",
        "const",
        "continue",
        "curry",
        "debug",
        "default",
        "do",
        "else",
        "eval",
        "exit",
        "export",
        "false",
        "fn",
        "for",
        "global",
        "go",
        "goto",
        "if",
        "import",
        "in",
        "is_def_fn",
        "is_def_var",
        "is_shared",
        "let",
        "loop",
        "match",
        "module",
        "new",
        "nil",
        "null",
        "package",
        "print",
        "private",
        "protected",
        "public",
        "return",
        "shared",
        "spawn",
        "static",
        "super",
        "switch",
        "sync",
        "this",
        "thread",
        "throw",
        "true",
        "try",
        "type_of",
        "until",
        "use",
        "var",
        "void",
        "while",
        "with",
    ];
    let bare = k.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !KEYWORDS.contains(&k);
    if bare { k.to_string() } else { quote(k) }
}

/// A value as a script writes it: a number as the shortest text that reads back the same, an effect's `fx` first,
/// and values by section as the `sections()` that makes them.
fn value_text(v: &serde_json::Value) -> String {
    use serde_json::Value;
    match v {
        Value::Null => "()".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => i.to_string(),
            (_, Some(x)) if (x as f32) as f64 == x => f32s(x as f32),
            (_, Some(x)) => f64s(x),
            _ => n.to_string(),
        },
        Value::String(s) => quote(s),
        Value::Array(a) => format!("[{}]", a.iter().map(value_text).collect::<Vec<_>>().join(", ")),
        Value::Object(m) => {
            if let (Some(rest), Some(Value::Object(by)), 2) = (m.get("rest"), m.get("sections"), m.len()) {
                let one = |v: &Value| match v {
                    Value::Array(a) if a.len() == 1 => value_text(&a[0]),
                    v => value_text(v),
                };
                let by: Vec<String> = by.iter().map(|(k, v)| format!("{}: {}", key_text(k), one(v))).collect();
                return format!("sections({}, #{{ {} }})", value_text(rest), by.join(", "));
            }
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort_by_key(|k| (!(k.as_str() == "fx" && m[k.as_str()].is_string()), k.as_str()));
            let fields: Vec<String> =
                keys.iter().map(|k| format!("{}: {}", key_text(k), value_text(&m[k.as_str()]))).collect();
            format!("#{{ {} }}", fields.join(", "))
        }
    }
}

/// a chain of effects as a script writes it, an effect a line when there are several
fn chain_text(chain: &[Effect]) -> String {
    let fx: Vec<String> = chain.iter().map(|e| value_text(&serde_json::to_value(e).unwrap_or_default())).collect();
    match fx.as_slice() {
        [] => "[]".into(),
        [one] => format!("[{one}]"),
        many => format!("[\n{}]", many.iter().map(|f| format!("  {f},\n")).collect::<String>()),
    }
}

/// The effect chains: each track's own, the buses', the master's and the sends'. A chain left empty prints too where
/// the genre (or the generator, for a chorus on a part that plays chords) would give one, so that the script rebuilds
/// the song as it is.
fn effects(bp: &Blueprint, out: &mut String) {
    let g = crate::genres::get(&bp.genre).ok();
    let mut lines = Vec::new();
    let mut track = |name: &str, chain: &[Effect], base: &[Effect], given: bool| {
        if chain != base || (chain.is_empty() && given) {
            lines.push(format!("fx({}, {});", quote(name), chain_text(chain)));
        }
    };
    let gets = |fx: Option<&TrackMix>| fx.is_some_and(|m| !m.fx.is_empty());
    for d in &bp.drums {
        track(
            &d.name,
            &d.mix.fx,
            &[],
            gets(g.as_ref().and_then(|g| g.drums.iter().find(|l| l.name == d.name)).map(|l| &l.mix)),
        );
    }
    for p in &bp.parts {
        let spec = g.as_ref().and_then(|g| g.parts.iter().find(|s| s.name == p.name));
        let chorus = matches!(p.role, Role::Chords | Role::Keys | Role::Pad);
        track(&p.name, &p.mix.fx, &[], chorus || gets(spec.map(|s| &s.mix)));
    }
    for l in &bp.loops {
        track(
            &l.name,
            &l.mix.fx,
            &[],
            gets(g.as_ref().and_then(|g| g.loops.iter().find(|s| s.name == l.name)).map(|s| &s.mix)),
        );
    }
    for p in &bp.patterns {
        let labels: Vec<String> = p.sounds.keys().cloned().collect();
        let base = eval_pattern(&p.code).map(|pat| pattern_mix(&pat.opts, &labels, &p.role).fx).unwrap_or_default();
        track(&p.name, &p.mix.fx, &base, false);
    }
    track("transitions", &bp.transitions.mix.fx, &[], gets(g.as_ref().map(|g| &g.transitions.mix)));
    for (name, chain) in &bp.mix.buses {
        let given = g.as_ref().and_then(|g| g.mix.buses.get(name)).is_some_and(|c| !c.is_empty());
        if !chain.is_empty() || given {
            lines.push(format!("bus({}, {});", quote(name), chain_text(chain)));
        }
    }
    let master = g.as_ref().is_some_and(|g| !g.mix.master.is_empty());
    if !bp.mix.master.fx.is_empty() || master {
        lines.push(format!("master(#{{ fx: {} }});", chain_text(&bp.mix.master.fx)));
    }
    let mut sends = Vec::new();
    for (k, e, given) in [
        ("reverb", &bp.mix.reverb, g.as_ref().map(|g| &g.mix.reverb)),
        ("delay", &bp.mix.delay, g.as_ref().map(|g| &g.mix.delay)),
    ] {
        if given != Some(e) {
            sends.push(format!("{k}: {}", value_text(&serde_json::to_value(e).unwrap_or_default())));
        }
    }
    if !sends.is_empty() {
        lines.push(format!("send_fx(#{{ {} }});", sends.join(", ")));
    }
    if !lines.is_empty() {
        out.push_str(
            "\n// effects: each track's own chain, then the buses', the master's and the sends'; a setting that\n",
        );
        out.push_str(
            "// can move is a number, a curve ([[beat, value], ...]) or sections(rest, #{ section: value })\n",
        );
        for l in lines {
            out.push_str(&l);
            out.push('\n');
        }
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
    for line in super::trigs::groove_lines(bp) {
        s.push_str(&line);
        s.push('\n');
    }
    for d in &bp.drums {
        let sound = match &d.sound {
            Sound::Drum { voice, params } => {
                // what drum() makes for this name needs no line; the generator's own settings have no word to print
                let made =
                    drum_name(&d.name).is_some_and(|(v, _)| v == *voice && *params == DrumParams::default_for(v));
                let label = drum_label(*voice, params).or_else(|| {
                    drum_name(&d.name).is_none_or(|(v, _)| v != *voice).then(|| voice_word(*voice).to_string())
                });
                label.filter(|_| !made).map(|label| format!("drum_sound({}, {});\n", quote(&d.name), quote(&label)))
            }
            Sound::Instrument { name, .. } => Some(format!("drum_sound({}, {});\n", quote(&d.name), quote(name))),
            // a one-shot cut from your library lives in claudius' cache: its path means nothing on another machine
            Sound::Sample { path, label, .. } if std::path::Path::new(path).starts_with(crate::config::home()) => {
                Some(format!("// {}: {label}\n", d.name))
            }
            Sound::Sample { path, .. } => Some(format!("drum_sound({}, {});\n", quote(&d.name), quote(path))),
            _ => None,
        };
        // drum() makes a lane only for a drum's name: a lane of another name is made by its sound, first
        let named = drum_name(&d.name).is_some() || d.name == "hats";
        if !named && let Some(line) = &sound {
            s.push_str(line);
        }
        for (v, hits) in &d.patterns {
            let (steps, res) = hits_to_steps(hits, d.pattern_bars);
            let steps = shortest_steps(&steps);
            if res == 16 {
                s.push_str(&format!("drum({}, {}, {});\n", quote(&d.name), quote(v), quote(&steps)));
            } else {
                s.push_str(&format!("drum({}, {}, {}, {res});\n", quote(&d.name), quote(v), quote(&steps)));
            }
            for line in super::trigs::lines(&d.name, v, hits) {
                s.push_str(&line);
                s.push('\n');
            }
        }
        if let Some(line) = super::trigs::lane_line(d) {
            s.push_str(&line);
            s.push('\n');
        }
        if named && let Some(line) = &sound {
            s.push_str(line);
        }
    }
    if !bp.parts.is_empty() {
        s.push_str("\n// parts written by the theory engine: change the style, preset, register, density or seed\n");
    }
    let genre = super::song::genre_of(bp);
    for p in &bp.parts {
        let preset = match &p.sound {
            Sound::Synth { preset, .. } => format!(", preset: {}", quote(preset)),
            Sound::Instrument { name, .. } => format!(", preset: {}", quote(name)),
            _ => String::new(),
        };
        let rate = if p.role == Role::Arp { format!(", rate: {}", f64s(p.rate)) } else { String::new() };
        // a part of your own says what it is; the genre's own are known by name
        let own = genre.as_ref().is_some_and(|g| g.parts.iter().any(|q| q.name == p.name && q.role == p.role));
        let role = if own { String::new() } else { format!("role: {}, ", quote(p.role.name())) };
        s.push_str(&format!(
            "part({}, #{{ {role}style: {}{preset}, register: [{}, {}], density: {}{rate}, seed: {} }});\n",
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
    // a track starts on the bus its genre gives it, or the one a new track of its kind gets
    let g = crate::genres::get(&bp.genre).ok();
    let on = |bus: Option<&TrackMix>, or: &str| TrackMix {
        bus: bus.map_or_else(|| or.to_string(), |m| m.bus.clone()),
        ..TrackMix::default()
    };
    for d in &bp.drums {
        let spec = g.as_ref().and_then(|g| g.drums.iter().find(|l| l.name == d.name)).map(|l| &l.mix);
        mix_line(&d.name, &d.mix, &on(spec, "drums"), &mut s);
    }
    for p in &bp.parts {
        let spec = g.as_ref().and_then(|g| g.parts.iter().find(|s| s.name == p.name)).map(|s| &s.mix);
        mix_line(&p.name, &p.mix, &on(spec, "music"), &mut s);
    }
    for l in &bp.loops {
        let spec = g.as_ref().and_then(|g| g.loops.iter().find(|s| s.name == l.name)).map(|s| &s.mix);
        mix_line(&l.name, &l.mix, &on(spec, "music"), &mut s);
    }
    for p in &bp.patterns {
        let labels: Vec<String> = p.sounds.keys().cloned().collect();
        let base = eval_pattern(&p.code).map(|pat| pattern_mix(&pat.opts, &labels, &p.role)).unwrap_or_default();
        mix_line(&p.name, &p.mix, &base, &mut s);
    }
    mix_line("fx", &bp.transitions.mix, &on(g.as_ref().map(|g| &g.transitions.mix), "fx"), &mut s);
    effects(bp, &mut s);
    s
}
