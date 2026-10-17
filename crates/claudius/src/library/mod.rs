//! The sample library: recorded sounds the composer can use instead of (or next to) synthesis. Today: Apple Loops
//! (when GarageBand / Logic content is installed), from which it also cuts drum one-shots; user folders and chops
//! of imported songs land in the same index.

pub mod apple;
pub mod chop;
pub mod oneshot;

use std::path::PathBuf;

use anyhow::Result;

use crate::compose;
use crate::genres::{GenreProfile, LoopSpec};
use crate::model::Sound;
use crate::rng::Rng;
use crate::theory::scale::Key;
use apple::LoopInfo;

pub struct Library {
    pub loops: Vec<LoopInfo>,
}

/// GarageBand genre names that suit each claudius genre (one-shots and loops are drawn from these first)
pub fn apple_genres(genre: &str) -> &'static [&'static str] {
    match genre {
        "house" => &["House", "Deep House", "Tech House", "Bass House", "Electro House", "Disco Funk"],
        "deep_house" => &["Deep House", "House", "Chillwave", "Tech House"],
        "techno" => &["Tech House", "Electronic/Dance", "Electro House", "Bass House"],
        "trap" => &["Hip Hop", "Hip Hop/RnB", "Modern RnB", "Urban"],
        "lofi" => &["Hip Hop", "Hip Hop/RnB", "Chillwave", "Vintage Breaks", "Jazz"],
        "dnb" => &["Electronic/Dance", "Dubstep", "Vintage Breaks", "Electro House"],
        "future_bass" => &["Future Bass", "Electronic Pop", "Dubstep", "Modern RnB"],
        "synthwave" => &["Electronic Pop", "Chillwave", "Indie", "Electronic/Dance"],
        _ => &[],
    }
}

fn user_index() -> PathBuf {
    crate::config::cache_dir("user").join("library.json")
}

/// where a recording comes from, as labels say it: Apple Loops, or your own library (chops, your folders)
fn source(path: &str) -> &'static str {
    if crate::config::APPLE_LOOP_DIRS.iter().any(|d| std::path::Path::new(path).starts_with(d)) {
        "Apple Loops"
    } else {
        "your library"
    }
}

