//! Genre profiles: the production knowledge claudius composes from, as data (crates/claudius/data/genres/*.json,
//! plus any *.json in claudius/genres under the user's config folder, ~/Library/Application Support on macOS and
//! ~/.config on Linux, to add or override genres without recompiling).

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::fx::Effect;
use crate::model::{Role, SectionSpec, TrackMix};
use crate::synth::drums::DrumVoice;
use crate::theory::chord::Voicing;
use crate::theory::rhythm::LaneTemplate;
use crate::theory::scale::Mode;

const BUILTIN: &[(&str, &str)] = &[
    ("house", include_str!("../data/genres/house.json")),
    ("deep_house", include_str!("../data/genres/deep_house.json")),
    ("techno", include_str!("../data/genres/techno.json")),
    ("trap", include_str!("../data/genres/trap.json")),
    ("lofi", include_str!("../data/genres/lofi.json")),
    ("dnb", include_str!("../data/genres/dnb.json")),
    ("future_bass", include_str!("../data/genres/future_bass.json")),
    ("synthwave", include_str!("../data/genres/synthwave.json")),
    ("trip_hop", include_str!("../data/genres/trip_hop.json")),
    ("big_beat", include_str!("../data/genres/big_beat.json")),
    ("electro_house", include_str!("../data/genres/electro_house.json")),
    ("nu_disco", include_str!("../data/genres/nu_disco.json")),
    ("filter_house", include_str!("../data/genres/filter_house.json")),
    ("dance_pop", include_str!("../data/genres/dance_pop.json")),
    ("neo_psychedelia", include_str!("../data/genres/neo_psychedelia.json")),
    ("art_rock", include_str!("../data/genres/art_rock.json")),
    ("idm", include_str!("../data/genres/idm.json")),
    ("dub_techno", include_str!("../data/genres/dub_techno.json")),
    ("boom_bap", include_str!("../data/genres/boom_bap.json")),
    ("uk_drill", include_str!("../data/genres/uk_drill.json")),
    ("dubstep", include_str!("../data/genres/dubstep.json")),
    ("uk_garage", include_str!("../data/genres/uk_garage.json")),
    ("jungle", include_str!("../data/genres/jungle.json")),
    ("afrobeats", include_str!("../data/genres/afrobeats.json")),
    ("amapiano", include_str!("../data/genres/amapiano.json")),
    ("reggaeton", include_str!("../data/genres/reggaeton.json")),
    ("trance", include_str!("../data/genres/trance.json")),
    ("neo_soul", include_str!("../data/genres/neo_soul.json")),
    ("hip_hop", include_str!("../data/genres/hip_hop.json")),
    ("alt_hip_hop", include_str!("../data/genres/alt_hip_hop.json")),
    ("rnb", include_str!("../data/genres/rnb.json")),
    ("funk", include_str!("../data/genres/funk.json")),
    ("tech_house", include_str!("../data/genres/tech_house.json")),
    ("ambient", include_str!("../data/genres/ambient.json")),
    ("grime", include_str!("../data/genres/grime.json")),
    ("phonk", include_str!("../data/genres/phonk.json")),
    ("reggae", include_str!("../data/genres/reggae.json")),
    ("g_funk", include_str!("../data/genres/g_funk.json")),
    ("gangsta_rap", include_str!("../data/genres/gangsta_rap.json")),
    ("jazz_rap", include_str!("../data/genres/jazz_rap.json")),
    ("chipmunk_soul", include_str!("../data/genres/chipmunk_soul.json")),
    ("crunk", include_str!("../data/genres/crunk.json")),
    ("cloud_rap", include_str!("../data/genres/cloud_rap.json")),
    ("emo_rap", include_str!("../data/genres/emo_rap.json")),
    ("rage", include_str!("../data/genres/rage.json")),
    ("plugg", include_str!("../data/genres/plugg.json")),
    ("ny_drill", include_str!("../data/genres/ny_drill.json")),
    ("chicago_drill", include_str!("../data/genres/chicago_drill.json")),
    ("rnb_90s", include_str!("../data/genres/rnb_90s.json")),
    ("new_jack_swing", include_str!("../data/genres/new_jack_swing.json")),
    ("alt_rnb", include_str!("../data/genres/alt_rnb.json")),
    ("big_room", include_str!("../data/genres/big_room.json")),
    ("progressive_house", include_str!("../data/genres/progressive_house.json")),
    ("slap_house", include_str!("../data/genres/slap_house.json")),
    ("hardstyle", include_str!("../data/genres/hardstyle.json")),
    ("hard_techno", include_str!("../data/genres/hard_techno.json")),
    ("gabber", include_str!("../data/genres/gabber.json")),
    ("hyperpop", include_str!("../data/genres/hyperpop.json")),
    ("jersey_club", include_str!("../data/genres/jersey_club.json")),
    ("afro_house", include_str!("../data/genres/afro_house.json")),
    ("melodic_techno", include_str!("../data/genres/melodic_techno.json")),
    ("baile_funk", include_str!("../data/genres/baile_funk.json")),
    ("breakcore", include_str!("../data/genres/breakcore.json")),
];

