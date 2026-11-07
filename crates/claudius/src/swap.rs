//! Another sound for a track, as a producer swaps one: the sounds a part or a drum lane may play instead, best first
//! (`choices`), and the one after the sound it plays now that no other part plays (`next`). A part's are its genre's
//! own sounds for it, then what its genre's palette draws for the role, the kinds it lists taken in turn as often as
//! their weights say (its synths, and the installed instruments of each kind that can play the part's register), then
//! the other synths made for the role and the other installed instruments that can play it. A sound of an era gone
//! plays only where the genre's draws would play one, and General MIDI's only where nothing else of its kind can. A
//! drum lane's are its sound on each of the genre's kits, on the other installed kits, and its synthesized voice, as it
//! is and in three other tunings. The drums also swap as a kit (`next_kit`): each lane onto the next kit with its
//! sound, or all of them back to the synthesized voices.

use std::collections::{BTreeSet, VecDeque};

use crate::compose;
use crate::genres::{self, GenreProfile};
use crate::model::{Blueprint, DrumLane, Part, Sound};
use crate::palette;
use crate::script::{drum_label, drum_name, drum_role, voice_word};
use crate::synth::drums::DrumVoice;
use crate::synth::presets;

/// what the drums play when they are on no kit: each lane's synthesized voice
pub const SYNTHESIZED: &str = "synthesized";

/// how near its genre a part's sound is (`Choice::group`)
const OWN: &str = "the genre's own";
const FITS: &str = "fits the genre";
const AFIELD: &str = "further afield";

/// A sound a track may swap to: what a script names it (a part's `preset`, a drum lane's `drum_sound`), what it is
/// (`synth`, a kind of instrument such as `bass` or `electric piano`, a kit's name, or `synthesized`), and how near
/// the genre it is (`the genre's own`, `fits the genre`, `further afield`; for drums `the genre's kits`, `other kits`,
/// `synthesized`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub name: String,
    pub kind: String,
    pub group: &'static str,
}

fn push(out: &mut Vec<Choice>, name: &str, kind: &str, group: &'static str) {
    if !name.is_empty() && !out.iter().any(|c| c.name == name) {
        out.push(Choice { name: name.to_string(), kind: kind.to_string(), group });
    }
}

/// The sounds track `track` of `bp` may swap to, best first; None for a track that is neither a part nor a drum
/// lane (a loop, a take or a recording has no other sound).
pub fn choices(bp: &Blueprint, track: &str) -> Option<Vec<Choice>> {
    let g = genres::get(&bp.genre).ok();
    if let Some(p) = bp.parts.iter().find(|p| p.name == track) {
        return Some(part_choices(g.as_ref(), p));
    }
    bp.drums.iter().find(|d| d.name == track).map(|d| lane_choices(g.as_ref(), d))
}

/// the choice `track` plays now, by name, when its sound is one a choice names
pub fn playing(bp: &Blueprint, track: &str) -> Option<String> {
    if let Some(p) = bp.parts.iter().find(|p| p.name == track) {
        return match &p.sound {
            Sound::Synth { preset, .. } => Some(preset.clone()),
            Sound::Instrument { name, .. } => Some(name.clone()),
            _ => None,
        };
    }
    lane_playing(bp.drums.iter().find(|d| d.name == track)?)
}

fn lane_playing(d: &DrumLane) -> Option<String> {
    match &d.sound {
        Sound::Instrument { name, .. } => Some(name.clone()),
        // the voice as it is is its first tuning
        Sound::Drum { voice, params } => {
            drum_label(*voice, params).map(|l| if l.contains(':') { l } else { format!("{l}:0") })
        }
        _ => None,
    }
}

/// what a part playing the sound a choice names plays, as `palette::identity` tells sounds apart
pub fn identity(name: &str) -> String {
    if presets::get(name).is_some() { format!("synth:{name}") } else { name.to_string() }
}

