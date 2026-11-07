//! A track written the way a genre plays it, into any song: a part of a role as the genre's own part of it is written
//! (its idiom, a sound from the genre's palette, its register, density, voicing and mix), or the genre's drums from
//! its lane templates and kits; added to the song, or in place of one of its tracks. And another take of a track, or
//! a variation of the take it plays, which keeps its rhythm and changes the details.

use std::collections::BTreeSet;

use crate::compose;
use crate::genres::{self, GenreProfile, LaneSpec, PartSpec};
use crate::model::*;
use crate::palette;
use crate::rng::{Rng, hash_str};
use crate::synth::drums::DrumVoice;
use crate::theory::rhythm::Hit;

/// what a track may be generated as
pub const WHAT: &[&str] = &["drums", "percussion", "bass", "chords", "keys", "pad", "strings", "lead", "arp"];

/// a lane a percussion track plays: none of the kit's kick, snare, clap or hats
fn percussive(lane: &LaneSpec) -> bool {
    matches!(
        lane.voice,
        DrumVoice::Shaker | DrumVoice::Perc | DrumVoice::Rim | DrumVoice::Cowbell | DrumVoice::Snap | DrumVoice::Tom
    )
}

/// a name of the song's own made from `base`: itself, or with the first number after it that is free
fn fresh(bp: &Blueprint, base: &str) -> String {
    let taken = bp.layer_names();
    std::iter::once(base.to_string())
        .chain((2..).map(|k| format!("{base}{k}")))
        .find(|n| !taken.contains(n))
        .unwrap_or_default()
}

/// Track `what` (one of `WHAT`) written as genre `genre` plays it into `bp`, from `seed`: a part added as `name`
/// (made from what it is when left out) or in place of the song's part `replace`; the genre's drums in place of the
/// song's lanes of their names and beside the others; its percussion beside the song's drums. What it did, or why
/// not.
pub fn track(
    bp: &mut Blueprint,
    what: &str,
    genre: &str,
    replace: Option<&str>,
    name: Option<&str>,
    seed: u64,
) -> Result<String, String> {
    let g = genres::get(genre).map_err(|e| e.to_string())?;
    match what {
        "drums" | "percussion" => drums(bp, &g, what == "percussion", seed),
        _ if WHAT.contains(&what) => part(bp, &g, what, replace, name, seed),
        other => Err(format!("a generated track is one of {}, not '{other}'", WHAT.join(" "))),
    }
}

/// the part a genre would write for `what`: one of its own parts of the role (its strings, for strings), or one its
/// palette draws for the role
fn spec(g: &GenreProfile, what: &str, rng: &mut Rng) -> PartSpec {
    let strings = what == "strings";
    let role = if strings { Role::Pad } else { Role::parse(what).unwrap_or(Role::Lead) };
    let stringy = |p: &&PartSpec| p.name.contains("string") || p.presets.iter().any(|e| e.contains("string"));
    let own: Vec<&PartSpec> = g.parts.iter().filter(|p| if strings { stringy(p) } else { p.role == role }).collect();
    let mut spec = match own.len() {
        0 => compose::spec_for(g, what, role, if strings { "string_ensemble|strings" } else { "pluck" }),
        n => own[rng.below(n)].clone(),
    };
    // strings play strings: the genre's own, or the string ensemble
    if strings {
        spec.presets.retain(|e| e.contains("string"));
        if spec.presets.is_empty() {
            spec.presets = vec!["string_ensemble|strings".into()];
        }
        spec.core = Some(1.0);
    }
    spec
}