fn straight() -> (f32, f32) {
    (0.5, 0.5)
}
fn kit_mix() -> f64 {
    0.3
}

fn yes() -> bool {
    true
}
fn eight() -> u32 {
    8
}
fn quarter() -> f32 {
    0.3
}
fn mid_density() -> (f32, f32) {
    (0.4, 0.6)
}
fn close() -> Voicing {
    Voicing::Close
}

#[derive(Clone, Debug, Deserialize)]
pub struct LaneSpec {
    pub name: String,
    pub voice: DrumVoice,
    #[serde(default)]
    pub template: Option<LaneTemplate>,
    /// extra pattern variants by name (alt, sparse, half ...)
    #[serde(default)]
    pub variants: BTreeMap<String, LaneTemplate>,
    /// (onsets, steps) instead of a template
    #[serde(default)]
    pub euclid: Option<(usize, usize)>,
    /// parameter ranges overriding the voice defaults ("decay": [0.3, 0.6])
    #[serde(default)]
    pub params: BTreeMap<String, (f32, f32)>,
    /// Apple Loops subtypes whose one-shots may replace the synthesized voice ("Kick", "Snare", "Hi-hat")
    #[serde(default)]
    pub samples: Vec<String>,
    #[serde(default)]
    pub choke: Option<String>,
    #[serde(default)]
    pub tune_to_key: bool,
    #[serde(default)]
    pub mix: TrackMix,
    /// how far behind the beat the lane plays, in milliseconds, drawn from this range (`DrumLane::late_ms`): a
    /// backbeat laid back, a kick pushed ahead
    #[serde(default)]
    pub late_ms: Option<(f32, f32)>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PartSpec {
    pub name: String,
    pub role: Role,
    pub presets: Vec<String>,
    pub styles: Vec<String>,
    pub register: (i32, i32),
    #[serde(default = "mid_density")]
    pub density: (f32, f32),
    #[serde(default)]
    pub voices: usize,
    #[serde(default = "close")]
    pub voicing: Voicing,
    #[serde(default)]
    pub rates: Vec<f64>,
    /// probability that a generated song leaves this part out
    #[serde(default)]
    pub optional: f64,
    /// the chance it plays one of its own presets, in place of its genre's palette's for the role (an acid line
    /// stays an acid line)
    #[serde(default)]
    pub core: Option<f64>,
    #[serde(default)]
    pub mix: TrackMix,
    /// the genre's part whose notes this one plays, `octave` octaves away, on a sound of its own (`Part::doubles`): a
    /// fuzzy synth bass an octave over an 808, a bell over the keys
    #[serde(default)]
    pub doubles: Option<String>,
    #[serde(default)]
    pub octave: i32,
    /// how far behind the beat the part plays, in milliseconds, drawn from this range (`Part::late_ms`)
    #[serde(default)]
    pub late_ms: Option<(f32, f32)>,
    /// the sounds it plays at once, each in its band, as a producer stacks a bass (`Sound::Rack`); none: the one
    /// sound it draws
    #[serde(default)]
    pub layers: Vec<LayerSpec>,
}

/// A layer of a part a genre stacks (`PartSpec::layers`), as `rack()` takes one: its sound, `"$"` for the one the part
/// draws, else a synth preset by name; the frequencies it keeps; its level against the others; how far it ducks under
/// the kick; the semitones it plays from the part's notes; its own effects, after its band (a parallel distortion has
/// no band, and filters what it adds).
#[derive(Clone, Debug, Deserialize)]
pub struct LayerSpec {
    pub sound: String,
    #[serde(default)]
    pub band: Option<[f32; 2]>,
    #[serde(default)]
    pub level: f32,
    #[serde(default)]
    pub duck: f32,
    #[serde(default)]
    pub transpose: i32,
    #[serde(default)]
    pub fx: Vec<crate::fx::Effect>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LoopSpec {
    pub name: String,
    pub role: String,
    #[serde(default)]
    pub instrument: Vec<String>,
    #[serde(default)]
    pub subtype: Vec<String>,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub descriptors: Vec<String>,
    #[serde(default)]
    pub tonal: bool,
    #[serde(default)]
    pub optional: f64,
    #[serde(default)]
    pub mix: TrackMix,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TransitionSpec {
    #[serde(default = "yes")]
    pub riser: bool,
    #[serde(default = "yes")]
    pub impact: bool,
    #[serde(default = "yes")]
    pub downlifter: bool,
    #[serde(default = "yes")]
    pub reverse_crash: bool,
    #[serde(default = "eight")]
    pub riser_bars: u32,
    #[serde(default)]
    pub mix: TrackMix,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MixSpec {
    pub lufs: f32,
    pub reverb: Effect,
    pub delay: Effect,
    #[serde(default)]
    pub buses: BTreeMap<String, Vec<Effect>>,
    #[serde(default)]
    pub master: Vec<Effect>,
    #[serde(default = "quarter")]
    pub duck_release: f32,
    /// role -> dB offset from the default target loudness
    #[serde(default)]
    pub levels: BTreeMap<String, f32>,
    /// band -> the healthy range of its energy share in dB, where this genre's masters part from the usual ones (an
    /// 808 genre's sub, and the bands above it that the 808 leaves less of)
    #[serde(default)]
    pub bands: BTreeMap<String, [f32; 2]>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GenreProfile {
    pub name: String,
    /// the family it belongs to ("hip hop", "house"), which the studio groups genres under
    #[serde(default)]
    pub family: String,
    /// what else it is called, and the eras, scenes and artists it is known by ("90s hip hop", "golden age"): a
    /// search finds it by them (`claudius_api::search_genres`), and `get` takes one that is no other genre's
    #[serde(default)]
    pub aliases: Vec<String>,
    pub description: String,
    /// what the ears should hear (a CLAP description; earshot's vocabulary terms calibrate best)
    pub query: String,
    #[serde(default)]
    pub alts: Vec<String>,
    /// production flaws of the ears' (`ears::FLAWS`) that are this genre's sound, such as lofi's hiss: the
    /// production reading leaves them out
    #[serde(default)]
    pub accepted_flaws: Vec<String>,
    pub bpm: (f64, f64),
    #[serde(default = "straight")]
    pub swing: (f32, f32),
    #[serde(default)]
    pub humanize: f32,
    pub modes: Vec<(Mode, f64)>,
    pub harmony: Vec<(String, f64)>,
    /// chance of writing a fresh progression with the Markov chain instead of picking from the library
    #[serde(default)]
    pub markov: f64,
    /// chord extensions for generated progressions (0 triads, 1 sevenths, 2 ninths)
    #[serde(default)]
    pub ext: u8,
    pub bars_per_chord: Vec<(u32, f64)>,
    pub forms: Vec<Vec<SectionSpec>>,
    /// how each role's sounds are drawn beyond the parts' own presets (`palette`): by role name
    #[serde(default)]
    pub palette: BTreeMap<String, crate::palette::RolePalette>,
    /// the chance that a drum lane plays a kit of its own rather than the song's
    #[serde(default = "kit_mix")]
    pub kit_mix: f64,
    pub drums: Vec<LaneSpec>,
    /// drum machines / kits from the sample packs a song may play its drums on (bank name, weight); the remaining
    /// weight up to 1 keeps the lanes' own sounds
    #[serde(default)]
    pub kits: Vec<(String, f64)>,
    pub parts: Vec<PartSpec>,
    #[serde(default)]
    pub loops: Vec<LoopSpec>,
    /// the samples the genre is built on, cut from recordings in free online libraries (`crate::sampling`)
    #[serde(default)]
    pub sampling: Option<SamplingSpec>,
    pub transitions: TransitionSpec,
    pub mix: MixSpec,
    /// how many layers its songs play when nobody says (`compose::Options::complexity`): 0.5, as the profile has
    /// them, unless the genre is a layered one
    #[serde(default = "as_written")]
    pub complexity: f64,
}

fn as_written() -> f64 {
    0.5
}

/// What a genre samples, from the records its producers dig for: a drum break, a phrase to build the song on, a
/// texture under it. Each plays as a loop track of its own (`sampling::BREAK`, `PHRASE`, `TEXTURE`) in the sections
/// that list it, or, in a genre whose sections do not, in place of the layers it plays `instead` of. A vocal is a note
/// a voice sang, chopped from another record, which a part plays on the song's chords where it lifts (`sampling::VOCAL`).
/// A bass is a note a bass played, which the song's bass part plays its own notes on, as a sampler would.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct SamplingSpec {
    /// whether a song of the genre is built on samples when nobody says (trip-hop's is; a house song is when asked)
    #[serde(default = "yes")]
    pub by_default: bool,
    #[serde(default, rename = "break")]
    pub drum_break: Option<SampleNeed>,
    #[serde(default)]
    pub phrase: Option<SampleNeed>,
    #[serde(default)]
    pub texture: Option<SampleNeed>,
    #[serde(default)]
    pub vocal: Option<SampleNeed>,
    #[serde(default)]
    pub bass: Option<SampleNeed>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SampleNeed {
    /// what to search the libraries for, from what the genre's producers sample ("soul", "film score", "funk drum
    /// break"): a gathering tries them in an order the song's seed picks
    pub search: Vec<String>,
    /// what the ears should hear in a good one, to choose among them ("a slow funky drum break")
    pub listen: String,
    /// the shortest and longest recording worth downloading, in seconds
    #[serde(default = "any_length")]
    pub seconds: (f64, f64),
    /// how many to keep at the ready for the genre
    #[serde(default = "three")]
    pub keep: usize,
    /// the chance that a song plays one, when one is at hand
    #[serde(default = "always")]
    pub chance: f64,
    /// the chance that the loop is re-chopped, its half-beat slices played in another order
    #[serde(default)]
    pub chop: f64,
    /// the semitones a phrase may be played up or down, a sampler's varispeed, which moves its tempo with its pitch
    /// (a soul record sped up into house, a film score slowed into trip-hop): the song takes the tempo and the key
    /// that come of it
    #[serde(default = "a_little")]
    pub pitch: (i32, i32),
    /// whether a voice in a phrase is sped up or slowed down with it, as the genre's producers mean it to be (chipmunk
    /// soul's singers, sped up until they rise); in a genre that does not, a remake's hook that a voice sings in plays
    /// in its own key (`remix`)
    #[serde(default)]
    pub speeds_voices: bool,
    /// the layers that play where the sample would, when the song has none (offline, or nothing gathered yet); in a
    /// genre whose sections do not list the sample, those it plays in place of (under everything, when none)
    #[serde(default)]
    pub instead: Vec<String>,
    #[serde(default)]
    pub mix: TrackMix,
    /// How a phrase may be treated, as a producer treats a record, each move (`sampling::MOVES`) by how often a song
    /// makes it: held behind a filter until the song lifts, darkened, narrowed to a telephone's band in the intro and
    /// the breaks, overloaded, crushed. A song makes one, which its seed picks. None said: those that suit most genres.
    #[serde(default = "moves")]
    pub treat: BTreeMap<String, f64>,
}

/// the moves a phrase is treated with when its genre does not say, and how often each is made
fn moves() -> BTreeMap<String, f64> {
    [
        ("as it is", 0.4),
        ("filtered until it lifts", 0.25),
        ("a telephone's band in the intro and the breaks", 0.2),
        ("overloaded", 0.15),
    ]
    .into_iter()
    .map(|(m, w)| (m.to_string(), w))
    .collect()
}

fn any_length() -> (f64, f64) {
    (2.0, 600.0)
}
fn a_little() -> (i32, i32) {
    (-3, 3)
}
fn three() -> usize {
    3
}
fn always() -> f64 {
    1.0
}

/// the genres claudius comes with (`all` adds the user's own)
pub fn builtin_names() -> impl Iterator<Item = &'static str> {
    BUILTIN.iter().map(|(name, _)| *name)
}

fn user_dir() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("claudius").join("genres"))
}

pub fn all() -> Result<Vec<GenreProfile>> {
    let mut out: BTreeMap<String, GenreProfile> = BTreeMap::new();
    for (name, src) in BUILTIN {
        out.insert(name.to_string(), builtin(name, src)?);
    }
    for g in own()? {
        out.insert(g.name.clone(), g);
    }
    Ok(out.into_values().collect())
}

fn builtin(name: &str, src: &str) -> Result<GenreProfile> {
    serde_json::from_str(src).with_context(|| format!("built-in genre {name}"))
}

/// the genres of the person's own, in their genres folder (`user_dir`)
fn own() -> Result<Vec<GenreProfile>> {
    let mut out = Vec::new();
    if let Some(dir) = user_dir()
        && let Ok(rd) = std::fs::read_dir(&dir)
    {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "json") {
                let src = std::fs::read_to_string(&p)?;
                out.push(serde_json::from_str(&src).with_context(|| format!("genre file {}", p.display()))?);
            }
        }
    }
    Ok(out)
}