/// The choice after the one `track` plays now, wrapping round, that no other track of `bp` plays and none of `taken`
/// (`identity`s) is; None when there is no such other. From a sound near its genre (or one none of them names) it
/// goes round the near ones, the genre's own and those that fit it, and further afield only when none of those is
/// left; from one further afield, on through them all.
pub fn next(bp: &Blueprint, track: &str, taken: &BTreeSet<String>) -> Option<Choice> {
    let list = choices(bp, track)?;
    let now = playing(bp, track);
    let mut busy: BTreeSet<String> = taken.clone();
    if bp.parts.iter().any(|p| p.name == track) {
        busy.extend(bp.parts.iter().filter(|p| p.name != track).filter_map(|p| palette::identity(&p.sound)));
    } else {
        busy.extend(bp.drums.iter().filter(|d| d.name != track).filter_map(lane_playing));
    }
    let free =
        |c: &Choice| Some(&c.name) != now.as_ref() && !busy.contains(&identity(&c.name)) && !busy.contains(&c.name);
    let after = |pool: &[&Choice]| {
        let from = now.as_ref().and_then(|n| pool.iter().position(|c| &c.name == n)).map_or(0, |i| i + 1);
        (0..pool.len()).map(|k| pool[(from + k) % pool.len()]).find(|c| free(c)).cloned()
    };
    let all: Vec<&Choice> = list.iter().collect();
    let near: Vec<&Choice> = list.iter().filter(|c| c.group != AFIELD).collect();
    let afield = now.as_ref().is_some_and(|n| list.iter().any(|c| &c.name == n && c.group == AFIELD));
    if afield { after(&all) } else { after(&near).or_else(|| after(&all)) }
}

fn part_choices(g: Option<&GenreProfile>, part: &Part) -> Vec<Choice> {
    let role = part.role;
    let mut out = Vec::new();
    // the genre's own for the part, or for its role when it has no part of the name
    let own: Vec<&String> = g.map_or_else(Vec::new, |g| {
        let mine: Vec<&String> =
            g.parts.iter().filter(|s| s.name == part.name && s.role == role).flat_map(|s| &s.presets).collect();
        if mine.is_empty() {
            g.parts.iter().filter(|s| s.role == role).flat_map(|s| &s.presets).collect()
        } else {
            mine
        }
    });
    for alt in own.iter().flat_map(|e| e.split('|')) {
        if presets::get(alt).is_some() {
            push(&mut out, alt, "synth", OWN);
        } else if let Some(r) = claudius_packs::resolve(alt, None) {
            let kind = palette::instruments().into_iter().find(|i| i.name == r.to_string()).map(|i| i.category);
            push(&mut out, &r.to_string(), kind.as_deref().unwrap_or("instrument"), OWN);
        }
    }
    // what its palette draws, as `compose::draw_sound` reads it
    let mut rp = g.and_then(|g| g.palette.get(role.name()).cloned()).unwrap_or_else(|| palette::default_for(role));
    if let Some(g) = g {
        if rp.synths.is_empty() {
            rp.synths = compose::fitting_synths(g, role);
        }
        if rp.instruments.is_empty() {
            rp.instruments = compose::named_instruments(g, role);
        }
    }
    let installed = palette::instruments();
    let can_play: Vec<&palette::Instrument> =
        installed.iter().filter(|i| palette::suits(i, role, part.register)).collect();
    let drawn = no_general_midi(
        can_play.iter().copied().filter(|i| palette::instrument_has_place(role, &rp, &i.name)).collect(),
    );
    let mut kinds: Vec<(&str, f64)> =
        rp.kinds.iter().filter(|(_, w)| **w > 0.0).map(|(k, w)| (k.as_str(), *w)).collect();
    kinds.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut queues: Vec<(&str, f64, VecDeque<String>)> = kinds
        .iter()
        .map(|&(kind, w)| {
            let names: VecDeque<String> = if kind == "synth" {
                palette::synths_drawn(role, &rp).into_iter().map(String::from).collect()
            } else {
                drawn.iter().filter(|i| i.category == kind).map(|i| i.name.clone()).collect()
            };
            (kind, w, names)
        })
        .collect();
    // each kind in turn as often as its weight says: a kind of half the weight every other time
    let mut drew = vec![0usize; queues.len()];
    while let Some(k) = (0..queues.len())
        .filter(|&k| !queues[k].2.is_empty())
        .min_by(|&a, &b| ((drew[a] + 1) as f64 / queues[a].1).total_cmp(&((drew[b] + 1) as f64 / queues[b].1)))
    {
        let (kind, _, names) = &mut queues[k];
        if let Some(name) = names.pop_front() {
            push(&mut out, &name, kind, FITS);
        }
        drew[k] += 1;
    }
    // the instruments its palette names, of kinds it does not list
    for i in &drawn {
        push(&mut out, &i.name, &i.category, FITS);
    }
    for p in presets::drawable(role.name()) {
        if rp.retro || !presets::DATED.contains(&p) {
            push(&mut out, p, "synth", AFIELD);
        }
    }
    // of the kinds any genre's palette may draw for the role: a piano plays no bass line
    let kinds_of_role = palette::default_for(role).kinds;
    let rest = can_play
        .into_iter()
        .filter(|i| (rp.retro || !palette::dated(&i.name)) && kinds_of_role.contains_key(&i.category))
        .collect();
    for i in no_general_midi(rest) {
        push(&mut out, &i.name, &i.category, AFIELD);
    }
    out
}

