//! A song in another genre. The song stays as it is: its form, its harmony and its key, the recordings it plays, the
//! notes written out on its parts and its lead's melody, its pattern tracks and grooves. The genre plays it: its drums,
//! bass and parts, their sounds, its feel and its mix, each section played by the genre's tracks of the kinds that
//! played in it (a kit's kick, backbeat and hats; a part's role).

use std::collections::{BTreeMap, BTreeSet};

use crate::compose::{self, Library, Options};
use crate::genres::GenreProfile;
use crate::model::{Blueprint, LoopPart, Part, PatternTrack, Role, SectionKind, SectionSpec};
use crate::theory::Note;

/// What a drum lane does in a kit, by its name: a section's drums go from one kit to the next by it.
fn kit(lane: &str) -> &'static str {
    match lane {
        "kick" | "808" | "sub" => "kick",
        "snare" | "clap" | "rim" | "snap" => "backbeat",
        "hat" | "ohat" | "hh" | "oh" | "shaker" | "ride" => "hats",
        "crash" => "crash",
        _ => "perc",
    }
}

/// Whether a track of the genre of a kind the song had none of plays in section `s`: its kick and backbeat in a full
/// section, its hats, percussion and parts by the section's energy, as a song like a recording places them
/// (`analysis::song::to_blueprint`).
fn fills(what: &str, s: &SectionSpec) -> bool {
    let quiet = matches!(s.kind, SectionKind::Intro | SectionKind::Outro | SectionKind::Break);
    match what {
        "kick" | "backbeat" => s.energy >= 0.5 && !quiet,
        "hats" | "chords" | "keys" => s.energy >= 0.4,
        "perc" => s.energy >= 0.6,
        "bass" => s.energy >= 0.45,
        "pad" => s.energy < 0.75 || s.kind == SectionKind::Break,
        "lead" => s.energy >= 0.8,
        "arp" => s.energy >= 0.9,
        _ => false,
    }
}

