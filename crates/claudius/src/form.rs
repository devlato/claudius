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
//!   the later ones play (never the bass, which carries the low end from the first verse on);
//! - fills, gaps before a drop and the intro's filter toggled now and then, and each section's energy nudged.
//!
//! A section that gains or loses tracks moves its energy with them.
//!
//! The song then keeps to two to five and a half minutes at its tempo, and every section has a name of its own.
//!
//! Evolution revises a song's form afterwards, one change at a time and toward what the critic asks of it ([`change`]):
//! a section longer or shorter, a verse and hook or a drop again, a section left out, a bridge or a lift, a shorter way
//! in or out, two neighbours set apart.

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

/// the melodic parts a section can do without for a while: not its bass, which carries its low end
fn thinnable<'a>(s: &'a SectionSpec, g: &GenreProfile) -> Vec<&'a String> {
    let bass = |l: &str| l == crate::sampling::BASS || g.parts.iter().any(|p| p.name == l && p.role == Role::Bass);
    melodic(s, g).into_iter().filter(|l| !bass(l)).collect()
}

/// whether a layer is sure to play in every song of the genre: a drum lane, or a part never left out
pub(crate) fn sure(layer: &str, g: &GenreProfile) -> bool {
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
    if let Some(at) = bridge_at(&form)
        && r.chance(0.35)
        && let Some(b) = bridge(&form, g, r.chance(0.4))
    {
        form.insert(at, b);
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
        let lift = lift(&form[h - 1]);
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
        let m: Vec<String> = thinnable(&form[first], g).into_iter().cloned().collect();
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
        let m: Vec<String> = thinnable(&form[hooks[0]], g).into_iter().cloned().collect();
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

/// A name for a section new to `form`, like `name`: its word alone, then numbered from 2, the first that no section of
/// `form` has, so that no section it has is renamed (a take starts from its section by name).
fn fresh(form: &[SectionSpec], name: &str) -> String {
    let base = name.trim_end_matches(|c: char| c.is_ascii_digit());
    std::iter::once(base.to_string())
        .chain((2..).map(|n| format!("{base}{n}")))
        .find(|n| !form.iter().any(|s| s.name == *n))
        .unwrap_or_else(|| name.to_string())
}

/// where a bridge goes: before the last peak (and the build into it), when the song has two peaks and nothing breaks
/// it between them; after a song's only peak, when more than its outro follows
fn bridge_at(form: &[SectionSpec]) -> Option<usize> {
    match (form.iter().position(is_peak), form.iter().rposition(is_peak)) {
        (Some(a), Some(b)) if a < b && !form[a..b].iter().any(|s| s.kind == SectionKind::Break) => {
            Some(if form[b - 1].kind == SectionKind::Build { b - 1 } else { b })
        }
        (Some(a), Some(b)) if a == b && form.get(a + 1).is_some_and(|s| s.kind != SectionKind::Outro) => Some(a + 1),
        _ => None,
    }
}

/// a bridge, eight bars on the second progression of the last peak's melodic parts without its drums and its bass (a
/// trap 808 alone is as loud as the hook); none when fewer than two of them play
fn bridge(form: &[SectionSpec], g: &GenreProfile, gap: bool) -> Option<SectionSpec> {
    let last = form.iter().rfind(|s| is_peak(s))?;
    let layers: Vec<String> = melodic(last, g)
        .into_iter()
        .filter(|l| {
            l.as_str() == crate::sampling::PHRASE || g.parts.iter().any(|q| q.name == **l && q.role != Role::Bass)
        })
        .cloned()
        .collect();
    (layers.len() >= 2).then(|| SectionSpec {
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
        gap,
        impact: false,
        fills: false,
    })
}

/// a four-bar build out of `before` into a hook, as loud as what came before: the riser and the roll make up for the
/// kick
fn lift(before: &SectionSpec) -> SectionSpec {
    let layers: Vec<String> = before.layers.iter().filter(|l| l.as_str() != "kick").cloned().collect();
    SectionSpec {
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
    }
}

/// What a revision of a song's form is for, as the critic's issue with it asks (`judge::Issue::target`): the song
/// longer or shorter, its way in shorter, two neighbours that sound as one set apart, its longest section shorter, its
/// hook or drop heard more often or less, or any of these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aim {
    Longer,
    Shorter,
    Intro,
    Apart,
    Section,
    MorePeaks,
    FewerPeaks,
    Any,
}

impl Aim {
    /// what the critic's issue with `target` asks of the form, when it asks something of it
    pub fn of(target: &str) -> Option<Aim> {
        Some(match target {
            "form:short" => Aim::Longer,
            "form:long" => Aim::Shorter,
            "form:intro" => Aim::Intro,
            "form:same" => Aim::Apart,
            "form:section" => Aim::Section,
            "form:peaks-" => Aim::MorePeaks,
            "form:peaks+" => Aim::FewerPeaks,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Change {
    Lengthen,
    Shorten,
    Again,
    Twice,
    Bridge,
    Lift,
    Intro,
    Outro,
    LeaveOut,
    Apart,
    Energy,
}

/// A song's form revised the way a producer revises one, by one change toward `aim`:
///
/// - a section lengthened or shortened to the next bar count its kind takes (for [`Aim::Section`], the longest);
/// - the last peak's way in and the peak played again after it (a verse and the hook, or a build and the drop);
/// - the last peak played twice;
/// - a bridge, or a lift into a hook without a build, as [`vary`] makes them;
/// - the intro or the outro shortened, or left out;
/// - a section heard earlier in the song left out again (with the build into it), never the last peak;
/// - of two neighbours that play the same layers, drums and progression, the second thinner by a part, fuller by one
///   of the next peak's, or on the other progression;
/// - a section's energy nudged.
///
/// The form changed and what changed, or None when no change toward `aim` fits it. The song keeps three sections, a
/// peak (or a groove), builds that lead into peaks, sections that rest on something sure to play, and
/// [`crate::model::MAX_BARS`] at most; a section new to it has a name no other has, and every other keeps its own.
pub fn change(form: &[SectionSpec], g: &GenreProfile, aim: Aim, rng: &mut Rng) -> Option<(Vec<SectionSpec>, String)> {
    use Change::*;
    let all = [Lengthen, Shorten, Again, Twice, Bridge, Lift, Intro, Outro, LeaveOut, Apart, Energy];
    let options: &[Change] = match aim {
        Aim::Longer => &[Lengthen, Again, Twice, Bridge],
        Aim::Shorter => &[Shorten, Intro, Outro, LeaveOut],
        Aim::Intro => &[Intro],
        Aim::Apart => &[Apart, Apart, Bridge, Lift],
        Aim::Section => &[Shorten],
        Aim::MorePeaks => &[Again, Twice],
        Aim::FewerPeaks => &[LeaveOut],
        Aim::Any => &all,
    };
    for _ in 0..8 {
        let mut f = form.to_vec();
        let Some(what) = apply(&mut f, g, *rng.pick(options), aim, rng) else { continue };
        if f != form && sound(&f, g) {
            return Some((f, what));
        }
    }
    None
}

/// whether a revised form is still one a song plays
fn sound(f: &[SectionSpec], g: &GenreProfile) -> bool {
    f.len() >= 3
        && f.iter().map(|s| s.bars).sum::<u32>() <= crate::model::MAX_BARS
        && (f.iter().any(is_peak) || f.iter().any(|s| s.kind == SectionKind::Groove))
        && f.iter().enumerate().all(|(i, s)| s.kind != SectionKind::Build || f.get(i + 1).is_some_and(is_peak))
        && f.iter().all(|s| s.layers.iter().any(|l| sure(l, g)))
}

fn apply(f: &mut Vec<SectionSpec>, g: &GenreProfile, c: Change, aim: Aim, rng: &mut Rng) -> Option<String> {
    let next = |s: &SectionSpec, up: bool| {
        let ls = lengths(s.kind).iter().copied();
        if up { ls.filter(|&b| b > s.bars).min() } else { ls.filter(|&b| b < s.bars).max() }
    };
    match c {
        Change::Lengthen | Change::Shorten => {
            let up = c == Change::Lengthen;
            let can: Vec<usize> = (0..f.len()).filter(|&i| next(&f[i], up).is_some()).collect();
            let i = if aim == Aim::Section {
                *can.iter().max_by_key(|&&i| f[i].bars)?
            } else {
                *can.get(rng.below(can.len()))?
            };
            f[i].bars = next(&f[i], up)?;
            Some(format!("{} now {} bars", f[i].name, f[i].bars))
        }
        Change::Again => {
            // a verse and its hook again where `vary` plays a third: after the last hook before the song's first
            // break after one, or after its last hook
            let first = f.iter().position(|s| s.kind == SectionKind::Hook);
            let brk = first.and_then(|h| (h..f.len()).find(|&i| f[i].kind == SectionKind::Break));
            let hook = match brk {
                Some(b) => f[..b].iter().rposition(|s| s.kind == SectionKind::Hook),
                None => f.iter().rposition(|s| s.kind == SectionKind::Hook),
            };
            let verse = hook.and_then(|h| f[..h].iter().rposition(|s| s.kind == SectionKind::Verse));
            let (at, again) = match (hook, verse) {
                (Some(h), Some(v)) => (h + 1, vec![f[v].clone(), f[h].clone()]),
                // else the last peak and its way in (a break, a build) after it, as a drop comes back
                _ => {
                    let p = f.iter().rposition(is_peak)?;
                    let way_in = |s: &SectionSpec| matches!(s.kind, SectionKind::Break | SectionKind::Build);
                    let from = (p.saturating_sub(2)..p).find(|&i| f[i..p].iter().all(way_in)).unwrap_or(p);
                    (p + 1, f[from..=p].to_vec())
                }
            };
            let what = format!(
                "{} again after {}",
                again.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(" and "),
                f[at - 1].name
            );
            for (k, mut s) in again.into_iter().enumerate() {
                s.name = fresh(f, &s.name);
                f.insert(at + k, s);
            }
            Some(what)
        }
        Change::Twice => {
            let p = f.iter().rposition(is_peak)?;
            let again = SectionSpec { name: fresh(f, &f[p].name), energy: 1.0, impact: false, ..f[p].clone() };
            let what = format!("{} twice", f[p].name);
            f.insert(p + 1, again);
            Some(what)
        }
        Change::Bridge => {
            let at = bridge_at(f)?;
            let b = SectionSpec { name: fresh(f, "bridge"), ..bridge(f, g, rng.chance(0.4))? };
            let what = match f.get(at) {
                Some(s) => format!("a bridge before {}", s.name),
                None => "a bridge at the end".to_string(),
            };
            f.insert(at, b);
            Some(what)
        }
        // in a genre that rises into its peaks, as `vary` lifts one
        Change::Lift if g.transitions.riser => {
            let h = (1..f.len()).find(|&i| f[i].kind == SectionKind::Hook && f[i - 1].kind != SectionKind::Build)?;
            let what = format!("a lift into {}", f[h].name);
            let lift = SectionSpec { name: fresh(f, "lift"), ..lift(&f[h - 1]) };
            f.insert(h, lift);
            Some(what)
        }
        Change::Lift => None,
        Change::Intro | Change::Outro => {
            let i = if c == Change::Intro { 0 } else { f.len().checked_sub(1)? };
            let kind = if c == Change::Intro { SectionKind::Intro } else { SectionKind::Outro };
            if f.get(i)?.kind != kind {
                return None;
            }
            match next(&f[i], false) {
                Some(b) if f.len() <= 4 || rng.chance(0.7) => {
                    f[i].bars = b;
                    Some(format!("{} now {b} bars", f[i].name))
                }
                _ if f.len() > 4 => Some(format!("{} left out", f.remove(i).name)),
                _ => None,
            }
        }
        Change::LeaveOut => {
            let last = f.iter().rposition(is_peak)?;
            let heard = |i: usize| f[..i].iter().any(|s| s.kind == f[i].kind);
            let can: Vec<usize> = (1..f.len())
                .filter(|&i| i != last && heard(i) && !matches!(f[i].kind, SectionKind::Build | SectionKind::Outro))
                .filter(|&i| aim != Aim::FewerPeaks || is_peak(&f[i]))
                .collect();
            let i = *can.get(rng.below(can.len()))?;
            let from = if is_peak(&f[i]) && f[i - 1].kind == SectionKind::Build { i - 1 } else { i };
            let gone: Vec<String> = f.drain(from..=i).map(|s| s.name).collect();
            Some(format!("{} left out", gone.join(" and ")))
        }
        Change::Apart => {
            let alike =
                |a: &SectionSpec, b: &SectionSpec| (&a.layers, &a.variant, &a.prog) == (&b.layers, &b.variant, &b.prog);
            let pairs: Vec<usize> = (1..f.len()).filter(|&i| alike(&f[i - 1], &f[i])).collect();
            let i = *pairs.get(rng.below(pairs.len()))?;
            let before = f[i].layers.clone();
            let thinner: Vec<String> = thinnable(&f[i], g).into_iter().cloned().collect();
            let fuller: Vec<String> = (i..f.len())
                .find(|&j| is_peak(&f[j]))
                .map(|j| f[j].layers.iter().filter(|l| !before.contains(l)).cloned().collect())
                .unwrap_or_default();
            let what = match rng.below(3) {
                0 if thinner.len() >= 2 => {
                    let out = rng.pick(&thinner).clone();
                    f[i].layers.retain(|l| *l != out);
                    format!("{} without {out}, unlike {}", f[i].name, f[i - 1].name)
                }
                1 if !fuller.is_empty() => {
                    let add = rng.pick(&fuller).clone();
                    f[i].layers.push(add.clone());
                    format!("{} with {add}, unlike {}", f[i].name, f[i - 1].name)
                }
                _ => {
                    f[i].prog = if f[i].prog == "A" { "B" } else { "A" }.into();
                    return Some(format!("{} on progression {}, unlike {}", f[i].name, f[i].prog, f[i - 1].name));
                }
            };
            retune(&mut f[i], &before, g);
            Some(what)
        }
        Change::Energy => {
            let i = rng.below(f.len());
            let e = f[i].energy;
            f[i].energy = (e + rng.range(-0.1, 0.1) as f32).clamp(0.1, 1.0);
            Some(format!("{} energy {e:.2} -> {:.2}", f[i].name, f[i].energy))
        }
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
    fn the_first_verse_and_hook_keep_the_bass_their_genre_gives_them() {
        for g in crate::genres::all().unwrap() {
            let bass: Vec<&str> = g.parts.iter().filter(|p| p.role == Role::Bass).map(|p| p.name.as_str()).collect();
            let plays = |s: &SectionSpec| s.layers.iter().any(|l| bass.contains(&l.as_str()));
            for (fi, template) in g.forms.iter().enumerate() {
                for kind in [SectionKind::Verse, SectionKind::Hook] {
                    if !template.iter().find(|s| s.kind == kind).is_some_and(plays) {
                        continue;
                    }
                    for seed in 0..24 {
                        let f = vary(template, &g, 120.0, &Rng::new(seed));
                        let first = f.iter().find(|s| s.kind == kind);
                        assert!(first.is_none_or(plays), "{} form {fi} seed {seed}: the first {kind:?}", g.name);
                    }
                }
            }
        }
    }

    #[test]
    fn a_revised_form_is_still_one_a_song_plays_and_goes_the_way_it_was_aimed() {
        let bars = |f: &[SectionSpec]| f.iter().map(|s| s.bars).sum::<u32>();
        let peaks = |f: &[SectionSpec]| f.iter().filter(|s| is_peak(s)).count();
        let aims = [
            Aim::Longer,
            Aim::Shorter,
            Aim::Intro,
            Aim::Apart,
            Aim::Section,
            Aim::MorePeaks,
            Aim::FewerPeaks,
            Aim::Any,
        ];
        let mut found = std::collections::BTreeMap::<String, usize>::new();
        for g in crate::genres::all().unwrap() {
            for (fi, template) in g.forms.iter().enumerate() {
                let mut rng = Rng::new(fi as u64);
                let start = vary(template, &g, 120.0, &Rng::new(fi as u64));
                for aim in aims {
                    let mut f = start.clone();
                    for _ in 0..4 {
                        let Some((next, what)) = change(&f, &g, aim, &mut rng) else { break };
                        let at = format!("{} form {fi} {aim:?}: {what}", g.name);
                        *found.entry(format!("{aim:?}")).or_default() += 1;
                        assert!(sound(&next, &g), "{at}");
                        let names: std::collections::BTreeSet<&str> = next.iter().map(|s| s.name.as_str()).collect();
                        assert_eq!(names.len(), next.len(), "{at}: names apart");
                        // what it kept keeps its name: the same kind, in the same order
                        let kept: Vec<(&str, SectionKind)> = f
                            .iter()
                            .filter_map(|s| next.iter().find(|n| n.name == s.name).map(|n| (n.name.as_str(), n.kind)))
                            .collect();
                        let was: Vec<(&str, SectionKind)> = f
                            .iter()
                            .filter(|s| next.iter().any(|n| n.name == s.name))
                            .map(|s| (s.name.as_str(), s.kind))
                            .collect();
                        assert_eq!(kept, was, "{at}");
                        let order: Vec<&str> =
                            next.iter().map(|n| n.name.as_str()).filter(|n| was.iter().any(|w| w.0 == *n)).collect();
                        assert_eq!(order, was.iter().map(|w| w.0).collect::<Vec<_>>(), "{at}: in its order");
                        let (b, nb, p, np) = (bars(&f), bars(&next), peaks(&f), peaks(&next));
                        match aim {
                            Aim::Longer => assert!(nb > b, "{at}"),
                            Aim::Shorter | Aim::Intro | Aim::Section => assert!(nb < b, "{at}"),
                            Aim::MorePeaks => assert!(np > p && nb > b, "{at}"),
                            Aim::FewerPeaks => assert!(np < p && nb < b, "{at}"),
                            Aim::Apart | Aim::Any => {}
                        }
                        f = next;
                    }
                }
            }
        }
        // every aim finds a change in most forms, but setting apart (which needs two neighbours alike) and fewer peaks
        // (which needs two); a form's intro runs out of changes once it is left out
        let forms: usize = crate::genres::all().unwrap().iter().map(|g| g.forms.len()).sum();
        for aim in aims.iter().filter(|a| !matches!(a, Aim::Apart | Aim::FewerPeaks)) {
            assert!(found.get(&format!("{aim:?}")).copied().unwrap_or(0) >= forms, "{aim:?}: {found:?}");
        }
    }

    #[test]
    fn a_section_added_takes_a_name_of_its_own_and_leaves_the_others_theirs() {
        // a bridge goes before the last hook, and this song already plays one after it
        let g = crate::genres::get("synthwave").unwrap();
        let mut form: Vec<SectionSpec> = g.forms[0].iter().filter(|s| s.kind != SectionKind::Break).cloned().collect();
        let last = form.iter().rposition(is_peak).unwrap();
        let after = SectionSpec { name: "bridge".into(), kind: SectionKind::Break, ..form[last].clone() };
        form.insert(last + 1, after);
        let mut rng = Rng::new(1);
        let (f, what) = (0..64)
            .find_map(|_| change(&form, &g, Aim::Longer, &mut rng).filter(|(_, w)| w.starts_with("a bridge")))
            .expect("a bridge");
        let named = |f: &[SectionSpec], n: &str| f.iter().position(|s| s.name == n).unwrap();
        assert!(named(&f, "bridge") > named(&f, "chorus3"), "{what}: the song's own bridge is where it was");
        assert!(named(&f, "bridge2") < named(&f, "chorus3"), "{what}: the new one before the last chorus");
    }

    #[test]
    fn neighbours_that_play_alike_are_set_apart() {
        let g = crate::genres::get("synthwave").unwrap();
        let mut form = g.forms[0].clone();
        let at = form.iter().position(|s| s.kind == SectionKind::Verse).unwrap();
        form.insert(at + 1, SectionSpec { name: "verse again".into(), ..form[at].clone() });
        let mut rng = Rng::new(3);
        for _ in 0..12 {
            let (f, what) = change(&form, &g, Aim::Apart, &mut rng).unwrap();
            if what.contains("unlike") {
                let (a, b) = (&f[at], &f[at + 1]);
                assert!(a.layers != b.layers || a.prog != b.prog, "{what}");
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