/// the user library: chops of imported songs and their one-shots
pub fn user_entries() -> Vec<LoopInfo> {
    std::fs::read_to_string(user_index()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn user_index_add(entries: &[LoopInfo]) -> Result<()> {
    let mut all = user_entries();
    all.retain(|e| !entries.iter().any(|n| n.path == e.path));
    all.extend(entries.iter().cloned());
    std::fs::write(user_index(), serde_json::to_string_pretty(&all)?)?;
    Ok(())
}

impl Library {
    /// the installed Apple Loops plus the user library (empty when neither exists)
    pub fn open() -> Result<Library> {
        let mut loops = match crate::config::apple_loops_db() {
            Some(db) => apple::load(&db)?,
            None => Vec::new(),
        };
        loops.extend(user_entries().into_iter().filter(|e| std::path::Path::new(&e.path).exists()));
        Ok(Library { loops })
    }

    pub fn is_empty(&self) -> bool {
        self.loops.is_empty()
    }

    fn matches_hint(l: &LoopInfo, hint: &str) -> bool {
        match hint.strip_prefix("name:") {
            Some(word) => l.name.to_lowercase().contains(&word.to_lowercase()),
            None => l.subtype.eq_ignore_ascii_case(hint),
        }
    }
}

const PART_WORDS: &[&str] = &[
    "beat",
    "beats",
    "bass",
    "synth",
    "chord",
    "chords",
    "layers",
    "piano",
    "keys",
    "lead",
    "pad",
    "kick",
    "snare",
    "hat",
    "hi-hat",
    "hihat",
    "clap",
    "claps",
    "perc",
    "percussion",
    "topper",
    "riser",
    "vocal",
    "vox",
    "guitar",
    "pluck",
    "plucks",
    "arp",
    "strings",
    "organ",
    "electric",
    "rhodes",
    "fx",
    "drums",
    "groove",
    "melody",
    "stab",
    "stabs",
    "sub",
    "line",
    "hook",
    "brass",
    "horn",
    "horns",
    "bell",
    "bells",
    "keyboard",
    "wurli",
    "clav",
    "marimba",
    "flute",
    "sax",
    "trumpet",
    "choir",
    "harp",
    "mallets",
    "chop",
    "chops",
    "texture",
    "atmosphere",
    "sequence",
    "loop",
];

/// the family part of a loop name: the words before the first instrument word ("Bliss Abyss Chord Layers 01")
pub fn family_prefix(name: &str) -> String {
    let mut out = Vec::new();
    for w in name.split_whitespace() {
        let lw = w.to_lowercase();
        if PART_WORDS.contains(&lw.as_str()) || lw.chars().all(|c| c.is_ascii_digit()) {
            break;
        }
        out.push(w);
    }
    out.join(" ")
}

fn harmonic(l: &LoopInfo) -> bool {
    l.key.is_some()
        && matches!(l.instrument.as_str(), "Keyboards" | "Guitars" | "Strings" | "Mallets" | "Horn/Wind" | "Mixed")
}

/// the chords of a harmony loop, one Roman numeral per bar, read from its chroma (bass chroma from the family's bass
/// loop when there is one)
fn read_progression(chords: &LoopInfo, bass: Option<&LoopInfo>, key: &Key) -> Option<Vec<String>> {
    use crate::analysis::chroma;
    use crate::config::SRF;
    let x = crate::audio::load(std::path::Path::new(&chords.path)).ok()?;
    let b = bass.and_then(|b| crate::audio::load(std::path::Path::new(&b.path)).ok());
    let bar = 4.0 * 60.0 / chords.tempo;
    let bars = ((chords.beats / 4.0).round() as usize).max(1);
    let mut out = Vec::new();
    for i in 0..bars {
        let (s0, s1) = (((i as f64 * bar) * SRF) as usize, ((((i + 1) as f64) * bar * SRF) as usize).min(x.len()));
        if s1 <= s0 + 8192 {
            break;
        }
        let avg = |frames: Vec<[f32; 12]>| -> [f32; 12] {
            let mut a = [0.0f32; 12];
            for f in &frames {
                for k in 0..12 {
                    a[k] += f[k];
                }
            }
            let s: f32 = a.iter().sum::<f32>().max(1e-9);
            a.map(|v| v / s)
        };
        let c = avg(chroma::chroma(&x.slice(s0, s1).mono(), 4096));
        let bc = match &b {
            Some(bx) if bx.len() > s1 => avg(chroma::chroma_band(&bx.slice(s0, s1).mono(), 4096, 30.0, 300.0)),
            _ => avg(chroma::chroma_band(&x.slice(s0, s1).mono(), 4096, 60.0, 300.0)),
        };
        let (ch, _) = chroma::chord_with_bass(&c, &bc, Some(key));
        out.push(crate::theory::chord::roman_of(&ch, key));
    }
    if out.is_empty() { None } else { Some(out) }
}

impl compose::Library for Library {
    fn family(&self, genre: &GenreProfile, rng: &mut Rng) -> Option<compose::FamilyPick> {
        use std::collections::BTreeMap;
        let pref = apple_genres(&genre.name);
        let (lo, hi) = (genre.bpm.0 * 0.92, genre.bpm.1 * 1.08);
        let mut groups: BTreeMap<(String, i64, String), Vec<&LoopInfo>> = BTreeMap::new();
        for l in &self.loops {
            if l.genre == "User" || l.tempo <= 0.0 || l.beats < 8.0 || l.name.contains("Fill") {
                continue;
            }
            let prefix = family_prefix(&l.name);
            if prefix.is_empty() {
                continue;
            }
            let dir =
                std::path::Path::new(&l.path).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
            groups.entry((dir, (l.tempo * 10.0).round() as i64, prefix)).or_default().push(l);
        }
        let mut cands: Vec<(&(String, i64, String), &Vec<&LoopInfo>, f64)> = Vec::new();
        for (k, members) in &groups {
            let tempo = k.1 as f64 / 10.0;
            if tempo < lo || tempo > hi || !members.iter().any(|l| harmonic(l)) {
                continue;
            }
            let mut w = 1.0;
            if members.iter().any(|l| l.instrument == "Bass") {
                w *= 3.0;
            }
            if members.iter().any(|l| pref.contains(&l.genre.as_str())) {
                w *= 3.0;
            }
            cands.push((k, members, w));
        }
        if cands.is_empty() {
            return None;
        }
        let w: Vec<f64> = cands.iter().map(|c| c.2).collect();
        let (k, members, _) = cands[rng.weighted(&w)];
        let mut harm: Vec<&LoopInfo> = members.iter().copied().filter(|l| harmonic(l)).collect();
        harm.sort_by(|a, b| a.name.cmp(&b.name));
        let lead = harm[0];
        let key = Key::new(
            lead.key.unwrap_or(0),
            if lead.minor { crate::theory::scale::Mode::Minor } else { crate::theory::scale::Mode::Major },
        );
        let bass = members
            .iter()
            .copied()
            .filter(|l| l.instrument == "Bass" && l.key.is_some())
            .min_by(|a, b| a.name.cmp(&b.name));
        // a beat: the family's own, else one from the same pack at the same tempo
        let beat = members.iter().copied().find(|l| l.instrument == "Drums").or_else(|| {
            let pack: Vec<&LoopInfo> = self
                .loops
                .iter()
                .filter(|l| {
                    l.instrument == "Drums"
                        && (l.tempo - lead.tempo).abs() < 0.5
                        && std::path::Path::new(&l.path).parent().map(|p| p.to_string_lossy())
                            == Some(k.0.as_str().into())
                        && !l.name.contains("Fill")
                })
                .collect();
            (!pack.is_empty()).then(|| pack[rng.below(pack.len())])
        });
        let progression = read_progression(lead, bass, &key)?;
        let as_loop = |l: &LoopInfo| Sound::Loop {
            path: l.path.clone(),
            bpm: l.tempo,
            beats: l.beats,
            key: l.key,
            tonal: l.key.is_some(),
            label: format!("'{}' ({})", l.name, source(&l.path)),
        };
        Some(compose::FamilyPick {
            name: k.2.clone(),
            bpm: lead.tempo,
            key,
            progression,
            chords: harm.iter().map(|l| as_loop(l)).collect(),
            bass: bass.map(as_loop),
            beat: beat.map(as_loop),
        })
    }

    fn one_shot(&self, lane: &str, hints: &[String], genre: &GenreProfile, rng: &mut Rng) -> Option<Sound> {
        let pref = apple_genres(&genre.name);
        let cands: Vec<&LoopInfo> = self
            .loops
            .iter()
            .filter(|l| hints.iter().any(|h| Self::matches_hint(l, h)) && !l.name.contains("Fill"))
            .collect();
        if cands.is_empty() {
            return None;
        }
        let w: Vec<f64> = cands.iter().map(|l| if pref.contains(&l.genre.as_str()) { 4.0 } else { 1.0 }).collect();
        // a few tries: some loops have no clean single hit (rolls, layered fills)
        for _ in 0..6 {
            let l = cands[rng.weighted(&w)];
            let seed = rng.next_u64() % 4;
            if let Ok(Some(p)) = oneshot::cached(std::path::Path::new(&l.path), lane, seed) {
                return Some(Sound::Sample {
                    path: p.to_string_lossy().into_owned(),
                    root: 60,
                    start: 0.0,
                    end: None,
                    label: format!("{lane} cut from '{}' ({})", l.name, source(&l.path)),
                });
            }
        }
        None
    }

    fn find_loop(
        &self,
        spec: &LoopSpec,
        genre: &GenreProfile,
        bpm: f64,
        key: &Key,
        rng: &mut Rng,
    ) -> Option<(Sound, i32)> {
        let pref = apple_genres(&genre.name);
        let mut cands: Vec<(&LoopInfo, i32, f64)> = Vec::new();
        for l in &self.loops {
            if l.tempo <= 0.0 || l.beats < 4.0 {
                continue;
            }
            if !spec.instrument.is_empty() && !spec.instrument.iter().any(|i| i.eq_ignore_ascii_case(&l.instrument)) {
                continue;
            }
            if !spec.subtype.is_empty() && !spec.subtype.iter().any(|s| s.eq_ignore_ascii_case(&l.subtype)) {
                continue;
            }
            if !spec.genres.is_empty() && !spec.genres.iter().any(|g| g.eq_ignore_ascii_case(&l.genre)) {
                continue;
            }
            if !spec.descriptors.is_empty()
                && !spec.descriptors.iter().any(|d| l.descriptors.iter().any(|x| x.eq_ignore_ascii_case(d)))
            {
                continue;
            }
            if l.name.contains("Fill") {
                continue;
            }
            let ratio = (bpm / l.tempo).log2();
            if ratio.abs() > 0.3 {
                continue;
            }
            let mut w = (-(ratio / 0.1).powi(2)).exp();
            let transpose = match (spec.tonal, l.key) {
                (true, Some(k)) => {
                    // a minor loop goes to the project's minor tonic (relative minor in a major key), and vice versa
                    let target = if l.minor == key.mode.is_minor() {
                        key.tonic
                    } else if l.minor {
                        (key.tonic + 9) % 12
                    } else {
                        (key.tonic + 3) % 12
                    };
                    let t = Key::new(target, key.mode).transpose_from(k);
                    w *= (-((t as f64) / 4.0).powi(2)).exp();
                    t
                }
                (true, None) => continue,
                _ => 0,
            };
            if pref.contains(&l.genre.as_str()) {
                w *= 2.0;
            }
            cands.push((l, transpose, w));
        }
        if cands.is_empty() {
            return None;
        }
        let w: Vec<f64> = cands.iter().map(|c| c.2).collect();
        let (l, t, _) = cands[rng.weighted(&w)];
        Some((
            Sound::Loop {
                path: l.path.clone(),
                bpm: l.tempo,
                beats: l.beats,
                key: l.key,
                tonal: spec.tonal,
                label: format!("'{}' ({}, Apple Loops)", l.name, l.genre),
            },
            t,
        ))
    }
}