/// The song `bp` in genre `g` (one, or two or three fused), made with `seed`, and what to say of it.
///
/// It keeps the song's title, its form (each section's name, kind, length, energy, progression and transitions, and
/// its drum variant where the genre's drums have it), its harmony and key, the recordings it plays (takes, loops,
/// sampled phrases), its pattern tracks and grooves; the notes written out on its parts, and its lead's melody note
/// for note, go to the genre's part of the same role, or stay on the part itself where the genre has none. The
/// genre's drums, bass, chords, pads and the rest, their sounds, swing and mix play it: in each section its tracks of
/// the kinds that played there, and of kinds the song had none of, where the section's energy calls for them. Its
/// tempo stays when recordings play in it, as they keep time with it; otherwise it is the nearest of the song's, its
/// half and its double that the genre plays.
pub fn change(bp: &Blueprint, g: &GenreProfile, seed: u64, lib: Option<&dyn Library>) -> (Blueprint, String) {
    let bpm = if bp.loops.is_empty() { crate::analysis::song::remake_tempo(bp.bpm, g) } else { bp.bpm };
    let opts = Options { bpm: Some(bpm), key: Some(bp.key), sampled: Some(false), ..Options::default() };
    let mut new = compose::generate(g, seed, &opts, lib);
    new.title.clone_from(&bp.title);
    new.harmony = bp.harmony.clone();
    new.grooves = bp.grooves.clone();
    let mut kept: Vec<String> = Vec::new();

    // A tempo half the song's or less takes its bars two to one, one twice the song's or more one to two, so that the
    // song keeps its length, and where its chords can, its pace: each chord as long as it was, and its melody's notes.
    // A chord held a bar cannot hold half of one, so there each section plays its chords half as many times instead.
    // `span` is the new bars to each of a section's, `time` to each of a chord's (and to each beat of a note).
    let ratio = bp.bpm / bpm;
    let span = if ratio >= 1.5 {
        0.5
    } else if ratio <= 1.0 / 1.5 {
        2.0
    } else {
        1.0
    };
    let time = if span < 1.0 && bp.harmony.bars_per_chord < 2 { 1.0 } else { span };
    let scale = |b: u32, by: f64| ((f64::from(b) * by).round() as u32).max(1);
    new.harmony.bars_per_chord = scale(bp.harmony.bars_per_chord, time);
    let timed = |notes: Vec<Note>| -> Vec<Note> {
        notes.into_iter().map(|n| Note { t: n.t * time, d: n.d * time, ..n }).collect()
    };

    // the notes written out on the song's parts, and its lead's as it plays them, on the genre's part of the same role
    let lead = bp.parts.iter().find(|p| p.role == Role::Lead && p.notes.is_empty()).map(|p| p.name.clone());
    let mut given: BTreeSet<String> = BTreeSet::new();
    let mut carried: Vec<Part> = Vec::new();
    for p in &bp.parts {
        let melody = lead.as_deref() == Some(p.name.as_str());
        if p.notes.is_empty() && !melody {
            continue;
        }
        let notes: BTreeMap<String, Vec<Note>> = match melody {
            true => compose::cycles(bp, p).into_iter().map(|(prog, notes, _, _)| (prog, timed(notes))).collect(),
            false => p.notes.iter().map(|(prog, notes)| (prog.clone(), timed(notes.clone()))).collect(),
        };
        kept.push(if melody { "its lead's melody".to_string() } else { format!("the notes written on {}", p.name) });
        match new.parts.iter_mut().find(|n| n.role == p.role && !given.contains(&n.name)) {
            Some(n) => {
                n.notes = notes;
                given.insert(n.name.clone());
            }
            None => carried.push(Part { notes, ..p.clone() }),
        }
    }

    // its recordings, pattern tracks and the parts the genre has none of their role for stay, by their names, or
    // another where one of the genre's tracks has it
    let mut taken: BTreeSet<String> = new
        .drums
        .iter()
        .map(|d| d.name.clone())
        .chain(new.parts.iter().map(|p| p.name.clone()))
        .chain(new.loops.iter().map(|l| l.name.clone()))
        .chain(new.patterns.iter().map(|t| t.name.clone()))
        .collect();
    let mut renamed: BTreeMap<String, String> = BTreeMap::new();
    let mut keep = |name: &str| -> String {
        let mut n = name.to_string();
        let mut k = 2;
        while taken.contains(&n) {
            n = format!("{name}{k}");
            k += 1;
        }
        taken.insert(n.clone());
        renamed.insert(name.to_string(), n.clone());
        n
    };
    let loops: Vec<LoopPart> = bp.loops.iter().map(|l| LoopPart { name: keep(&l.name), ..l.clone() }).collect();
    let patterns: Vec<PatternTrack> =
        bp.patterns.iter().map(|t| PatternTrack { name: keep(&t.name), ..t.clone() }).collect();
    let carried: Vec<Part> = carried.into_iter().map(|p| Part { name: keep(&p.name), ..p }).collect();
    if !loops.is_empty() {
        let names: Vec<&str> = loops.iter().map(|l| l.name.as_str()).collect();
        kept.push(format!("the recordings it plays ({})", names.join(", ")));
    }

    // what played in each section of the song, by kind: a drum lane by what it does in its kit, a part by its role
    let kind_of = |name: &str| -> Option<&'static str> {
        match bp.drums.iter().any(|d| d.name == name) {
            true => Some(kit(name)),
            false => bp.parts.iter().find(|p| p.name == name).map(|p| p.role.name()),
        }
    };
    let ever: BTreeSet<&str> = bp.form.iter().flat_map(|s| &s.layers).filter_map(|l| kind_of(l)).collect();
    let variants: BTreeSet<&String> = new.drums.iter().flat_map(|d| d.patterns.keys()).collect();
    let form: Vec<SectionSpec> = bp
        .form
        .iter()
        .map(|s| {
            let here: BTreeSet<&str> = s.layers.iter().filter_map(|l| kind_of(l)).collect();
            let plays = |what: &str| if ever.contains(what) { here.contains(what) } else { fills(what, s) };
            let mut layers: Vec<String> =
                new.drums.iter().filter(|d| plays(kit(&d.name))).map(|d| d.name.clone()).collect();
            layers.extend(new.parts.iter().filter(|p| plays(p.role.name())).map(|p| p.name.clone()));
            layers.extend(s.layers.iter().filter_map(|l| renamed.get(l).cloned()));
            if layers.is_empty() {
                layers.extend(new.parts.first().map(|p| p.name.clone()));
            }
            let variant = if variants.contains(&s.variant) { s.variant.clone() } else { "main".to_string() };
            SectionSpec { layers, variant, bars: scale(s.bars, span), ..s.clone() }
        })
        .collect();
    new.form = form;
    new.loops.extend(loops);
    new.patterns.extend(patterns);
    new.parts.extend(carried);

    let named = |genre: &str| genre.replace('_', " ");
    let tempo = if (bpm - bp.bpm).abs() < 1e-6 {
        String::new()
    } else if time < 1.0 {
        format!(" at {bpm:.0} BPM in half time (it was {:.0}, two of its bars to each)", bp.bpm)
    } else if span < 1.0 {
        format!(" at {bpm:.0} BPM (it was {:.0}), each section playing its chords half as many times", bp.bpm)
    } else if span > 1.0 {
        format!(" at {bpm:.0} BPM in double time (it was {:.0}, each of its bars two)", bp.bpm)
    } else {
        format!(" at {bpm:.0} BPM (it was {:.0})", bp.bpm)
    };
    let with = if kept.is_empty() { String::new() } else { format!(", with {}", kept.join(", ")) };
    let said = format!(
        "changed from {} to {}{tempo}: its form, chords and key kept{with}; {}'s drums, bass, parts and sounds play it \
         (seed {seed})",
        named(&bp.genre),
        named(&g.name),
        named(&g.name)
    );
    new.lineage = bp.lineage.clone();
    new.lineage.push(said.clone());
    (new, said)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genres;
    use crate::model::Sound;
    use crate::theory::Note;

    /// the form as a song keeps it: each section's name, kind, progression and transitions
    fn form(bp: &Blueprint) -> Vec<(String, SectionKind, String, bool, bool, bool)> {
        bp.form.iter().map(|s| (s.name.clone(), s.kind, s.prog.clone(), s.build, s.gap, s.impact)).collect()
    }

    /// its lead's notes, cycle by cycle, their beats times `time`
    fn melody(bp: &Blueprint, time: f64) -> Vec<(String, Vec<Note>)> {
        let lead = bp.parts.iter().find(|p| p.role == Role::Lead).expect("a lead");
        let timed = |n: &Note| Note { t: n.t * time, d: n.d * time, ..*n };
        compose::cycles(bp, lead)
            .into_iter()
            .map(|(prog, notes, _, _)| (prog, notes.iter().map(timed).collect()))
            .collect()
    }

    /// how long a song is, in seconds
    fn length(bp: &Blueprint) -> f64 {
        bp.form.iter().map(|s| f64::from(s.bars) * 240.0 / bp.bpm).sum()
    }

    #[test]
    fn a_song_in_another_genre_keeps_its_form_chords_key_and_melody_and_the_genre_plays_the_rest() {
        let (house, lofi) = (genres::get("house").unwrap(), genres::get("lofi").unwrap());
        let bp = compose::generate(&house, 7, &Options::default(), None);
        let (new, said) = change(&bp, &lofi, 3, None);
        assert_eq!((new.genre.as_str(), &new.title, new.key), ("lofi", &bp.title, bp.key));
        assert_eq!(new.harmony.progressions, bp.harmony.progressions);
        assert_eq!(form(&new), form(&bp));
        assert!((lofi.bpm.0..=lofi.bpm.1).contains(&new.bpm), "{} BPM", new.bpm);
        // at under half its tempo, half as many bars: as long as it was, near enough
        assert!(new.form.iter().zip(&bp.form).all(|(n, o)| n.bars == (o.bars as f64 / 2.0).round().max(1.0) as u32));
        assert!((length(&new) / length(&bp) - 1.0).abs() < 0.2, "{} s, it was {} s", length(&new), length(&bp));
        let time = f64::from(new.harmony.bars_per_chord) / f64::from(bp.harmony.bars_per_chord);
        assert_eq!(melody(&new, 1.0), melody(&bp, time), "its lead's melody, note for note, in time with its chords");
        // lofi's own drums, on its own sounds, where the song had drums
        let opts = Options { bpm: Some(new.bpm), key: Some(bp.key), sampled: Some(false), ..Options::default() };
        let own = compose::generate(&lofi, 3, &opts, None);
        let kit = |b: &Blueprint| b.drums.iter().map(|d| (d.name.clone(), d.sound.clone())).collect::<Vec<_>>();
        assert_eq!(kit(&new), kit(&own));
        for (old, now) in bp.form.iter().zip(&new.form) {
            let drums = |s: &SectionSpec, b: &Blueprint| s.layers.iter().any(|l| b.drums.iter().any(|d| &d.name == l));
            assert_eq!(drums(now, &new), drums(old, &bp), "{}", old.name);
        }
        assert!(said.starts_with("changed from house to lofi") && said.contains("its lead's melody"), "{said}");
        assert_eq!(new.lineage.last(), Some(&said));
    }

    #[test]
    fn a_song_at_twice_its_tempo_plays_each_of_its_bars_as_two_and_its_chords_as_long() {
        let (lofi, dnb) = (genres::get("lofi").unwrap(), genres::get("drum_and_bass").unwrap());
        let bp = compose::generate(&lofi, 5, &Options::default(), None);
        let (new, said) = change(&bp, &dnb, 2, None);
        assert!(new.bpm >= 1.5 * bp.bpm, "{} BPM, it was {}", new.bpm, bp.bpm);
        assert_eq!(new.harmony.bars_per_chord, 2 * bp.harmony.bars_per_chord);
        assert!(new.form.iter().zip(&bp.form).all(|(n, o)| n.bars == 2 * o.bars));
        assert!((length(&new) / length(&bp) - 1.0).abs() < 0.2, "{} s, it was {} s", length(&new), length(&bp));
        assert_eq!(melody(&new, 1.0), melody(&bp, 2.0));
        assert!(said.contains("in double time"), "{said}");
    }

    #[test]
    fn a_song_with_recordings_keeps_them_where_they_play_and_keeps_its_tempo() {
        let (house, dnb) = (genres::get("house").unwrap(), genres::get("drum_and_bass").unwrap());
        let mut bp = compose::generate(&house, 9, &Options::default(), None);
        let from = bp.form[1].name.clone();
        bp.loops.push(LoopPart {
            name: "vox1".into(),
            role: "vocal".into(),
            sound: Sound::Loop {
                path: "/x/vox.wav".into(),
                bpm: bp.bpm,
                beats: 8.0,
                key: None,
                tonal: true,
                label: String::new(),
            },
            transpose: 0,
            chop: Vec::new(),
            slice_beats: 0.0,
            retime: false,
            mix: Default::default(),
            from: Some(from.clone()),
            sampled: false,
        });
        bp.form[1].layers.push("vox1".into());
        let (new, said) = change(&bp, &dnb, 1, None);
        assert_eq!(new.bpm, bp.bpm, "a recording keeps time with the song");
        assert!(new.loops.iter().any(|l| l.name == "vox1" && l.from.as_deref() == Some(from.as_str())));
        let plays = |i: usize| new.form[i].layers.iter().any(|l| l == "vox1");
        assert!(plays(1) && !plays(0), "where it played");
        assert!(said.contains("the recordings it plays (vox1)"), "{said}");
    }

    #[test]
    fn a_song_goes_into_a_fusion_and_the_notes_written_on_it_go_with_it() {
        let (house, fused) = (genres::get("house").unwrap(), genres::get("trip_hop+idm").unwrap());
        let mut bp = compose::generate(&house, 11, &Options::default(), None);
        let bass = bp.parts.iter().position(|p| p.role == Role::Bass).expect("a bass");
        let written: BTreeMap<String, Vec<Note>> =
            [("A".to_string(), vec![Note { t: 0.0, d: 1.0, p: 36, v: 0.9, lock: None }])].into();
        bp.parts[bass].notes = written.clone();
        let (new, said) = change(&bp, &fused, 2, None);
        assert_eq!(new.genre, fused.name);
        assert!(new.parts.iter().any(|p| p.role == Role::Bass && p.notes == written), "on the genre's bass");
        assert!(said.contains(&format!("the notes written on {}", bp.parts[bass].name)), "{said}");
    }
}
