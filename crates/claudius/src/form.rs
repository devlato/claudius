//! A form of its own for every song. A genre's `forms` are templates; a generated song plays one of them varied, each
//! change drawn with its own chance:
//!
//! - its sections lengthened or shortened, to the bar counts a section of that kind takes;
//! - a third verse and hook after the last hook;
//! - a bridge before the last hook or drop: the song's melodic parts alone, on its second progression;
//! - the last hook or drop played twice;
//! - a lift, a four-bar build, into the first hook that has none;
//! - the intro left out (a cold start) or stripped to its drums or to its melodic parts;
//! - the outro left out (a cold ending) or thinned to half its parts;
//! - a first verse without one of its parts, a later verse with one more from the hook, a first hook without one part
//!   the later ones play;
//! - fills, gaps before a drop and the intro's filter toggled now and then, and each section's energy nudged.
//!
//! The song then keeps to two to five and a half minutes at its tempo, and every section has a name of its own.

use crate::genres::GenreProfile;
use crate::model::{SectionKind, SectionSpec};
use crate::rng::Rng;

/// the bar counts a section of `kind` takes
fn lengths(kind: SectionKind) -> &'static [u32] {
    match kind {
        SectionKind::Intro => &[4, 8, 16],
        SectionKind::Verse => &[8, 16],
        SectionKind::Build => &[4, 8],
        SectionKind::Drop => &[16, 24, 32],
        SectionKind::Break => &[4, 8, 16],
        SectionKind::Hook => &[8, 16],
        SectionKind::Groove => &[8, 16, 32],
        SectionKind::Outro => &[4, 8, 16],
    }
}

fn is_peak(s: &SectionSpec) -> bool {
    matches!(s.kind, SectionKind::Hook | SectionKind::Drop)
}

/// the drum lanes among a section's layers
fn drums<'a>(s: &'a SectionSpec, g: &GenreProfile) -> Vec<&'a String> {
    s.layers.iter().filter(|l| g.drums.iter().any(|d| d.name == **l)).collect()
}

/// the parts among a section's layers that every song of the genre has (a loop, or a part a song may leave out, is
/// not sure to play)
fn melodic<'a>(s: &'a SectionSpec, g: &GenreProfile) -> Vec<&'a String> {
    s.layers.iter().filter(|l| g.parts.iter().any(|p| p.name == **l && p.optional <= 0.0)).collect()
}

/// whether a layer is sure to play in every song of the genre: a drum lane, or a part never left out
fn sure(layer: &str, g: &GenreProfile) -> bool {
    g.drums.iter().any(|d| d.name == layer) || g.parts.iter().any(|p| p.name == layer && p.optional <= 0.0)
}