/// how many genres one fusion takes at most
pub const MOST_FUSED: usize = 3;

/// A genre by name, or two or three fused (`trip_hop+idm`): the first leads and each after it plays the drums
/// (`fuse`).
pub fn get(name: &str) -> Result<GenreProfile> {
    if name.contains('+') {
        let names: Vec<&str> = name.split('+').map(str::trim).collect();
        if names.len() > MOST_FUSED || names.iter().any(|n| n.is_empty()) {
            anyhow::bail!("a fusion is two or three genres joined by +, such as trip_hop+idm, not '{name}'");
        }
        let first = get(names[0])?;
        return names[1..].iter().try_fold(first, |g, n| Ok(fuse(&g, &get(n)?)));
    }
    let key = name.trim().to_ascii_lowercase().replace([' ', '-'], "_");
    let named = |n: &str| n == key || folded(n) == folded(name);
    // one of the person's own of that name, which takes a built-in's place; else the built-in, the only one read
    let mut own = own()?;
    if let Some(at) = own.iter().position(|g| named(&g.name)) {
        return Ok(own.swap_remove(at));
    }
    if let Some((n, src)) = BUILTIN.iter().find(|(n, _)| named(n)) {
        return builtin(n, src);
    }
    let all = all()?;
    let called: Vec<GenreProfile> =
        all.iter().filter(|g| g.aliases.iter().any(|a| folded(a) == folded(name))).cloned().collect();
    match called.as_slice() {
        [g] => Ok(g.clone()),
        [] => anyhow::bail!(
            "unknown genre '{name}' (try: {})",
            all.iter().map(|g| g.name.as_str()).collect::<Vec<_>>().join(", ")
        ),
        many => anyhow::bail!(
            "'{name}' is what {} are called: say which",
            many.iter().map(|g| g.name.as_str()).collect::<Vec<_>>().join(" and ")
        ),
    }
}