fn part(
    bp: &mut Blueprint,
    g: &GenreProfile,
    what: &str,
    replace: Option<&str>,
    name: Option<&str>,
    seed: u64,
) -> Result<String, String> {
    let mut rng = Rng::new(seed ^ hash_str(&g.name) ^ hash_str(what));
    let mut spec = spec(g, what, &mut rng);
    let role = spec.role;
    if let Some(old) = replace
        && !bp.parts.iter().any(|p| p.name == old)
    {
        let parts: Vec<&str> = bp.parts.iter().map(|p| p.name.as_str()).collect();
        return Err(format!("no part '{old}' to write again (parts: {})", parts.join(" ")));
    }
    let base = if g.name == bp.genre { what.to_string() } else { format!("{}_{what}", g.name) };
    let name = match (replace, name.map(str::trim).filter(|n| !n.is_empty())) {
        (Some(old), _) => old.to_string(),
        (None, Some(n)) if bp.layer_names().iter().any(|l| l == n) => {
            return Err(format!("the song already has a track '{n}': give the new one another name"));
        }
        (None, Some(n)) => n.to_string(),
        (None, None) => fresh(bp, &base),
    };
    spec.name = name.clone();
    let mut used: BTreeSet<String> =
        bp.parts.iter().filter(|p| p.name != name).filter_map(|p| palette::identity(&p.sound)).collect();
    let made = compose::new_part(&spec, g, &palette::instruments(), &mut used, &mut rng);
    let said = format!(
        "{name}: {}'s {what}, {} on {}",
        g.name,
        crate::theory::bass::idiom(&made.style).map_or(made.style.clone(), |i| format!("{} ({})", i.name, i.about)),
        made.sound.describe()
    );
    match bp.parts.iter_mut().find(|p| p.name == name) {
        // the genre's writing in its place; its name, mix and sections stay, and what it played of another part goes
        Some(p) => {
            *p = Part { name: p.name.clone(), mix: p.mix.clone(), doubles: None, harmonizes: None, ..made };
        }
        None => {
            // where the song plays a part of the role, or as a new track goes in
            let with: Vec<String> = bp.parts.iter().filter(|p| p.role == role).map(|p| p.name.clone()).collect();
            bp.parts.push(Part { name: name.clone(), ..made });
            if with.is_empty() {
                crate::script::place_new(bp, &name);
            } else {
                for s in bp.form.iter_mut().filter(|s| s.layers.iter().any(|l| with.contains(l))) {
                    s.layers.push(name.clone());
                }
            }
        }
    }
    Ok(said)
}

fn drums(bp: &mut Blueprint, g: &GenreProfile, percussion: bool, seed: u64) -> Result<String, String> {
    let lanes: Vec<&LaneSpec> = g.drums.iter().filter(|l| !percussion || percussive(l)).collect();
    if lanes.is_empty() {
        return Err(format!("{} plays no {}", g.name, if percussion { "percussion" } else { "drums" }));
    }
    let rng = Rng::new(seed ^ hash_str(&g.name) ^ hash_str(if percussion { "percussion" } else { "drums" }));
    let kit = compose::pick_kit(&g.kits, &mut rng.fork("kit"));
    let kits = palette::kits();
    let sounds = compose::LaneSounds { samples: 0.0, lib: None, kit: kit.as_deref(), kits: &kits };
    let mut dr = rng.fork("drums");
    // the sections the song's drums play in, which a lane of its own joins
    let drummed: Vec<usize> = (0..bp.form.len())
        .filter(|&i| bp.form[i].layers.iter().any(|l| bp.drums.iter().any(|d| &d.name == l)))
        .collect();
    let (mut rewritten, mut added) = (Vec::new(), Vec::new());
    for spec in lanes {
        let lane = compose::write_lane(spec, g, &bp.key, &sounds, &mut dr);
        match bp.drums.iter_mut().find(|d| d.name == lane.name) {
            // its patterns, sound and feel in the song's lane of the name; its mix stays
            Some(d) if !percussion => {
                *d = DrumLane { mix: d.mix.clone(), ..lane };
                rewritten.push(d.name.clone());
            }
            found => {
                let name =
                    if found.is_some() { fresh(bp, &format!("{}_{}", g.name, lane.name)) } else { lane.name.clone() };
                bp.drums.push(DrumLane { name: name.clone(), ..lane });
                if drummed.is_empty() {
                    crate::script::place_new(bp, &name);
                } else {
                    for &i in &drummed {
                        bp.form[i].layers.push(name.clone());
                    }
                }
                added.push(name);
            }
        }
    }
    let on = kit.map(|k| format!(", on {k}")).unwrap_or_default();
    let mut said = vec![format!("{}'s {}{on}", g.name, if percussion { "percussion" } else { "drums" })];
    if !rewritten.is_empty() {
        said.push(format!("written again: {}", rewritten.join(" ")));
    }
    if !added.is_empty() {
        said.push(format!("added: {}", added.join(" ")));
    }
    Ok(said.join("; "))
}

/// the high half of a part's seed: what draws its idiom's cells, which a variation keeps
const TAKE: u64 = 0xffff_ffff_0000_0000;