/// `template` varied for one song (the module's doc says how), at `bpm`
pub fn vary(template: &[SectionSpec], g: &GenreProfile, bpm: f64, rng: &mut Rng) -> Vec<SectionSpec> {
    let mut form: Vec<SectionSpec> = template.to_vec();
    if form.is_empty() {
        return form;
    }
    // lengths
    for s in form.iter_mut() {
        let options: Vec<u32> = lengths(s.kind).iter().copied().filter(|&b| b != s.bars).collect();
        if !options.is_empty() && rng.chance(0.45) {
            s.bars = options[rng.below(options.len())];
        }
    }
    // a third verse and hook, before the song's first break after a hook (verse, chorus, verse, chorus, bridge keeps
    // its shape), or after its last hook
    let first_hook = form.iter().position(|s| s.kind == SectionKind::Hook);
    let brk = first_hook.and_then(|h| (h..form.len()).find(|&i| form[i].kind == SectionKind::Break));
    let last_hook = match brk {
        Some(b) => form[..b].iter().rposition(|s| s.kind == SectionKind::Hook),
        None => form.iter().rposition(|s| s.kind == SectionKind::Hook),
    };
    let last_verse = last_hook.and_then(|h| form[..h].iter().rposition(|s| s.kind == SectionKind::Verse));
    if let (Some(h), Some(v)) = (last_hook, last_verse)
        && rng.chance(0.3)
    {
        let (verse, hook) = (form[v].clone(), form[h].clone());
        form.insert(h + 1, verse);
        form.insert(h + 2, hook);
    }
    // a bridge before the last peak, when nothing breaks the song between its first peak and its last
    let first = form.iter().position(is_peak);
    let last = form.iter().rposition(is_peak);
    let broken = matches!((first, last), (Some(a), Some(b)) if form[a..b].iter().any(|s| s.kind == SectionKind::Break));
    if let Some(p) = last
        && p > 0
        && !broken
        && rng.chance(0.35)
    {
        let layers: Vec<String> = melodic(&form[p], g).into_iter().cloned().collect();
        if layers.len() >= 2 {
            form.insert(
                p,
                SectionSpec {
                    name: "bridge".into(),
                    kind: SectionKind::Break,
                    bars: 8,
                    energy: 0.45,
                    prog: "B".into(),
                    layers,
                    variant: "sparse".into(),
                    filter_in: false,
                    build: false,
                    gap: rng.chance(0.4),
                    impact: false,
                    fills: false,
                },
            );
        }
    }
    // the last peak twice
    if let Some(p) = form.iter().rposition(is_peak)
        && rng.chance(0.3)
    {
        let again = SectionSpec { energy: 1.0, impact: false, ..form[p].clone() };
        form.insert(p + 1, again);
    }
    // a lift into the first hook that has no build before it
    if let Some(h) = form.iter().position(|s| s.kind == SectionKind::Hook)
        && h > 0
        && form[h - 1].kind != SectionKind::Build
        && rng.chance(0.35)
    {
        let before = &form[h - 1];
        let layers: Vec<String> = before.layers.iter().filter(|l| l.as_str() != "kick").cloned().collect();
        let energy = ((before.energy + form[h].energy) / 2.0).min(0.85);
        let lift = SectionSpec {
            name: "lift".into(),
            kind: SectionKind::Build,
            bars: 4,
            energy,
            layers,
            filter_in: false,
            build: true,
            gap: false,
            impact: false,
            fills: false,
            ..before.clone()
        };
        form.insert(h, lift);
    }
    // the intro: left out, or stripped to its drums or its melodic parts
    if form.len() > 4 && form[0].kind == SectionKind::Intro {
        let x = rng.f64();
        if x < 0.15 {
            form.remove(0);
        } else if x < 0.4 {
            let (d, m): (Vec<String>, Vec<String>) = (
                drums(&form[0], g).into_iter().cloned().collect(),
                melodic(&form[0], g).into_iter().cloned().collect(),
            );
            let keep = if rng.chance(0.5) { d } else { m };
            if !keep.is_empty() {
                form[0].layers = keep;
            }
        }
    }
    // the outro: left out (the song ends on its last section), or thinned
    if form.len() > 4 && form.last().is_some_and(|s| s.kind == SectionKind::Outro) {
        let x = rng.f64();
        if x < 0.15 {
            form.pop();
        } else if x < 0.4
            && let Some(o) = form.last_mut()
            && o.layers.len() > 2
        {
            let keep = o.layers.len().div_ceil(2);
            let mut layers = o.layers.clone();
            while layers.len() > keep {
                layers.remove(rng.below(layers.len()));
            }
            // something sure to play stays
            if layers.iter().any(|l| sure(l, g)) {
                o.layers = layers;
            }
        }
    }
    // layers: the first verse thinner, a later one fuller, the first hook without one of the later hooks' parts
    let verses: Vec<usize> = (0..form.len()).filter(|&i| form[i].kind == SectionKind::Verse).collect();
    if let Some(&first) = verses.first() {
        let m: Vec<String> = melodic(&form[first], g).into_iter().cloned().collect();
        if m.len() >= 2 && rng.chance(0.3) {
            let drop = &m[rng.below(m.len())];
            form[first].layers.retain(|l| l != drop);
        }
    }
    for &v in verses.iter().skip(1) {
        if let Some(h) = (v..form.len()).find(|&i| form[i].kind == SectionKind::Hook) {
            let missing: Vec<String> = form[h].layers.iter().filter(|l| !form[v].layers.contains(l)).cloned().collect();
            if !missing.is_empty() && rng.chance(0.35) {
                let add = missing[rng.below(missing.len())].clone();
                form[v].layers.push(add);
            }
        }
    }
    let hooks: Vec<usize> = (0..form.len()).filter(|&i| form[i].kind == SectionKind::Hook).collect();
    if hooks.len() >= 2 && rng.chance(0.25) {
        let m: Vec<String> = melodic(&form[hooks[0]], g).into_iter().cloned().collect();
        if m.len() >= 3 {
            let drop = &m[rng.below(m.len())];
            form[hooks[0]].layers.retain(|l| l != drop);
        }
    }
    // flags and energy
    for i in 0..form.len() {
        let next_peak = form.get(i + 1).is_some_and(is_peak);
        let s = &mut form[i];
        if matches!(s.kind, SectionKind::Verse | SectionKind::Groove | SectionKind::Drop) && rng.chance(0.25) {
            s.fills = !s.fills;
        }
        if next_peak && !s.build && rng.chance(0.3) {
            s.gap = !s.gap;
        }
        if s.kind == SectionKind::Intro && rng.chance(0.4) {
            s.filter_in = !s.filter_in;
        }
        s.energy = (s.energy + rng.range(-0.05, 0.05) as f32).clamp(0.1, 1.0);
    }
    fit(&mut form, bpm);
    name_apart(&mut form);
    form
}