/// a name as a search compares it: lowercase letters and digits only, `&` as `n` (R&B, rnb and r n b alike)
pub fn folded(name: &str) -> String {
    name.to_lowercase().replace('&', "n").chars().filter(|c| c.is_alphanumeric()).collect()
}

/// the lanes that are drums, for the levels a fusion takes from the genre that plays its drums
const DRUM_ROLES: &[&str] = &["kick", "snare", "clap", "hat", "ohat", "shaker", "perc", "ride", "tom", "crash", "top"];

/// Two genres as one. `a` leads: its tempo, keys, harmony, forms, parts, loops, transitions and mix, and what the ears
/// listen for. `b` plays the drums: its lanes, kits, drums bus and drum levels, in `a`'s sections where `a`'s drums
/// played. Its harmony comes in now and then, its first preset and its styles join `a`'s parts of the same role (so
/// that each part mostly sounds as it did), and its parts of roles `a` has none of come in now and then too. Sounding like `b` no longer counts against the song.
pub fn fuse(a: &GenreProfile, b: &GenreProfile) -> GenreProfile {
    let mut g = a.clone();
    g.name = format!("{}+{}", a.name, b.name);
    g.aliases = Vec::new();
    g.description = format!(
        "{} Fused with {}: its drums and their sound, and its sounds and styles in the parts.",
        a.description.trim_end_matches('.'),
        b.name.replace('_', " ")
    );
    g.alts.retain(|alt| *alt != b.query);
    for flaw in &b.accepted_flaws {
        if !g.accepted_flaws.contains(flaw) {
            g.accepted_flaws.push(flaw.clone());
        }
    }
    g.swing = ((a.swing.0 + b.swing.0) / 2.0, (a.swing.1 + b.swing.1) / 2.0);
    g.humanize = a.humanize.max(b.humanize);
    // b's progressions a quarter of the time
    for h in g.harmony.iter_mut() {
        h.1 *= 0.75;
    }
    for (style, w) in &b.harmony {
        match g.harmony.iter_mut().find(|h| h.0 == *style) {
            Some(h) => h.1 += w * 0.25,
            None => g.harmony.push((style.clone(), w * 0.25)),
        }
    }
    // a's sampled break is a's drums: the lanes it falls back to stand in for it, and b's drums replace them below
    if let Some(n) = a.sampling.as_ref().and_then(|s| s.drum_break.as_ref()) {
        for s in g.forms.iter_mut().flatten() {
            if s.layers.iter().any(|l| l == crate::sampling::BREAK) {
                s.layers.retain(|l| l != crate::sampling::BREAK);
                for l in &n.instead {
                    if !s.layers.contains(l) {
                        s.layers.push(l.clone());
                    }
                }
            }
        }
        if let Some(sp) = g.sampling.as_mut() {
            sp.drum_break = None;
        }
    }
    // b's drums play where a's did: a lane of the same name where that lane played, b's others where most of a's did
    let lanes_a: Vec<&str> = a.drums.iter().map(|d| d.name.as_str()).collect();
    let lanes_b: Vec<String> = b.drums.iter().map(|d| d.name.clone()).collect();
    let mut added: Vec<String> = Vec::new();
    for p in &b.parts {
        if !a.parts.iter().any(|q| q.role == p.role || q.name == p.name) {
            added.push(p.name.clone());
        }
    }
    for form in g.forms.iter_mut() {
        for s in form.iter_mut() {
            let drums_a = s.layers.iter().filter(|l| lanes_a.contains(&l.as_str())).count();
            let most = drums_a * 2 >= lanes_a.len().max(1);
            let mut layers: Vec<String> = s.layers.iter().filter(|l| !lanes_a.contains(&l.as_str())).cloned().collect();
            for lane in &lanes_b {
                if s.layers.contains(lane) || (!lanes_a.contains(&lane.as_str()) && most) {
                    layers.push(lane.clone());
                }
            }
            if s.energy >= 0.6 {
                layers.extend(added.iter().cloned());
            }
            s.layers = layers;
        }
    }
    g.drums = b.drums.clone();
    g.kits = b.kits.clone();
    for p in g.parts.iter_mut() {
        if let Some(q) = b.parts.iter().find(|q| q.role == p.role) {
            if let Some(preset) = q.presets.first().filter(|x| !p.presets.contains(x)) {
                p.presets.push(preset.clone());
            }
            for style in &q.styles {
                if !p.styles.contains(style) {
                    p.styles.push(style.clone());
                }
            }
        }
    }
    for q in b.parts.iter().filter(|q| added.contains(&q.name)) {
        let mut q = q.clone();
        q.optional = q.optional.max(0.4);
        g.parts.push(q);
    }
    for l in &b.loops {
        if !g.loops.iter().any(|x| x.name == l.name) {
            g.loops.push(l.clone());
        }
    }
    match b.mix.buses.get("drums") {
        Some(chain) => g.mix.buses.insert("drums".into(), chain.clone()),
        None => g.mix.buses.remove("drums"),
    };
    g.mix.levels.retain(|role, _| !DRUM_ROLES.contains(&role.as_str()));
    for (role, db) in &b.mix.levels {
        if DRUM_ROLES.contains(&role.as_str()) {
            g.mix.levels.insert(role.clone(), *db);
        }
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_genres_bands_are_bands_the_critic_measures_each_with_a_range() {
        for (name, src) in BUILTIN {
            let g: GenreProfile = serde_json::from_str(src).unwrap();
            for (band, [lo, hi]) in &g.mix.bands {
                assert!(crate::judge::BANDS.iter().any(|(b, _, _)| b == band), "{name}: no band '{band}'");
                assert!(lo < hi, "{name}: {band} from {lo} to {hi}");
            }
        }
        let trap = get("trap").unwrap();
        assert!(trap.mix.bands["sub"][1] > -4.0, "an 808 genre's sub may be heavier than the usual master's");
    }

    #[test]
    fn every_genre_samples_from_its_own_records_in_place_of_layers_it_has() {
        for (name, src) in BUILTIN {
            let g: GenreProfile = serde_json::from_str(src).unwrap();
            let s = g.sampling.as_ref().unwrap_or_else(|| panic!("{name} says what it samples"));
            let phrase = s.phrase.as_ref().unwrap_or_else(|| panic!("{name} samples a phrase"));
            assert!(phrase.search.len() >= 6, "{name}: a phrase has several records to dig for");
            assert!(phrase.pitch.0 <= 0 && phrase.pitch.1 >= 0, "{name}: a phrase may play at its own pitch");
            let known: Vec<&str> = g
                .parts
                .iter()
                .map(|p| p.name.as_str())
                .chain(g.drums.iter().map(|d| d.name.as_str()))
                .chain(g.loops.iter().map(|l| l.name.as_str()))
                .collect();
            for n in [&s.drum_break, &s.phrase, &s.texture, &s.vocal].into_iter().flatten() {
                for l in &n.instead {
                    assert!(known.contains(&l.as_str()), "{name}: a sample stands in for '{l}', which it lacks");
                }
                // what is searched for is a kind of record, not the genre itself or a mood
                for w in &n.search {
                    assert!(!w.contains(&name.replace('_', " ")), "{name} searches for itself: '{w}'");
                    assert!(!["sad", "dark", "happy"].iter().any(|m| w.split(' ').any(|x| x == *m)), "{name}: '{w}'");
                }
            }
        }
        assert!(get("trip_hop").unwrap().sampling.unwrap().by_default);
        assert!(!get("house").unwrap().sampling.unwrap().by_default);
    }

    #[test]
    fn a_genre_is_found_by_another_name_of_its_own() {
        assert_eq!(get("90s hip hop").unwrap().name, "boom_bap");
        assert_eq!(get("R&B").unwrap().name, "rnb");
        assert_eq!(get("Lo-Fi Hip Hop").unwrap().name, "lofi");
        let both = get("uk rap").unwrap_err().to_string();
        assert!(both.contains("uk_drill") && both.contains("grime"), "{both}");
        assert!(get("polka").unwrap_err().to_string().starts_with("unknown genre 'polka'"));
        let builtin: Vec<GenreProfile> = BUILTIN.iter().map(|(_, src)| serde_json::from_str(src).unwrap()).collect();
        for ((listed, _), g) in BUILTIN.iter().zip(&builtin) {
            assert_eq!(*listed, g.name, "a built-in genre is listed by its own name, which get reads it by");
        }
        for g in &builtin {
            assert!(!g.family.is_empty(), "{} belongs to a family", g.name);
            for a in &g.aliases {
                assert!(builtin.iter().all(|o| folded(&o.name) != folded(a)), "{}: '{a}' is a genre's name", g.name);
            }
        }
    }

    #[test]
    fn a_fusion_keeps_the_first_genre_and_takes_the_seconds_drums() {
        let (a, b) = (get("trip_hop").unwrap(), get("idm").unwrap());
        let g = get("Trip-Hop + IDM").unwrap();
        assert_eq!((g.name.as_str(), g.bpm, &g.query), ("trip_hop+idm", a.bpm, &a.query));
        assert_eq!(
            g.drums.iter().map(|d| &d.name).collect::<Vec<_>>(),
            b.drums.iter().map(|d| &d.name).collect::<Vec<_>>()
        );
        assert_eq!((&g.kits, g.mix.buses.get("drums")), (&b.kits, b.mix.buses.get("drums")));
        assert_eq!(
            (g.mix.lufs, &g.mix.master, g.mix.buses.get("music")),
            (a.mix.lufs, &a.mix.master, a.mix.buses.get("music"))
        );
        // the parts: trip-hop's, the lead with the bells' presets too, and IDM's arpeggio, which trip-hop has none of
        let lead = g.parts.iter().find(|p| p.name == "lead").unwrap();
        let bells = &b.parts.iter().find(|p| p.role == lead.role).unwrap().presets[0];
        assert!(lead.presets.ends_with(std::slice::from_ref(bells)), "{:?}", lead.presets);
        let arp = g.parts.iter().find(|p| p.name == "arp").expect("the arp comes in");
        assert!(arp.optional >= 0.4);
        // IDM's lanes play where trip-hop's did: a section with most of the drums has them all, the intro's one
        // lane none of them but its own name's
        let hook = g.forms[0].iter().find(|s| s.name == "hook").unwrap();
        for lane in ["kick", "snare", "hat", "click", "tom", "arp"] {
            assert!(hook.layers.iter().any(|l| l == lane), "{lane} in {:?}", hook.layers);
        }
        assert!(!hook.layers.iter().any(|l| l == "ohat"), "trip-hop's open hat goes with its drums");
        let intro = &g.forms[0][0];
        assert!(!intro.layers.iter().any(|l| l == "click" || l == "kick"), "{:?}", intro.layers);
        // the harmony leans on the first genre's, the flaws either genre accepts are accepted
        let lead_weight: f64 = g.harmony.iter().filter(|h| a.harmony.iter().any(|x| x.0 == h.0)).map(|h| h.1).sum();
        assert!(lead_weight >= 0.75);
        assert!(g.accepted_flaws.iter().any(|f| f == "a cheap synthesized beep"));
    }

    #[test]
    fn a_fusion_says_what_it_takes() {
        assert!(get("house+techno+dnb").unwrap().name == "house+techno+dnb");
        let e = |n: &str| get(n).unwrap_err().to_string();
        assert!(e("house+techno+dnb+trap").contains("two or three genres"));
        assert!(e("house+").contains("two or three genres"));
        assert!(e("house+polka").contains("unknown genre 'polka'"));
        // what the second genre is no longer counts against the first
        let g = get("house+dnb").unwrap();
        assert!(!g.alts.iter().any(|a| a == "drum and bass music"), "{:?}", g.alts);
    }

    #[test]
    fn the_accepted_flaws_are_flaws_the_ears_know() {
        for (name, src) in BUILTIN {
            let g: GenreProfile = serde_json::from_str(src).unwrap();
            for flaw in &g.accepted_flaws {
                assert!(crate::ears::FLAWS.contains(&flaw.as_str()), "{name}: {flaw}");
            }
        }
    }
}