/// Another take of track `track` of `bp` (`take`), or a variation of the take it plays, from `seed`: a part's seed
/// anew, or only its low half, which keeps its idiom's cells and draws their details again; a drum lane's patterns
/// written again from its genre's template of the lane, or varied a step or two with its strong hits kept.
pub fn vary(bp: &mut Blueprint, track: &str, take: bool, seed: u64) -> Result<String, String> {
    let mut rng = Rng::new(seed ^ hash_str(track));
    if let Some(p) = bp.parts.iter_mut().find(|p| p.name == track) {
        let new = rng.next_u64();
        p.seed = if take { new } else { p.seed & TAKE | new & !TAKE };
        let written = if p.notes.is_empty() { "" } else { " (the notes written out stay)" };
        return Ok(if take {
            format!("{track}: another take{written}")
        } else {
            format!("{track}: a variation of its take{written}")
        });
    }
    let genre = genres::get(&bp.genre).ok();
    let Some(d) = bp.drums.iter_mut().find(|d| d.name == track) else {
        let tracks: Vec<String> =
            bp.parts.iter().map(|p| p.name.clone()).chain(bp.drums.iter().map(|d| d.name.clone())).collect();
        return Err(format!("no part or drum lane '{track}' to vary (they are: {})", tracks.join(" ")));
    };
    if take {
        let spec = genre.as_ref().and_then(|g| g.drums.iter().find(|l| l.name == d.name)).ok_or_else(|| {
            format!("{} has no lane '{track}' to take again: generate drums in a genre that plays one", bp.genre)
        })?;
        d.patterns = compose::lane_patterns(spec, d.pattern_bars, &mut rng);
        return Ok(format!("{track}: another take, as {} plays it", bp.genre));
    }
    let bars = d.pattern_bars;
    for hits in d.patterns.values_mut() {
        *hits = varied(hits, bars, &mut rng);
    }
    Ok(format!("{track}: a variation of its patterns, its strong hits kept"))
}

