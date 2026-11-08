//! A form of its own for every song. A genre's `forms` are templates; a generated song plays one of them varied, each
//! change drawn with its own chance:
//!
//! - its sections lengthened or shortened, to the bar counts a section of that kind takes;
//! - a third verse and hook, after the last hook before the song's first break (or after its last hook);
//! - a bridge, the song's melodic parts without its drums and bass on its second progression: before the last hook or
//!   drop (and the build into it), in a song of two or more with no break between the first and the last, or after
//!   a song's only hook or drop when more than the outro follows;
//! - the last hook or drop played twice;
//! - a lift, a four-bar build, into the first hook that has none, in a genre that rises into its peaks;
//! - the intro left out (a cold start) or stripped to its drums (when they have the kick) or to its melodic parts;
//! - the outro left out (a cold ending) or thinned to half its parts;
//! - a first verse without one of its parts, a later verse with one more from the hook, a first hook without one part
//!   the later ones play;
//! - fills, gaps before a drop and the intro's filter toggled now and then, and each section's energy nudged.
//!
//! A section that gains or loses tracks moves its energy with them.
//!
//! The song then keeps to two to five and a half minutes at its tempo, and every section has a name of its own.

use crate::genres::GenreProfile;
use crate::model::{Role, SectionKind, SectionSpec};
use crate::rng::Rng;

/// the bar counts a section of `kind` takes
fn lengths(kind: SectionKind) -> &'static [u32] {
    match kind {
        SectionKind::Intro => &[4, 8, 16],
        SectionKind::Verse => &[8, 16],
        SectionKind::Build => &[4, 8],
        SectionKind::Drop => &[16, 24, 32],
        // a breakdown of four bars is over before it is heard as one
        SectionKind::Break => &[8, 16],
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
    // a genre built on samples plays its phrase as a part (or, without one, the parts it names instead)
    let phrase = crate::sampling::need(g, crate::library::prepared::Role::Phrase).is_some();
    s.layers
        .iter()
        .filter(|l| {
            g.parts.iter().any(|p| p.name == **l && p.optional <= 0.0)
                || (phrase && l.as_str() == crate::sampling::PHRASE)
        })
        .collect()
}

/// whether a layer is sure to play in every song of the genre: a drum lane, or a part never left out
fn sure(layer: &str, g: &GenreProfile) -> bool {
    let plays =
        |l: &str| g.drums.iter().any(|d| d.name == l) || g.parts.iter().any(|p| p.name == l && p.optional <= 0.0);
    // a sample, or the layers the genre plays instead of it when it has none
    let sampled = crate::library::prepared::Role::ALL.into_iter().any(|r| {
        crate::sampling::track_name(r) == layer
            && crate::sampling::need(g, r).is_some_and(|n| n.instead.iter().any(|l| plays(l)))
    });
    plays(layer) || sampled
}

/// how much energy a track brings to a section: a kick the most, then a bass, then anything else
fn weight(layer: &str, g: &GenreProfile) -> f32 {
    if layer == "kick" || layer == crate::sampling::BREAK {
        0.15
    } else if g.parts.iter().any(|p| p.name == layer && p.role == Role::Bass) {
        0.1
    } else {
        0.05
    }
}

/// a section's energy moved with the tracks it gained or lost since it played `before`, so that its loudness still
/// follows its energy; a section that lost all its drums is quiet whatever it was
pub(crate) fn retune(s: &mut SectionSpec, before: &[String], g: &GenreProfile) {
    let gained: f32 = s.layers.iter().filter(|l| !before.contains(l)).map(|l| weight(l, g)).sum();
    let lost: f32 = before.iter().filter(|l| !s.layers.contains(l)).map(|l| weight(l, g)).sum();
    s.energy = (s.energy + gained - lost).clamp(0.1, 1.0);
    if drums(s, g).is_empty() && before.iter().any(|l| g.drums.iter().any(|d| d.name == *l)) {
        s.energy = s.energy.min(0.2);
    }
}