/// keeps the song between two minutes and five and a half at `bpm`: the longest sections shortened, the shortest
/// lengthened, by their kind's bar counts
fn fit(form: &mut [SectionSpec], bpm: f64) {
    let secs = |f: &[SectionSpec]| f.iter().map(|s| s.bars as f64).sum::<f64>() * 240.0 / bpm.max(1.0);
    for _ in 0..64 {
        let t = secs(form);
        if t > 330.0 {
            let Some(i) = (0..form.len())
                .filter(|&i| lengths(form[i].kind).iter().any(|&b| b < form[i].bars))
                .max_by_key(|&i| form[i].bars)
            else {
                return;
            };
            form[i].bars = lengths(form[i].kind).iter().copied().filter(|&b| b < form[i].bars).max().unwrap_or(4);
        } else if t < 120.0 {
            let Some(i) = (0..form.len())
                .filter(|&i| lengths(form[i].kind).iter().any(|&b| b > form[i].bars))
                .min_by_key(|&i| form[i].bars)
            else {
                return;
            };
            form[i].bars = lengths(form[i].kind).iter().copied().filter(|&b| b > form[i].bars).min().unwrap_or(16);
        } else {
            return;
        }
    }
}

/// every section a name of its own: a second "hook" becomes "hook2"
fn name_apart(form: &mut [SectionSpec]) {
    let mut seen: Vec<String> = Vec::new();
    for s in form.iter_mut() {
        let base = s.name.trim_end_matches(|c: char| c.is_ascii_digit()).to_string();
        let mut name = s.name.clone();
        let mut n = 2;
        while seen.contains(&name) {
            name = format!("{base}{n}");
            n += 1;
        }
        seen.push(name.clone());
        s.name = name;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn songs_of_a_genre_play_many_forms_all_of_a_sensible_length() {
        for name in ["house", "lofi", "trap", "dance_pop", "techno"] {
            let g = crate::genres::get(name).unwrap();
            let mut seen = std::collections::BTreeSet::new();
            for seed in 0..24 {
                let f = vary(&g.forms[0], &g, 120.0, &mut Rng::new(seed));
                let secs = f.iter().map(|s| s.bars as f64).sum::<f64>() * 2.0;
                assert!((120.0..=330.0).contains(&secs), "{name} {seed}: {secs} s");
                let names: std::collections::BTreeSet<&str> = f.iter().map(|s| s.name.as_str()).collect();
                assert_eq!(names.len(), f.len(), "{name} {seed}: names apart");
                assert!(f.iter().any(is_peak) || f.iter().any(|s| s.kind == SectionKind::Groove), "{name} {seed}");
                let layers: Vec<String> = g.forms[0].iter().flat_map(|s| s.layers.clone()).collect();
                assert!(f.iter().all(|s| s.layers.iter().all(|l| layers.contains(l))), "{name} {seed}: its own layers");
                // no section rests on a loop or a part a song may not have
                for s in &f {
                    assert!(s.layers.iter().any(|l| sure(l, &g)), "{name} {seed}: {} plays {:?}", s.name, s.layers);
                }
                seen.insert(f.iter().map(|s| format!("{}{}", s.kind.name(), s.bars)).collect::<Vec<_>>().join(" "));
            }
            assert!(seen.len() >= 18, "{name}: {} forms in 24 songs", seen.len());
        }
    }

    #[test]
    fn every_genres_sections_rest_on_something_sure_to_play() {
        for g in crate::genres::all().unwrap() {
            for (fi, template) in g.forms.iter().enumerate() {
                for seed in 0..12 {
                    for s in vary(template, &g, 120.0, &mut Rng::new(seed)) {
                        assert!(
                            s.layers.iter().any(|l| sure(l, &g)),
                            "{} form {fi} seed {seed}: {} {:?}",
                            g.name,
                            s.name,
                            s.layers
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_bridge_plays_the_melodic_parts_on_the_second_progression() {
        let g = crate::genres::get("dance_pop").unwrap();
        let bridge = (0..200)
            .map(|seed| vary(&g.forms[0], &g, 120.0, &mut Rng::new(seed)))
            .find_map(|f| f.into_iter().find(|s| s.name == "bridge"))
            .expect("some song has a bridge");
        assert_eq!((bridge.kind, bridge.prog.as_str()), (SectionKind::Break, "B"));
        assert!(bridge.layers.iter().all(|l| !g.drums.iter().any(|d| &d.name == l)), "{:?}", bridge.layers);
    }
}