/// `list` without General MIDI's instruments of a kind it has others of
fn no_general_midi(list: Vec<&palette::Instrument>) -> Vec<&palette::Instrument> {
    let better = |category: &str| list.iter().any(|i| i.category == category && !palette::general_midi(&i.name));
    list.iter().copied().filter(|i| !palette::general_midi(&i.name) || !better(&i.category)).collect()
}

/// The voice a drum lane plays, or would synthesized: its synthesized voice's; its genre's for a lane of its name, as
/// it was made (house's perc is a rim); the drum its instrument is (a kit's rim is a rim, whatever its lane is
/// called); or the one its name says.
fn lane_voice(g: Option<&GenreProfile>, d: &DrumLane) -> Option<DrumVoice> {
    if let Sound::Drum { voice, .. } = &d.sound {
        return Some(*voice);
    }
    if let Some(spec) = g.and_then(|g| g.drums.iter().find(|s| s.name == d.name)) {
        return Some(spec.voice);
    }
    if let Sound::Instrument { name, .. } = &d.sound {
        let drum = name.split(':').nth(1).and_then(drum_name);
        if let Some((voice, _)) = drum.or_else(|| drum_role(name).and_then(|role| drum_name(&role))) {
            return Some(voice);
        }
    }
    drum_name(&d.name).map(|(voice, _)| voice)
}

/// what a kit calls a voice's sound; a percussion lane plays a kit's rim, as `compose::kit_sound` plays it
fn kit_word(voice: DrumVoice) -> &'static str {
    match voice {
        DrumVoice::Kick => "bd",
        DrumVoice::Snare => "sd",
        DrumVoice::Clap => "cp",
        DrumVoice::ClosedHat => "hh",
        DrumVoice::OpenHat => "oh",
        DrumVoice::Ride => "rd",
        DrumVoice::Crash => "cr",
        DrumVoice::Tom => "mt",
        DrumVoice::Shaker => "sh",
        DrumVoice::Cowbell => "cb",
        DrumVoice::Rim | DrumVoice::Snap | DrumVoice::Perc => "rim",
    }
}

/// what `kit` plays for a lane of `voice`, by name, when it has a sound for it
fn on(kit: &str, voice: DrumVoice) -> Option<String> {
    claudius_packs::resolve(kit_word(voice), Some(kit)).map(|r| r.to_string())
}

/// The kits the drums swap through, in order: the genre's that are installed (its likeliest first), then the other
/// installed kits free to publish with, then `SYNTHESIZED`.
pub fn kits(bp: &Blueprint) -> Vec<String> {
    let installed = palette::kits();
    let mut own: Vec<(String, f64)> = genres::get(&bp.genre).map(|g| g.kits).unwrap_or_default();
    own.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut out: Vec<String> = own.into_iter().map(|(k, _)| k).filter(|k| installed.contains(k)).collect();
    for k in installed {
        if !out.contains(&k) {
            out.push(k);
        }
    }
    out.push(SYNTHESIZED.to_string());
    out
}

fn lane_choices(g: Option<&GenreProfile>, d: &DrumLane) -> Vec<Choice> {
    let Some(voice) = lane_voice(g, d) else { return Vec::new() };
    let installed = palette::kits();
    let own: Vec<&String> = g.map_or_else(Vec::new, |g| {
        let mut k: Vec<&(String, f64)> = g.kits.iter().collect();
        k.sort_by(|a, b| b.1.total_cmp(&a.1));
        k.into_iter().map(|(name, _)| name).filter(|k| installed.contains(k)).collect()
    });
    let mut out = Vec::new();
    for kit in &own {
        if let Some(name) = on(kit, voice) {
            push(&mut out, &name, kit, "the genre's kits");
        }
    }
    for kit in installed.iter().filter(|k| !own.contains(k)) {
        if let Some(name) = on(kit, voice) {
            push(&mut out, &name, kit, "other kits");
        }
    }
    let word = voice_word(voice);
    for tuning in 0..4 {
        push(&mut out, &format!("{word}:{tuning}"), SYNTHESIZED, SYNTHESIZED);
    }
    out
}