/// `hits` (a pattern `bars` long) varied a step or two: a hit off the strong beats moved a sixteenth, a ghost note
/// added beside a hit, or a hit off the strong beats left out; the hits on a beat played hard stay where they are
fn varied(hits: &[Hit], bars: u32, rng: &mut Rng) -> Vec<Hit> {
    let len = bars.max(1) as f64 * 4.0;
    let mut out = hits.to_vec();
    let strong = |h: &Hit| (h.t - h.t.round()).abs() < 1e-6 && h.v >= 0.85;
    let free = |out: &[Hit], t: f64| t >= 0.0 && t < len - 1e-9 && !out.iter().any(|h| (h.t - t).abs() < 1e-6);
    for _ in 0..1 + rng.below(2) {
        let weak: Vec<usize> = (0..out.len()).filter(|&i| !strong(&out[i])).collect();
        match rng.below(3) {
            0 if !weak.is_empty() => {
                let i = weak[rng.below(weak.len())];
                let t = out[i].t + if rng.chance(0.5) { 0.25 } else { -0.25 };
                if free(&out, t) {
                    out[i].t = t;
                }
            }
            1 if !out.is_empty() => {
                let by = out[rng.below(out.len())].t;
                let t = by + if rng.chance(0.5) { 0.25 } else { -0.25 };
                if free(&out, t) {
                    out.push(Hit::new(t, rng.range(0.3, 0.45) as f32));
                }
            }
            _ if weak.len() > 1 => {
                out.remove(weak[rng.below(weak.len())]);
            }
            _ => {}
        }
    }
    out.sort_by(|a, b| a.t.total_cmp(&b.t));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compose::Options;

    fn song(genre: &str, seed: u64) -> Blueprint {
        compose::generate(&genres::get(genre).unwrap(), seed, &Options::default(), None)
    }

    #[test]
    fn a_part_of_another_genre_is_written_as_that_genre_writes_it_and_plays_where_the_songs_of_its_role_do() {
        let mut bp = song("lofi", 3);
        let said = track(&mut bp, "bass", "reggae", None, None, 9).unwrap();
        let bass = bp.parts.iter().find(|p| p.name == "reggae_bass").expect(&said);
        let reggae = genres::get("reggae").unwrap();
        let theirs = reggae.parts.iter().find(|p| p.role == Role::Bass).unwrap();
        assert!(theirs.styles.contains(&bass.style) && bass.register == theirs.register, "{said}");
        assert!(said.starts_with("reggae_bass: reggae's bass, ") && said.contains(&bass.style), "{said}");
        for s in &bp.form {
            assert_eq!(
                s.layers.contains(&"reggae_bass".to_string()),
                s.layers.contains(&"bass".to_string()),
                "{}",
                s.name
            );
        }
        // in place of the song's bass: its name, mix and sections kept, the genre's writing in it
        let mix = bp.parts.iter().find(|p| p.name == "bass").unwrap().mix.clone();
        track(&mut bp, "bass", "funk", Some("bass"), None, 4).unwrap();
        let bass = bp.parts.iter().find(|p| p.name == "bass").unwrap();
        let funk = genres::get("funk").unwrap();
        assert!(funk.parts.iter().find(|p| p.role == Role::Bass).unwrap().styles.contains(&bass.style));
        assert_eq!(bass.mix, mix);
        assert!(track(&mut bp, "bass", "funk", Some("nope"), None, 4).unwrap_err().contains("no part 'nope'"));
        assert!(track(&mut bp, "tuba", "funk", None, None, 4).unwrap_err().contains("one of drums percussion bass"));
    }

    #[test]
    fn strings_are_a_genres_string_part_or_a_pad_on_strings() {
        let mut bp = song("house", 3);
        track(&mut bp, "strings", "g_funk", None, None, 1).unwrap();
        let s = bp.parts.iter().find(|p| p.name == "g_funk_strings").unwrap();
        assert_eq!(s.role, Role::Pad);
        track(&mut bp, "strings", "techno", None, None, 1).unwrap();
        let t = bp.parts.iter().find(|p| p.name == "techno_strings").unwrap();
        assert!(t.sound.describe().contains("string"), "{}", t.sound.describe());
    }

    #[test]
    fn a_genres_drums_take_the_place_of_the_songs_lanes_of_their_names_and_join_the_others() {
        let mut bp = song("house", 3);
        let reggaeton = genres::get("reggaeton").unwrap();
        let kick_mix = bp.drums.iter().find(|d| d.name == "kick").unwrap().mix.clone();
        let said = track(&mut bp, "drums", "reggaeton", None, None, 2).unwrap();
        assert!(said.starts_with("reggaeton's drums") && said.contains("written again: kick"), "{said}");
        let snare = bp.drums.iter().find(|d| d.name == "snare").expect("house has no snare: reggaeton's is added");
        assert!(!snare.patterns["main"].is_empty());
        // the dembow's snare: on the tresillo's offbeats, as reggaeton's template has it
        let template = &reggaeton.drums.iter().find(|l| l.name == "snare").unwrap().template.as_ref().unwrap().steps;
        for h in &snare.patterns["main"] {
            let step = ((h.t * 4.0).round() as usize) % template.len();
            assert!(template[step] > 0.0, "a snare on step {step}, where reggaeton plays none");
        }
        assert_eq!(bp.drums.iter().find(|d| d.name == "kick").unwrap().mix, kick_mix, "the song's mix stays");
        let kicked: Vec<&str> =
            bp.form.iter().filter(|s| s.layers.iter().any(|l| l == "kick")).map(|s| s.name.as_str()).collect();
        let snared: Vec<&str> =
            bp.form.iter().filter(|s| s.layers.iter().any(|l| l == "snare")).map(|s| s.name.as_str()).collect();
        assert!(
            snared.iter().all(|s| bp.form.iter().any(|f| f.name == *s))
                && !snared.is_empty()
                && kicked.len() >= snared.len()
        );
        // percussion goes in beside them, a lane of a name the song has getting the genre's name
        let said = track(&mut bp, "percussion", "afrobeats", None, None, 2).unwrap();
        assert!(said.contains("added:"), "{said}");
        assert!(
            track(&mut bp, "percussion", "synthwave", None, None, 2)
                .unwrap_err()
                .contains("synthwave plays no percussion")
        );
    }

    #[test]
    fn a_variation_keeps_a_parts_take_and_a_lanes_strong_hits() {
        let mut bp = song("reggae", 3);
        let before = bp.parts.iter().find(|p| p.name == "bass").unwrap().seed;
        vary(&mut bp, "bass", false, 5).unwrap();
        let after = bp.parts.iter().find(|p| p.name == "bass").unwrap().seed;
        assert!(after != before && after & TAKE == before & TAKE, "the same take, other details");
        vary(&mut bp, "bass", true, 6).unwrap();
        assert_ne!(bp.parts.iter().find(|p| p.name == "bass").unwrap().seed & TAKE, before & TAKE);
        let kick = |bp: &Blueprint| bp.drums.iter().find(|d| d.name == "kick").unwrap().patterns["main"].clone();
        let strong: Vec<f64> =
            kick(&bp).iter().filter(|h| (h.t - h.t.round()).abs() < 1e-6 && h.v >= 0.85).map(|h| h.t).collect();
        for seed in 1..10 {
            vary(&mut bp, "kick", false, seed).unwrap();
            let now = kick(&bp);
            assert!(strong.iter().all(|t| now.iter().any(|h| (h.t - t).abs() < 1e-6)), "{now:?}");
        }
        assert!(vary(&mut bp, "kick", true, 3).unwrap().contains("as reggae plays it"));
        assert!(vary(&mut bp, "nope", false, 3).unwrap_err().contains("no part or drum lane 'nope'"));
    }
}