/// `template` varied for one song (the module's doc says how), at `bpm`. Each change draws from a fork of `rng` of its
/// own, so that changing one leaves the others' draws as they were.
pub fn vary(template: &[SectionSpec], g: &GenreProfile, bpm: f64, rng: &Rng) -> Vec<SectionSpec> {
    let mut form: Vec<SectionSpec> = template.to_vec();
    if form.is_empty() {
        return form;
    }
    // lengths
    let mut r = rng.fork("lengths");
    for s in form.iter_mut() {
        let options: Vec<u32> = lengths(s.kind).iter().copied().filter(|&b| b != s.bars).collect();
        if !options.is_empty() && r.chance(0.45) {
            s.bars = options[r.below(options.len())];
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
        && rng.fork("third verse").chance(0.3)
    {
        let (verse, hook) = (form[v].clone(), form[h].clone());
        form.insert(h + 1, verse);
        form.insert(h + 2, hook);
    }
    // a bridge before the last peak (and the build into it), when the song has two peaks and nothing breaks it between
    // them; after a song's only peak, when more than its outro follows
    let mut r = rng.fork("bridge");
    let (first, last) = (form.iter().position(is_peak), form.iter().rposition(is_peak));
    let at = match (first, last) {
        (Some(a), Some(b)) if a < b && !form[a..b].iter().any(|s| s.kind == SectionKind::Break) => {
            Some(if form[b - 1].kind == SectionKind::Build { b - 1 } else { b })
        }
        (Some(a), Some(b)) if a == b && form.get(a + 1).is_some_and(|s| s.kind != SectionKind::Outro) => Some(a + 1),
        _ => None,
    };
    if let (Some(at), Some(last)) = (at, last)
        && r.chance(0.35)
    {
        // a breakdown: the bass leaves with the drums (a trap 808 alone is as loud as the hook)
        let layers: Vec<String> = melodic(&form[last], g)
            .into_iter()
            .filter(|l| {
                l.as_str() == crate::sampling::PHRASE || g.parts.iter().any(|q| q.name == **l && q.role != Role::Bass)
            })
            .cloned()
            .collect();
        if layers.len() >= 2 {
            form.insert(
                at,
                SectionSpec {
                    name: "bridge".into(),
                    kind: SectionKind::Break,
                    bars: 8,
                    energy: 0.25,
                    prog: "B".into(),
                    layers,
                    variant: "sparse".into(),
                    filter_in: false,
                    build: false,
                    build_style: Default::default(),
                    build_amount: 0.5,
                    gap: r.chance(0.4),
                    impact: false,
                    fills: false,
                },
            );
        }
    }
    // the last peak twice
    if let Some(p) = form.iter().rposition(is_peak)
        && rng.fork("twice").chance(0.3)
    {
        let again = SectionSpec { energy: 1.0, impact: false, ..form[p].clone() };
        form.insert(p + 1, again);
    }
    // a lift into the first hook that has no build before it, in a genre that rises into its peaks
    if let Some(h) = form.iter().position(|s| s.kind == SectionKind::Hook)
        && h > 0
        && form[h - 1].kind != SectionKind::Build
        && g.transitions.riser
        && rng.fork("lift").chance(0.35)
    {
        // as loud as what came before: the riser and the roll make up for the kick
        let before = &form[h - 1];
        let layers: Vec<String> = before.layers.iter().filter(|l| l.as_str() != "kick").cloned().collect();
        let lift = SectionSpec {
            name: "lift".into(),
            kind: SectionKind::Build,
            bars: 4,
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
    let mut r = rng.fork("intro");
    if form.len() > 4 && form[0].kind == SectionKind::Intro {
        let x = r.f64();
        if x < 0.15 {
            form.remove(0);
        } else if x < 0.4 {
            let to_drums = r.chance(0.5);
            let keep: Vec<String> =
                if to_drums { drums(&form[0], g) } else { melodic(&form[0], g) }.into_iter().cloned().collect();
            // drums alone open on their kick: a hat alone, behind an intro's filter, is next to silence
            if !keep.is_empty() && (!to_drums || keep.iter().any(|l| l == "kick")) {
                let before = std::mem::replace(&mut form[0].layers, keep);
                retune(&mut form[0], &before, g);
            }
        }
    }
    // the outro: left out (the song ends on its last section), or thinned
    let mut r = rng.fork("outro");
    if form.len() > 4 && form.last().is_some_and(|s| s.kind == SectionKind::Outro) {
        let x = r.f64();
        if x < 0.15 {
            form.pop();
        } else if x < 0.4
            && let Some(o) = form.last_mut()
            && o.layers.len() > 2
        {
            let keep = o.layers.len().div_ceil(2);
            let mut layers = o.layers.clone();
            while layers.len() > keep {
                layers.remove(r.below(layers.len()));
            }
            // something sure to play stays
            if layers.iter().any(|l| sure(l, g)) {
                let before = std::mem::replace(&mut o.layers, layers);
                retune(o, &before, g);
            }
        }
    }
    // layers: the first verse thinner, a later one fuller, the first hook without one of the later hooks' parts
    let verses: Vec<usize> = (0..form.len()).filter(|&i| form[i].kind == SectionKind::Verse).collect();
    let mut r = rng.fork("first verse");
    if let Some(&first) = verses.first() {
        let m: Vec<String> = melodic(&form[first], g).into_iter().cloned().collect();
        if m.len() >= 2 && r.chance(0.3) {
            let drop = &m[r.below(m.len())];
            let before = form[first].layers.clone();
            form[first].layers.retain(|l| l != drop);
            retune(&mut form[first], &before, g);
        }
    }
    let mut r = rng.fork("later verses");
    for &v in verses.iter().skip(1) {
        if let Some(h) = (v..form.len()).find(|&i| form[i].kind == SectionKind::Hook) {
            let missing: Vec<String> = form[h].layers.iter().filter(|l| !form[v].layers.contains(l)).cloned().collect();
            if !missing.is_empty() && r.chance(0.35) {
                let add = missing[r.below(missing.len())].clone();
                let before = form[v].layers.clone();
                form[v].layers.push(add);
                retune(&mut form[v], &before, g);
            }
        }
    }
    let hooks: Vec<usize> = (0..form.len()).filter(|&i| form[i].kind == SectionKind::Hook).collect();
    let mut r = rng.fork("first hook");
    if hooks.len() >= 2 && r.chance(0.25) {
        let m: Vec<String> = melodic(&form[hooks[0]], g).into_iter().cloned().collect();
        if m.len() >= 3 {
            let drop = &m[r.below(m.len())];
            let before = form[hooks[0]].layers.clone();
            form[hooks[0]].layers.retain(|l| l != drop);
            retune(&mut form[hooks[0]], &before, g);
        }
    }
    // flags and energy
    let (mut fills, mut gaps, mut filters, mut nudges) =
        (rng.fork("fills"), rng.fork("gaps"), rng.fork("filters"), rng.fork("energy"));
    for i in 0..form.len() {
        let next_peak = form.get(i + 1).is_some_and(is_peak);
        let s = &mut form[i];
        if matches!(s.kind, SectionKind::Verse | SectionKind::Groove | SectionKind::Drop) && fills.chance(0.25) {
            s.fills = !s.fills;
        }
        if next_peak && !s.build && gaps.chance(0.3) {
            s.gap = !s.gap;
        }
        if s.kind == SectionKind::Intro && filters.chance(0.4) {
            // an intro opening through a lowpass plays most of its length dark and quieter
            s.filter_in = !s.filter_in;
            s.energy += if s.filter_in { -0.1 } else { 0.1 };
        }
        s.energy = (s.energy + nudges.range(-0.05, 0.05) as f32).clamp(0.1, 1.0);
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
                let f = vary(&g.forms[0], &g, 120.0, &Rng::new(seed));
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
    fn every_genres_sections_rest_on_something_sure_to_play_and_builds_lead_into_peaks() {
        for g in crate::genres::all().unwrap() {
            for (fi, template) in g.forms.iter().enumerate() {
                for seed in 0..12 {
                    let f = vary(template, &g, 120.0, &Rng::new(seed));
                    for (i, s) in f.iter().enumerate() {
                        assert!(
                            s.layers.iter().any(|l| sure(l, &g)),
                            "{} form {fi} seed {seed}: {} {:?}",
                            g.name,
                            s.name,
                            s.layers
                        );
                        if s.kind == SectionKind::Build {
                            let next = f.get(i + 1).map(|n| n.name.as_str());
                            assert!(
                                f.get(i + 1).is_some_and(is_peak),
                                "{} seed {seed}: {} then {next:?}",
                                g.name,
                                s.name
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_bridge_plays_the_melodic_parts_without_drums_or_bass_on_the_second_progression() {
        // boom bap's melody is its sampled phrase
        for name in ["trap", "lofi", "boom_bap"] {
            let g = crate::genres::get(name).unwrap();
            let bridge = (0..200)
                .map(|seed| vary(&g.forms[0], &g, 120.0, &Rng::new(seed)))
                .find_map(|f| f.into_iter().find(|s| s.name == "bridge"))
                .unwrap_or_else(|| panic!("some {name} song has a bridge"));
            assert_eq!((bridge.kind, bridge.prog.as_str()), (SectionKind::Break, "B"));
            let melodic = |l: &String| {
                g.parts.iter().any(|p| &p.name == l && p.role != Role::Bass) || l == crate::sampling::PHRASE
            };
            assert!(bridge.layers.iter().all(melodic), "{name}: {:?}", bridge.layers);
        }
    }
}