/// The kit most of the drum lanes play now (`SYNTHESIZED` for their synthesized voices); None without lanes, or when
/// two play as many.
pub fn kit_of(bp: &Blueprint) -> Option<String> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for d in &bp.drums {
        let kit = match &d.sound {
            Sound::Drum { .. } => SYNTHESIZED.to_string(),
            Sound::Instrument { name, .. } => name.split(':').next().unwrap_or_default().to_string(),
            _ => continue,
        };
        match counts.iter_mut().find(|(k, _)| *k == kit) {
            Some((_, n)) => *n += 1,
            None => counts.push((kit, 1)),
        }
    }
    let most = counts.iter().map(|(_, n)| *n).max()?;
    let top: Vec<&String> = counts.iter().filter(|(_, n)| *n == most).map(|(k, _)| k).collect();
    (top.len() == 1).then(|| top[0].clone())
}

/// What each drum lane plays on `kit` (`SYNTHESIZED`: its synthesized voice as it is), by lane: those it has a sound
/// for, and that do not play it already.
pub fn on_kit(bp: &Blueprint, kit: &str) -> Vec<(String, String)> {
    let g = genres::get(&bp.genre).ok();
    bp.drums
        .iter()
        .filter_map(|d| {
            let voice = lane_voice(g.as_ref(), d)?;
            let name = if kit == SYNTHESIZED { format!("{}:0", voice_word(voice)) } else { on(kit, voice)? };
            (lane_playing(d).as_deref() != Some(name.as_str())).then(|| (d.name.clone(), name))
        })
        .collect()
}

/// the kit after the one the drums play now (`kit_of`; the first, when they play none) that changes a lane's sound
pub fn next_kit(bp: &Blueprint) -> Option<String> {
    let list = kits(bp);
    let from = kit_of(bp).and_then(|k| list.iter().position(|x| *x == k)).map_or(0, |i| i + 1);
    (0..list.len()).map(|k| &list[(from + k) % list.len()]).find(|k| !on_kit(bp, k).is_empty()).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compose::Options;

    fn song(genre: &str, seed: u64) -> Blueprint {
        compose::generate(&genres::get(genre).unwrap(), seed, &Options::default(), None)
    }

    /// the sound a part plays for a choice, as `part()` makes it
    fn sound_of(name: &str) -> Sound {
        match presets::get(name) {
            Some(patch) => Sound::Synth { preset: name.to_string(), patch },
            None => Sound::Instrument { name: name.to_string(), label: name.to_string() },
        }
    }

    #[test]
    fn a_part_swaps_first_to_its_genres_own_then_to_what_its_palette_draws() {
        let bp = song("house", 3);
        let bass = bp.parts.iter().find(|p| p.name == "bass").unwrap();
        let list = choices(&bp, "bass").unwrap();
        let g = genres::get("house").unwrap();
        let own: Vec<&str> = g
            .parts
            .iter()
            .find(|s| s.name == "bass")
            .unwrap()
            .presets
            .iter()
            .flat_map(|e| e.split('|'))
            .filter(|a| presets::get(a).is_some())
            .collect();
        assert!(!own.is_empty());
        let groups: Vec<&str> = list.iter().map(|c| c.group).collect();
        assert!(groups.windows(2).all(|w| rank(w[0]) <= rank(w[1])), "the genre's own first, then nearer: {groups:?}");
        assert!(list.iter().take(own.len()).all(|c| own.contains(&c.name.as_str())), "{list:?}");
        // every choice is a sound a bass plays, and none of an era house does not play
        for c in &list {
            assert!(presets::for_role("bass").contains(&c.name.as_str()) || own.contains(&c.name.as_str()), "{c:?}");
            assert!(!presets::DATED.contains(&c.name.as_str()) || own.contains(&c.name.as_str()), "{c:?}");
        }
        assert!(list.iter().all(|c| c.name != "log_drum"), "amapiano's own stays amapiano's");
        assert_eq!(
            playing(&bp, "bass"),
            palette::identity(&bass.sound).map(|i| i.trim_start_matches("synth:").to_string())
        );
    }

    fn rank(group: &str) -> usize {
        ["the genre's own", "fits the genre", "further afield"].iter().position(|g| *g == group).unwrap_or(9)
    }

    #[test]
    fn swapping_again_and_again_goes_round_the_sounds_near_its_genre_that_no_other_part_plays() {
        let mut bp = song("house", 3);
        let list = choices(&bp, "bass").unwrap();
        let others: BTreeSet<String> =
            bp.parts.iter().filter(|p| p.name != "bass").filter_map(|p| palette::identity(&p.sound)).collect();
        let mut heard = Vec::new();
        for _ in 0..list.len() + 2 {
            let c = next(&bp, "bass", &BTreeSet::new()).unwrap();
            assert!(!others.contains(&identity(&c.name)), "{} is another part's", c.name);
            assert_ne!(c.group, AFIELD, "{} is not house's", c.name);
            bp.parts.iter_mut().find(|p| p.name == "bass").unwrap().sound = sound_of(&c.name);
            heard.push(c.name);
        }
        let near = list.iter().filter(|c| c.group != AFIELD && !others.contains(&identity(&c.name))).count();
        let distinct: BTreeSet<&String> = heard.iter().take(near).collect();
        assert_eq!(distinct.len(), near, "each once before any comes round again: {heard:?}");
        assert_eq!(heard[near], heard[0], "and round again");
        // a sound named as taken is passed over too
        let after = next(&bp, "bass", &BTreeSet::new()).unwrap();
        let skip = next(&bp, "bass", &[identity(&after.name)].into()).unwrap();
        assert_ne!(after.name, skip.name);
        // from one further afield, on through those
        let far: Vec<&Choice> = list.iter().filter(|c| c.group == AFIELD).collect();
        bp.parts.iter_mut().find(|p| p.name == "bass").unwrap().sound = sound_of(&far[0].name);
        assert_eq!(next(&bp, "bass", &BTreeSet::new()).unwrap().name, far[1].name);
        // trap's bass is its 808 alone: the next is further afield
        let trap = song("trap", 2);
        let c = next(&trap, "bass", &BTreeSet::new()).unwrap();
        assert_eq!((playing(&trap, "bass").as_deref(), c.group), (Some("808"), AFIELD), "{c:?}");
    }

    #[test]
    fn a_drum_lane_swaps_between_its_kits_and_its_synthesized_voice() {
        let mut bp = song("house", 3);
        let list = choices(&bp, "kick").unwrap();
        // without sample packs: the synthesized voice, as it is and in three other tunings
        let names: Vec<&str> = list.iter().map(|c| c.name.as_str()).collect();
        for tuning in ["bd:0", "bd:1", "bd:2", "bd:3"] {
            assert!(names.contains(&tuning), "{names:?}");
        }
        let kick = bp.drums.iter_mut().find(|d| d.name == "kick").unwrap();
        kick.sound = crate::script::resolve_sound("bd:1", None, None, None, 0, None).unwrap();
        assert_eq!(playing(&bp, "kick").as_deref(), Some("bd:1"));
        assert_eq!(next(&bp, "kick", &BTreeSet::new()).unwrap().name, "bd:2");
        // as a kit: back to the synthesized voices, every lane as it is
        assert_eq!(kits(&bp).last().map(String::as_str), Some(SYNTHESIZED));
        let lanes = on_kit(&bp, SYNTHESIZED);
        assert!(lanes.iter().any(|(l, n)| l == "kick" && n == "bd:0"), "{lanes:?}");
        assert!(choices(&bp, "nope").is_none());
        // a lane on a kit goes back to the voice its genre made it with: house's perc is a rim, not a perc
        let made = genres::get("house").unwrap().drums.iter().find(|s| s.name == "perc").unwrap().voice;
        assert_eq!(made, DrumVoice::Rim);
        let perc = bp.drums.iter_mut().find(|d| d.name == "perc").unwrap();
        perc.sound = Sound::Instrument { name: "tr909:rim".into(), label: "rim".into() };
        let names: Vec<String> = choices(&bp, "perc").unwrap().into_iter().map(|c| c.name).collect();
        assert!(names.contains(&"rim:0".to_string()) && !names.iter().any(|n| n.starts_with("perc:")), "{names:?}");
        assert!(on_kit(&bp, SYNTHESIZED).contains(&("perc".to_string(), "rim:0".to_string())));
    }
}
