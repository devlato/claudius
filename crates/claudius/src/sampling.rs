//! Sampling. Genres built on samples (trip-hop's slowed breaks and film strings, boom bap's chopped records) get them
//! from free online libraries (`claudius_packs::online`). `gather` downloads recordings for what a genre's `sampling`
//! asks for and cuts each on its beat grid into what a song can play: a break loop and its own hits, a phrase of two bars
//! in its key with its chords, a stretch of texture. With the ears it keeps only those that sound like what the genre
//! listens for, and lists them (`library::prepared`). `apply` puts them into a song as it is composed: the phrase sets
//! the song's key, its tempo (within the genre's range) and its first progression, the break plays (re-chopped at
//! times) and lends the drum lanes its hits, and the texture lies under everything.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use claudius_packs::online::{self, Found, Policy, Query};

use crate::analysis::drums::{self, Kind};
use crate::analysis::song as analysis;
use crate::audio::{self, Stereo};
use crate::config::SRF;
use crate::ears::Ears;
use crate::fx::Effect;
use crate::genres::{GenreProfile, SampleNeed};
use crate::library::prepared::{Index, Passage, Prepared, Role};
use crate::model::*;
use crate::rng::Rng;
use crate::theory::scale::{Key, Mode};

/// the loop tracks the roles play as, which sections list by these names
pub const BREAK: &str = "break";
pub const PHRASE: &str = "sample";
pub const TEXTURE: &str = "texture";
/// the loop track a phrase's second passage plays as, in some of the sections that list the phrase
pub const SECOND: &str = "sample2";
/// the progression of the second passage's chords: the phrase's are `A`, and the genre's own `B` stays the contrast
/// it gives the sections on it
pub const SECOND_PROG: &str = "A2";

/// the ears' z below which a cut is not kept: a reading short of plausible (2.5) can still be the best a library has,
/// but under this it is unlikely to be what was asked for
pub const MIN_Z: f64 = 1.5;

/// how long a phrase is, in bars
const PHRASE_BARS: usize = 2;

pub fn track_name(role: Role) -> &'static str {
    match role {
        Role::Break => BREAK,
        Role::Phrase => PHRASE,
        Role::Texture => TEXTURE,
    }
}

/// what the genre asks of `role`, if it samples for it
pub fn need(g: &GenreProfile, role: Role) -> Option<&SampleNeed> {
    let s = g.sampling.as_ref()?;
    match role {
        Role::Break => s.drum_break.as_ref(),
        Role::Phrase => s.phrase.as_ref(),
        Role::Texture => s.texture.as_ref(),
    }
}

/// the layer names a genre's sampling adds to its sections
pub fn layer_names(g: &GenreProfile) -> Vec<&'static str> {
    Role::ALL.into_iter().filter(|r| need(g, *r).is_some()).map(track_name).collect()
}

/// the longest search of one's own, in characters
pub const MOST_WORDS: usize = 60;

/// A search of one's own as a gathering keeps it: lower case, single spaces, at most `MOST_WORDS` characters; none
/// when nothing is left.
pub fn words(search: &str) -> Option<String> {
    let w: String = search.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    let w: String = w.chars().take(MOST_WORDS).collect();
    let w = w.trim().to_string();
    (!w.is_empty()).then_some(w)
}

/// Whether a song of `g` is built on samples: as `sampled` says, else as the genre is by default, or when it is given
/// words of its own to search for.
pub fn sampled(g: &GenreProfile, sampled: Option<bool>, search: Option<&str>) -> bool {
    let Some(s) = &g.sampling else { return false };
    sampled.unwrap_or(s.by_default || search.and_then(words).is_some())
}

/// What a gathering is for: the song whose seed picks the order the genre's searches are tried in (and the order of
/// what they find), the words of one's own its phrase is searched for, and how many new recordings each role may
/// keep at most this time (the rest come with later songs, until the role has its `keep`).
#[derive(Clone, Debug)]
pub struct Ask {
    /// none: the searches in the order the genre lists them
    pub seed: Option<u64>,
    pub search: Option<String>,
    pub most: usize,
}

impl Ask {
    /// a gathering of everything the genre keeps, in the order its searches are listed in
    pub fn all() -> Ask {
        Ask { seed: None, search: None, most: usize::MAX }
    }

    /// the words a role is searched for with, when they are one's own (the phrase's)
    pub fn asked(&self, role: Role) -> Option<&str> {
        self.search.as_deref().filter(|_| role == Role::Phrase)
    }
}

/// what a gathering did: the samples it prepared, a line for each thing worth saying, and the roles the libraries had
/// nothing more of (every search answered, and none found a recording it had not tried that it could take)
#[derive(Debug, Default)]
pub struct Gathered {
    pub added: Vec<Prepared>,
    pub notes: Vec<String>,
    pub exhausted: Vec<Role>,
}

/// `items` in the order `rng` picks
fn shuffled<T: Clone>(items: &[T], rng: &mut Rng) -> Vec<T> {
    let mut out = items.to_vec();
    for i in (1..out.len()).rev() {
        out.swap(i, rng.below(i + 1));
    }
    out
}

/// Download and prepare samples toward the genre's `keep` of each role it samples, at most `ask.most` new ones each:
/// search the libraries (for the role's words, in the order the song's seed picks, or for the words asked for the
/// phrase), download recordings of a fitting length (at most three for each one still missing), cut them, and with
/// the ears keep those that sound like what was asked for. A recording found wanting is remembered and not fetched
/// again. `stop` is asked before each download: when it says so, what was kept so far is what there is.
pub fn gather(
    g: &GenreProfile,
    ask: &Ask,
    ears: Option<&Ears>,
    policy: &Policy,
    ix: &Index,
    stop: &dyn Fn() -> bool,
    say: &mut dyn FnMut(&str),
) -> Result<Gathered> {
    let mut out = Gathered::default();
    let rng = Rng::new(ask.seed.unwrap_or_default() ^ 0x6a7e_5a3b);
    for role in Role::ALL {
        let Some(n) = need(g, role) else { continue };
        if stop() {
            break;
        }
        let asked = ask.asked(role);
        let mut known: HashSet<String> = ix.of(role, lead(&g.name), asked).into_iter().map(|p| p.source).collect();
        let mut have = known.len();
        if have >= n.keep {
            continue;
        }
        let goal = n.keep.min(have.saturating_add(ask.most));
        let mut budget = (goal - have) * 3;
        let mut r = rng.fork(role.name());
        let searches = match (asked, ask.seed) {
            (Some(w), _) => vec![w.to_string()],
            (None, None) => n.search.clone(),
            (None, Some(_)) => shuffled(&n.search, &mut r),
        };
        // the ears listen for what was asked: the words of one's own, or what the genre listens for
        let n = &SampleNeed { listen: asked.map_or_else(|| n.listen.clone(), str::to_string), ..n.clone() };
        let who = asked.map_or_else(|| lead(&g.name).to_string(), |w| format!("\"{w}\""));
        let theirs = whose(g, asked);
        let folder = ix.root().join(role.name()).join(&theirs);
        // whether every search answered and was looked through to its end: then what is not had is not there
        let mut through = true;
        'search: for words in &searches {
            if stop() {
                through = false;
                break;
            }
            let q = Query { text: words.clone(), sources: Vec::new(), policy: *policy, limit: 12 };
            let got = match online::search(&q) {
                Ok(s) => s,
                Err(e) => {
                    out.notes.push(format!("searching for \"{words}\": {e:#}"));
                    through = false;
                    continue;
                }
            };
            for (s, e) in &got.failed {
                out.notes.push(format!("{} did not answer: {e}", s.name()));
                through = false;
            }
            let found = if ask.seed.is_some() { shuffled(&got.found, &mut r) } else { got.found };
            for f in found {
                if have >= goal || budget == 0 || stop() {
                    through = false;
                    break 'search;
                }
                if known.contains(&f.id)
                    || ix.rejected(role, &f.id, &theirs)
                    || f.seconds.is_some_and(|s| s < n.seconds.0 || s > n.seconds.1)
                {
                    continue;
                }
                budget -= 1;
                known.insert(f.id.clone());
                say(&format!("{} for {who}: \"{}\" by {} ({})", role.name(), f.title, f.creator, f.id));
                let path = match online::fetch(&f, policy) {
                    Ok(p) => p,
                    Err(e) => {
                        // a download that did not come may come next time: it is not the recording's fault
                        out.notes.push(format!("{}: {e:#}", f.id));
                        known.remove(&f.id);
                        through = false;
                        continue;
                    }
                };
                match prepare(role, &path, &f, g, n, ears, &folder) {
                    Ok(mut p) if p.z.is_none_or(|z| z >= MIN_Z) => {
                        p.asked = asked.map(str::to_string);
                        ix.add(p.clone())?;
                        have += 1;
                        out.added.push(p);
                    }
                    Ok(p) => {
                        out.notes.push(format!(
                            "{}: the ears heard \"{}\" only at z {:.1}, so it is left out",
                            f.id,
                            n.listen,
                            p.z.unwrap_or_default()
                        ));
                        forget(&p);
                        // the ears heard other than what this gathering asked for: another may want it
                        ix.reject(role, &f.id, Some(&theirs))?;
                    }
                    Err(e) => {
                        out.notes.push(format!("{}: {e:#}", f.id));
                        ix.reject(role, &f.id, None)?;
                    }
                }
            }
        }
        if have < goal {
            out.notes.push(format!("only {have} of the {} {}s for {who} could be prepared", n.keep, role.name()));
            if through {
                out.exhausted.push(role);
            }
        }
    }
    Ok(out)
}

/// remove the files a cut left behind
fn forget(p: &Prepared) {
    let second = p.second.as_ref().map(|s| &s.path);
    for f in [Some(&p.path), p.kick.as_ref(), p.snare.as_ref(), p.hat.as_ref(), second].into_iter().flatten() {
        let _ = std::fs::remove_file(f);
        let _ = std::fs::remove_file(format!("{f}.json"));
    }
}

/// A recording cut into what a role plays: the sample as the composer lists it, and each file it names with the audio
/// that goes in it, the loop first (`float`: written as 32-bit floats, as a drum hit is, else as 24-bit samples).
pub struct Cut {
    pub prepared: Prepared,
    pub files: Vec<CutFile>,
}

pub struct CutFile {
    pub path: PathBuf,
    pub audio: Stereo,
    pub float: bool,
}

impl Cut {
    fn new(prepared: Prepared) -> Cut {
        Cut { prepared, files: Vec::new() }
    }

    /// keeps `audio` as the file at `path`, and says where that is
    fn file(&mut self, path: PathBuf, audio: Stereo, float: bool) -> String {
        let at = path.to_string_lossy().into_owned();
        self.files.push(CutFile { path, audio, float });
        at
    }

    /// writes its files, each with the recording's credit beside it
    pub fn write(&self, f: &Found) -> Result<()> {
        for c in &self.files {
            audio::save_wav(&c.path, &c.audio, c.float)?;
            online::write_credit(&c.path, f)?;
        }
        Ok(())
    }
}

/// Cut `x`, the recording `f`, into what `role` plays, its files named in `dir`, and with the ears say how strongly it
/// sounds like what was asked for. Nothing is written: `prepare` writes the files, and a browser keeps them in memory.
pub fn cut(
    role: Role,
    x: &Stereo,
    f: &Found,
    g: &GenreProfile,
    n: &SampleNeed,
    ears: Option<&Ears>,
    dir: &Path,
) -> Result<Cut> {
    if x.len() < SRF as usize {
        bail!("shorter than a second");
    }
    let mut c = match role {
        Role::Break => cut_break(x, f, g, dir)?,
        Role::Phrase => cut_phrase(x, n, ears, dir)?,
        Role::Texture => cut_texture(x, dir)?,
    };
    c.prepared.genre = lead(&g.name).to_string();
    c.prepared.source = f.id.clone();
    if role != Role::Phrase
        && let Some(e) = ears
        && let Some(main) = c.files.first()
    {
        c.prepared.z = Some(e.z(&main.audio, &n.listen)?);
    }
    Ok(c)
}

/// the folder of `<root>/<role>/` the cuts of a gathering go into: the genre's, or the words asked for (`asked-…`, with
/// a hash of the words themselves, since words that differ only in what a folder's name leaves out are other words), so
/// that one recording cut for two of them is two cuts, neither in the other's place
pub fn whose(g: &GenreProfile, asked: Option<&str>) -> String {
    match asked {
        Some(w) => {
            let hash = &blake3::hash(w.as_bytes()).to_hex()[..8];
            match crate::song::slug(w) {
                s if s.is_empty() => format!("asked-{hash}"),
                s => format!("asked-{s}-{hash}"),
            }
        }
        None => lead(&g.name).to_string(),
    }
}

/// Cut a downloaded recording into what `role` plays, in `<folder>/<recording>/` (the gathering's folder, `whose`), with
/// the recording's credit beside each file, and with the ears say how strongly it sounds like what was asked for.
pub fn prepare(
    role: Role,
    path: &Path,
    f: &Found,
    g: &GenreProfile,
    n: &SampleNeed,
    ears: Option<&Ears>,
    folder: &Path,
) -> Result<Prepared> {
    let x = audio::load(path).with_context(|| format!("reading {}", path.display()))?;
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "sound".into());
    let dir = folder.join(crate::song::slug(&stem));
    let c = cut(role, &x, f, g, n, ears, &dir)?;
    std::fs::create_dir_all(&dir).with_context(|| format!("cannot create {}", dir.display()))?;
    c.write(f)?;
    Ok(c.prepared)
}

fn blank(path: &Path, role: Role, bpm: f64, beats: f64) -> Prepared {
    Prepared {
        path: path.to_string_lossy().into_owned(),
        role,
        genre: String::new(),
        bpm,
        beats,
        key: None,
        minor: false,
        kick: None,
        snare: None,
        hat: None,
        chords: Vec::new(),
        z: None,
        source: String::new(),
        asked: None,
        second: None,
    }
}

/// a tempo a recording's title or tags state: `[128bpm]`, `128 BPM`, ccMixter's `bpm_080_085`
pub fn said_bpm(f: &Found) -> Option<f64> {
    let words = format!("{} {}", f.title, f.tags.join(" ")).to_lowercase();
    if let Some(i) = words.find("bpm_") {
        let n: Vec<f64> =
            words[i + 4..].split(|c: char| !c.is_ascii_digit()).take(2).filter_map(|w| w.parse::<f64>().ok()).collect();
        if !n.is_empty() {
            return Some(n.iter().sum::<f64>() / n.len() as f64);
        }
    }
    let i = words.find("bpm")?;
    let before: String = words[..i]
        .trim_end()
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    before.parse::<f64>().ok().filter(|b| (40.0..=240.0).contains(b))
}

/// A short loop's tempo and length in beats from its duration: the whole number of bars (one, two, four or eight)
/// nearest the tempo its title states, else nearest the middle of the genre's tempo range.
pub fn loop_tempo(secs: f64, said: Option<f64>, range: (f64, f64)) -> Option<(f64, f64)> {
    let mid = (range.0 + range.1) / 2.0;
    let miss = |bpm: f64| (bpm - said.unwrap_or(mid)).abs();
    [4.0, 8.0, 16.0, 32.0]
        .iter()
        .map(|beats| (beats * 60.0 / secs, *beats))
        .filter(|(bpm, _)| (50.0..=200.0).contains(bpm))
        .min_by(|a, b| miss(a.0).total_cmp(&miss(b.0)))
}

/// the loudest clean hit of `kind` among `hits`, cut to its decay or the next hit
fn cut_hit(x: &Stereo, hits: &[drums::Hit], kind: Kind) -> Option<Stereo> {
    let (shortest, longest) = match kind {
        Kind::Kick => (0.1, 0.45),
        Kind::Snare => (0.08, 0.35),
        Kind::Hat => (0.03, 0.15),
    };
    let next = |t: f64| hits.iter().map(|h| h.t).filter(|u| *u > t + 0.02).fold(f64::MAX, f64::min);
    let best = hits
        .iter()
        .filter(|h| h.kind == kind && next(h.t) - h.t >= shortest)
        .max_by(|a, b| a.level.total_cmp(&b.level))?;
    let end = (best.t + longest).min(next(best.t) - 0.003);
    let (a, b) = (((best.t - 0.0015).max(0.0) * SRF) as usize, ((end * SRF) as usize).min(x.len()));
    if b <= a + 64 {
        return None;
    }
    let mut s = x.slice(a, b);
    let len = s.len();
    s.fade(((0.0015 * SRF) as usize).min(len / 4), ((0.012 * SRF) as usize).min(len / 2));
    s.normalize(-1.0);
    Some(s)
}

fn cut_break(x: &Stereo, f: &Found, g: &GenreProfile, dir: &Path) -> Result<Cut> {
    let secs = x.len() as f64 / SRF;
    let (bpm, beats, start) = if secs <= 16.0 {
        let (bpm, beats) = loop_tempo(secs, said_bpm(f), g.bpm).context("no tempo fits its length")?;
        (bpm, beats, 0.0)
    } else {
        let a = analysis::analyze(x, Some((60.0, 180.0)));
        if a.bar_secs <= 0.0 {
            bail!("no beat to cut it on");
        }
        let bars = if a.first_downbeat + 2.0 * a.bar_secs <= secs { 2.0 } else { 1.0 };
        (said_bpm(f).filter(|s| (s / a.bpm - 1.0).abs() < 0.04).unwrap_or(a.bpm), bars * 4.0, a.first_downbeat)
    };
    let (s0, s1) = ((start * SRF) as usize, (((start + beats * 60.0 / bpm) * SRF) as usize).min(x.len()));
    let mut seg = x.slice(s0, s1);
    let len = seg.len();
    seg.fade(32.min(len), 64.min(len));
    let file = dir.join("break.wav");
    let hits = drums::hear(&seg);
    let mut c = Cut::new(blank(&file, Role::Break, bpm, beats));
    for kind in Kind::ALL {
        if let Some(hit) = cut_hit(&seg, &hits, kind) {
            let name = match kind {
                Kind::Kick => "kick",
                Kind::Snare => "snare",
                Kind::Hat => "hat",
            };
            let path = Some(c.file(dir.join(format!("{name}.wav")), hit, true));
            match kind {
                Kind::Kick => c.prepared.kick = path,
                Kind::Snare => c.prepared.snare = path,
                Kind::Hat => c.prepared.hat = path,
            }
        }
    }
    c.files.insert(0, CutFile { path: file, audio: seg, float: false });
    Ok(c)
}

/// how many places in a recording a phrase is listened for at most, spread over it
const STARTS: usize = 24;

/// a passage of a recording `cut_phrase` weighs: the bar it starts on, the section of the recording it is in, its
/// power, the ears' z for what the genre listens for, and how much the recording plays it again (`repeats`)
struct Passed {
    bar: usize,
    section: usize,
    power: f64,
    z: Option<f64>,
    repeats: f64,
}

fn cut_phrase(x: &Stereo, n: &SampleNeed, ears: Option<&Ears>, dir: &Path) -> Result<Cut> {
    let a = analysis::analyze(x, Some((55.0, 180.0)));
    let bar = a.bar_secs;
    if bar <= 0.0 || a.bars < PHRASE_BARS {
        bail!("no bars to cut it on");
    }
    // where a phrase may start: every two bars of the sections that hold harmony, else of the whole recording
    let mut starts: Vec<(usize, usize)> = Vec::new();
    for (si, s) in a.sections.iter().enumerate().filter(|(_, s)| s.harmony) {
        let mut b = s.start_bar;
        while b + PHRASE_BARS <= s.start_bar + s.bars {
            starts.push((b, si));
            b += 2;
        }
    }
    if starts.is_empty() {
        starts = (0..=a.bars - PHRASE_BARS).step_by(2).map(|b| (b, 0)).collect();
    }
    if starts.len() > STARTS {
        starts = (0..STARTS).map(|i| starts[i * starts.len() / STARTS]).collect();
    }
    let cut = |b: usize| -> Option<Stereo> {
        let t0 = a.first_downbeat + b as f64 * bar;
        let (s0, s1) = ((t0 * SRF) as usize, ((t0 + PHRASE_BARS as f64 * bar) * SRF) as usize);
        (s1 <= x.len()).then(|| x.slice(s0, s1))
    };
    let power = |s: &Stereo| s.mono().iter().map(|v| (v * v) as f64).sum::<f64>() / s.len().max(1) as f64;
    let mut passages: Vec<Passed> = Vec::new();
    for &(b, section) in &starts {
        let Some(seg) = cut(b) else { continue };
        let z = match ears {
            Some(e) => Some(e.z(&seg, &n.listen)?),
            None => None,
        };
        passages.push(Passed { bar: b, section, power: power(&seg), z, repeats: repeats(&a.bar_chroma, b) });
    }
    let loudest = passages.iter().map(|p| p.power).fold(0.0, f64::max);
    let level = |p: &Passed| if loudest > 0.0 { (p.power / loudest).sqrt() } else { 0.0 };
    // the ears choose the passage most like what the genre listens for, without them the loudest, and either way
    // rather one the recording plays again, which loops as the recording plays it
    let score = |p: &Passed| p.z.unwrap_or_else(|| level(p)) + p.repeats;
    let best =
        |ok: &dyn Fn(&Passed) -> bool| passages.iter().filter(|p| ok(p)).min_by(|x, y| score(y).total_cmp(&score(x)));
    let first = best(&|p| p.power > 0.0).context("no whole phrase in it")?;
    let chords = |b: usize| a.numerals.get(b..b + PHRASE_BARS).map(<[String]>::to_vec).unwrap_or_default();
    // the other sections of a song play another passage: one the ears hear what the genre listens for in too, or,
    // without them, one at most 6 dB quieter; from another section of the recording, else on other chords
    let other = |p: &Passed| p.bar.abs_diff(first.bar) >= PHRASE_BARS && p.z.map_or(level(p) >= 0.5, |z| z >= MIN_Z);
    let second = best(&|p| other(p) && p.section != first.section)
        .or_else(|| best(&|p| other(p) && chords(p.bar) != chords(first.bar)));
    let take = |b: usize| -> Result<Stereo> {
        let mut seg = cut(b).context("no whole phrase in it")?;
        let len = seg.len();
        seg.fade(64.min(len), 256.min(len));
        Ok(seg)
    };
    let file = dir.join("phrase.wav");
    let mut c = Cut::new(blank(&file, Role::Phrase, a.bpm, (PHRASE_BARS * 4) as f64));
    c.prepared.key = Some(a.key.tonic);
    c.prepared.minor = a.key.mode.is_minor();
    c.prepared.chords = chords(first.bar);
    c.prepared.z = first.z;
    c.file(file, take(first.bar)?, false);
    if let Some(s) = second {
        let path = c.file(dir.join("phrase-2.wav"), take(s.bar)?, false);
        c.prepared.second = Some(Passage { path, chords: chords(s.bar), louder: s.power > first.power });
    }
    Ok(c)
}

/// How much a recording plays the bars of a phrase from bar `b` again, in the bars after them or in those before:
/// the correlation of their pitch classes (`chroma`, a bar's each) with those bars', from -1 to 1; 0 with no bars on
/// either side.
fn repeats(chroma: &[[f32; 12]], b: usize) -> f64 {
    let at = |from: usize| chroma.get(from..from + PHRASE_BARS);
    let Some(own) = at(b) else { return 0.0 };
    [Some(b + PHRASE_BARS), b.checked_sub(PHRASE_BARS)]
        .into_iter()
        .flatten()
        .filter_map(|o| correlation(own, at(o)?))
        .reduce(f64::max)
        .unwrap_or(0.0)
}

fn correlation(x: &[[f32; 12]], y: &[[f32; 12]]) -> Option<f64> {
    let flat = |v: &[[f32; 12]]| -> Vec<f64> { v.iter().flatten().map(|s| *s as f64).collect() };
    let (x, y) = (flat(x), flat(y));
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
    let (mx, my) = (mean(&x), mean(&y));
    let (mut xy, mut xx, mut yy) = (0.0, 0.0, 0.0);
    for (a, b) in x.iter().zip(&y) {
        let (da, db) = (a - mx, b - my);
        xy += da * db;
        xx += da * da;
        yy += db * db;
    }
    (xx > 0.0 && yy > 0.0).then(|| xy / (xx * yy).sqrt())
}

fn cut_texture(x: &Stereo, dir: &Path) -> Result<Cut> {
    let secs = x.len() as f64 / SRF;
    let take = secs.min(16.0);
    let start = ((secs - take) / 2.0).max(0.0);
    let mut seg = x.slice((start * SRF) as usize, (((start + take) * SRF) as usize).min(x.len()));
    let len = seg.len();
    let fade = ((0.4 * SRF) as usize).min(len / 4);
    seg.fade(fade, fade);
    let file = dir.join("texture.wav");
    // a texture has no tempo: it is warped as a loop of two beats a second, which it stretches along with
    let mut c = Cut::new(blank(&file, Role::Texture, 120.0, (take * 2.0).floor().max(1.0)));
    c.file(file, seg, false);
    Ok(c)
}

/// a recording's tempo, halved or doubled, inside `range`: the one nearest its middle, if any
pub fn fit_tempo(bpm: f64, range: (f64, f64)) -> Option<f64> {
    let mid = (range.0 + range.1) / 2.0;
    [bpm, bpm / 2.0, bpm * 2.0]
        .into_iter()
        .filter(|t| *t >= range.0 && *t <= range.1)
        .min_by(|a, b| (a - mid).abs().total_cmp(&(b - mid).abs()))
}

/// a fusion samples as the genre that leads it
fn lead(genre: &str) -> &str {
    genre.split('+').next().unwrap_or(genre)
}

/// a loop's tempo and beats, halved or doubled toward `target`: a break of 8 beats at 150 plays as 4 at 75 under a
/// song at 80, rather than slowed to half its speed
fn fold(bpm: f64, beats: f64, target: f64) -> (f64, f64) {
    [(bpm, beats), (bpm / 2.0, beats / 2.0), (bpm * 2.0, beats * 2.0)]
        .into_iter()
        .filter(|(_, b)| *b >= 1.0)
        .min_by(|a, b| (a.0 / target).ln().abs().total_cmp(&(b.0 / target).ln().abs()))
        .unwrap_or((bpm, beats))
}

/// The semitones to play a phrase of `bpm` up or down by, within `pitch`, so that its tempo, which moves with its
/// pitch as a sampler's varispeed moves it, falls in the genre's `range` (halved or doubled): the fewest semitones,
/// then the tempo nearest the range's middle, `rng` choosing between near equals; and the song's tempo that comes of
/// it.
pub fn speed_for(bpm: f64, range: (f64, f64), pitch: (i32, i32), rng: &mut Rng) -> Option<(i32, f64)> {
    let mut best: Option<(f64, i32, f64)> = None;
    for (semis, t, cost) in speeds(bpm, range, pitch) {
        let cost = cost - rng.f64() * 0.3;
        if best.is_none_or(|(c, _, _)| cost < c) {
            best = Some((cost, semis, t));
        }
    }
    best.map(|(_, semis, t)| (semis, (t * 100.0).round() / 100.0))
}

/// every speed `speed_for` chooses among: the semitones, the tempo, and what it costs
fn speeds(bpm: f64, range: (f64, f64), pitch: (i32, i32)) -> Vec<(i32, f64, f64)> {
    let mid = (range.0 * range.1).sqrt();
    let mut out = Vec::new();
    for semis in pitch.0.min(pitch.1)..=pitch.0.max(pitch.1) {
        let sped = bpm * 2f64.powf(semis as f64 / 12.0);
        for t in [sped, sped / 2.0, sped * 2.0] {
            if (range.0..=range.1).contains(&t) {
                out.push((semis, t, semis.abs() as f64 + 3.0 * (t / mid).ln().abs()));
            }
        }
    }
    out
}

/// Whether a song of `g` can play `p` at the genre's tempo as it was played: where claudius can stretch a loop in time
/// without changing its pitch, any; else (in a browser, which only varispeeds) a phrase some speed fits in tune, and a
/// break whose tempo, or half or twice it, lies within about a semitone of the genre's. A texture has no tempo.
pub fn plays_in(p: &Prepared, g: &GenreProfile) -> bool {
    plays(p, g, cfg!(feature = "native"))
}

/// `plays_in`, where claudius `stretches` or does not
fn plays(p: &Prepared, g: &GenreProfile, stretches: bool) -> bool {
    match p.role {
        Role::Texture => true,
        _ if stretches => true,
        Role::Phrase => need(g, Role::Phrase).is_some_and(|n| !speeds(p.bpm, g.bpm, n.pitch).is_empty()),
        Role::Break => [p.bpm, p.bpm / 2.0, p.bpm * 2.0].iter().any(|t| *t >= g.bpm.0 * 0.94 && *t <= g.bpm.1 * 1.06),
    }
}

/// the tempo a loop plays at in a song at `bpm` before it is sped up or slowed down by `semis`
fn unsped(bpm: f64, semis: i32) -> f64 {
    bpm / 2f64.powf(semis as f64 / 12.0)
}

/// the moves a phrase may be treated with (`SampleNeed::treat`)
pub const MOVES: [&str; 7] = [
    "as it is",
    "filtered until it lifts",
    "darkened",
    "a telephone's band in the intro and the breaks",
    "overloaded",
    "smashed",
    "crushed",
];

/// What a move adds to a phrase's mix: the effects after its own and the drive that overloads it; and the kinds of
/// section it works on, one of which the phrase must play in for the move to be heard (none: it works all through).
struct Move {
    fx: Vec<Effect>,
    drive_db: f32,
    works_in: &'static [SectionKind],
}

/// a move by its name (`MOVES`)
fn treatment(name: &str) -> Option<Move> {
    use crate::dsp::env::{BySection, Param};
    use crate::dsp::filter::FilterMode;
    let by = |rest: f32, at: &[(&str, f32)]| {
        Param::Sections(BySection { rest, sections: at.iter().map(|(k, v)| (k.to_string(), vec![*v])).collect() })
    };
    let filter = |mode, cutoff, mix| Effect::Filter { mode, cutoff, res: Param::Const(0.1), drive: 1.0, mix };
    let nothing = |fx: Vec<Effect>, drive_db: f32| Move { fx, drive_db, works_in: &[] };
    Some(match name {
        "as it is" => nothing(Vec::new(), 0.0),
        // closed to 1.4 kHz, opening across a build before a lift, and out of the way where the song lifts
        "filtered until it lifts" => {
            let mut cutoff = by(1400.0, &[("drop", 16_000.0), ("hook", 16_000.0)]);
            if let Param::Sections(s) = &mut cutoff {
                s.sections.insert("build".into(), vec![1400.0, 16_000.0]);
            }
            Move {
                fx: vec![filter(FilterMode::Lp, cutoff, by(1.0, &[("drop", 0.0), ("hook", 0.0)]))],
                drive_db: 0.0,
                works_in: &[SectionKind::Drop, SectionKind::Hook],
            }
        }
        "darkened" => nothing(vec![filter(FilterMode::Lp, Param::Const(2600.0), Param::Const(1.0))], 0.0),
        // 450 Hz to 3.2 kHz in the intro and the breaks, as it is elsewhere
        "a telephone's band in the intro and the breaks" => {
            let there = by(0.0, &[("intro", 1.0), ("break", 1.0)]);
            Move {
                fx: vec![
                    filter(FilterMode::Hp, Param::Const(450.0), there.clone()),
                    filter(FilterMode::Lp, Param::Const(3200.0), there),
                ],
                drive_db: 0.0,
                works_in: &[SectionKind::Intro, SectionKind::Break],
            }
        }
        "overloaded" => nothing(Vec::new(), 9.0),
        "smashed" => nothing(Vec::new(), 15.0),
        // a 12-bit sampler at half the rate
        "crushed" => {
            nothing(vec![Effect::Bitcrush { bits: Param::Const(12.0), downsample: 2.0, mix: 1.0.into() }], 0.0)
        }
        _ => return None,
    })
}

/// The move a song's phrase is treated with: one of the genre's (`treat`) that it would be heard in, the sections the
/// phrase plays in being `kinds`, picked by `rng` as often as each is made; none to make, or one that is no move: as
/// it is.
fn treat(n: &SampleNeed, kinds: &[SectionKind], rng: &mut Rng) -> (&'static str, Move) {
    let heard: Vec<(&'static str, f64, Move)> = MOVES
        .iter()
        .filter_map(|&name| {
            let weight = n.treat.get(name).copied().filter(|w| *w > 0.0)?;
            let m = treatment(name)?;
            (m.works_in.is_empty() || m.works_in.iter().any(|k| kinds.contains(k))).then_some((name, weight, m))
        })
        .collect();
    let total: f64 = heard.iter().map(|(_, w, _)| w).sum();
    let at = rng.f64() * total;
    let i = heard
        .iter()
        .scan(0.0, |sum, (_, w, _)| {
            *sum += w;
            Some(*sum)
        })
        .position(|sum| at < sum)
        .unwrap_or(heard.len().saturating_sub(1));
    match heard.into_iter().nth(i) {
        Some((name, _, m)) => (name, m),
        None => ("as it is", Move { fx: Vec::new(), drive_db: 0.0, works_in: &[] }),
    }
}

/// The sections that play a phrase's second passage in its place: the louder of the two passages plays where the song
/// lifts (its drops and hooks) and the other elsewhere; in a song that only lifts, or never does, every other section
/// the phrase plays in, from the second.
fn second_sections(bp: &Blueprint, louder: bool) -> Vec<usize> {
    let playing: Vec<usize> = (0..bp.form.len()).filter(|&i| bp.form[i].layers.iter().any(|l| l == PHRASE)).collect();
    let (lifts, rest): (Vec<usize>, Vec<usize>) =
        playing.iter().partition(|&&i| matches!(bp.form[i].kind, SectionKind::Drop | SectionKind::Hook));
    if lifts.is_empty() || rest.is_empty() {
        playing.into_iter().skip(1).step_by(2).collect()
    } else if louder {
        lifts
    } else {
        rest
    }
}

/// Make room for `role`'s track in a song whose genre's sections do not list it: it plays in place of the layers its
/// need names `instead`, in each section that has one of them (in every section, when it names none).
fn place(bp: &mut Blueprint, role: Role, n: &SampleNeed) {
    let name = track_name(role);
    if bp.form.iter().any(|s| s.layers.iter().any(|l| l == name)) {
        return;
    }
    for s in bp.form.iter_mut() {
        if n.instead.is_empty() || s.layers.iter().any(|l| n.instead.contains(l)) {
            s.layers.retain(|l| !n.instead.contains(l));
            s.layers.push(name.to_string());
        }
    }
}

/// Put the genre's prepared samples into `bp` as it is composed (see the module's doc), when the song is built on
/// samples (`sampled`). A role without one at hand leaves the sections that list it, and the layers the genre names
/// `instead` play there.
pub fn apply(
    bp: &mut Blueprint,
    g: &GenreProfile,
    opts: &crate::compose::Options,
    lib: Option<&dyn crate::compose::Library>,
    rng: &Rng,
) {
    let Some(_) = &g.sampling else { return };
    let search = opts.sample_search.as_deref().and_then(words);
    let on = sampled(g, opts.sampled, search.as_deref());
    let tempo_given = opts.bpm.is_some();
    let mut placed: Vec<(Role, String)> = Vec::new();
    for role in [Role::Phrase, Role::Break, Role::Texture] {
        let Some(n) = need(g, role) else { continue };
        let mut r = rng.fork(role.name());
        if !on || !r.chance(n.chance) {
            continue;
        }
        let asked = search.as_deref().filter(|_| role == Role::Phrase);
        let Some(p) = lib.and_then(|l| l.prepared(role, g, asked, &mut r)) else { continue };
        // a phrase plays sped up or slowed down by whole semitones, as a sampler plays it, to the genre's tempo (or the
        // one given); one that no such speed fits is stretched to it in tune, where claudius can stretch (not in a
        // browser, which would play it out of tune, so it goes without)
        let speed = match (role, tempo_given) {
            (Role::Phrase, false) => speed_for(p.bpm, g.bpm, n.pitch, &mut r),
            (Role::Phrase, true) => {
                speed_for(p.bpm, (bp.bpm * 0.997, bp.bpm * 1.003), n.pitch, &mut r).map(|(s, _)| (s, bp.bpm))
            }
            _ => None,
        };
        if role == Role::Phrase && speed.is_none() && !cfg!(feature = "native") {
            continue;
        }
        let semis = speed.map_or(0, |(s, _)| s);
        place(bp, role, n);
        // a phrase is treated as the genre's producers treat a record, the move its seed picks
        let mut mix = n.mix.clone();
        let mut treated = String::new();
        if role == Role::Phrase {
            let kinds: Vec<SectionKind> =
                bp.form.iter().filter(|s| s.layers.iter().any(|l| l == PHRASE)).map(|s| s.kind).collect();
            let (name, m) = treat(n, &kinds, &mut r.fork("treat"));
            mix.fx.extend(m.fx);
            mix.drive_db = mix.drive_db.max(m.drive_db);
            if name != "as it is" {
                treated = format!(", {name}");
            }
        }
        // the second passage of a phrase, and its chords as the song counts them
        let mut second: Option<(&Passage, Vec<String>)> = None;
        if role == Role::Phrase {
            // the song is built on the phrase: its key, its tempo where the genre allows, its chords
            let mut heard = None;
            if let Some(k) = p.key {
                let was = bp.key;
                let k = (k as i32 + semis).rem_euclid(12) as u8;
                let h = Key::new(k, if p.minor { Mode::Minor } else { Mode::Major });
                // a genre that knows no major mode plays a major phrase in its relative minor, where the phrase's
                // chord on its first degree is the third (a swell on A major is III of F sharp minor)
                let dark = !g.modes.is_empty() && g.modes.iter().all(|(m, _)| m.is_minor());
                bp.key = if !p.minor && dark { Key::new((k + 9) % 12, Mode::Minor) } else { h };
                // the other progressions move into the new key as key() would move them
                for prog in bp.harmony.progressions.values_mut() {
                    *prog = prog.iter().map(|rn| crate::script::remode(rn, &was, &bp.key)).collect();
                }
                heard = Some(h);
            }
            match speed {
                Some((_, t)) => bp.bpm = t,
                None if !tempo_given => {
                    if let Some(t) = fit_tempo(p.bpm, g.bpm) {
                        bp.bpm = t.round();
                    }
                }
                None => {}
            }
            // a bar of the phrase as the song counts it: counted at half its tempo, a song's bar holds two of its
            // bars (and plays the first one's chord); at twice its tempo, each of its chords lasts two
            let (_, beats) = fold(p.bpm, p.beats, unsped(bp.bpm, semis));
            let key = bp.key;
            let counted = |numerals: &[String]| -> (Vec<String>, u32) {
                let chords: Vec<String> = match &heard {
                    Some(h) => {
                        let cs = crate::theory::progression::chords(numerals, h);
                        if cs.len() == numerals.len() {
                            cs.iter().map(|c| crate::theory::chord::roman_of(c, &key)).collect()
                        } else {
                            Vec::new()
                        }
                    }
                    None => numerals.to_vec(),
                };
                if beats < p.beats {
                    (chords.iter().step_by(2).cloned().collect(), 1)
                } else {
                    (chords, if beats > p.beats { 2 } else { 1 })
                }
            };
            let (chords, per) = counted(&p.chords);
            if !chords.is_empty() {
                bp.harmony.progressions.insert("A".into(), chords);
                bp.harmony.bars_per_chord = per;
            }
            // a recording plays its own chords: the sections it plays in play them too
            for s in bp.form.iter_mut().filter(|s| s.layers.iter().any(|l| l == PHRASE)) {
                s.prog = "A".into();
            }
            second = p.second.as_ref().map(|s| (s, counted(&s.chords).0));
        }
        let tonal = role == Role::Phrase && p.key.is_some();
        let (bpm, beats) =
            if role == Role::Texture { (p.bpm, p.beats) } else { fold(p.bpm, p.beats, unsped(bp.bpm, semis)) };
        let slices = (beats / 0.5).round().max(1.0) as i32;
        let chop: Vec<i32> = if slices >= 4 && r.chance(n.chop) {
            (0..slices).map(|i| if i % 4 == 0 { i } else { r.below(slices as usize) as i32 }).collect()
        } else {
            Vec::new()
        };
        let label = Path::new(&p.path)
            .parent()
            .and_then(|d| d.file_name())
            .map_or_else(|| p.path.clone(), |f| f.to_string_lossy().into_owned());
        bp.loops.retain(|l| l.name != track_name(role));
        bp.loops.push(LoopPart {
            name: track_name(role).into(),
            // the mix levels it as a drum loop, a sample or a texture (`render::target_level`)
            role: match role {
                Role::Break => "drums",
                Role::Phrase => "sample",
                Role::Texture => "texture",
            }
            .into(),
            sound: Sound::Loop {
                path: p.path.clone(),
                bpm,
                beats,
                key: p.key.filter(|_| tonal),
                tonal,
                label: label.clone(),
            },
            transpose: semis,
            chop,
            slice_beats: 0.5,
            mix,
            from: None,
            sampled: false,
        });
        if role == Role::Phrase {
            bp.loops.retain(|l| l.name != SECOND);
        }
        let mut also = String::new();
        if let Some((s, chords)) = second
            && let Some(mut other) = bp.loops.last().cloned()
        {
            let at = second_sections(bp, s.louder);
            if !at.is_empty() {
                other.name = SECOND.into();
                if let Sound::Loop { path, label, .. } = &mut other.sound {
                    *path = s.path.clone();
                    *label = format!("{label}, another passage");
                }
                bp.loops.push(other);
                for &i in &at {
                    let sec = &mut bp.form[i];
                    for l in sec.layers.iter_mut().filter(|l| *l == PHRASE) {
                        *l = SECOND.into();
                    }
                    if !chords.is_empty() {
                        sec.prog = SECOND_PROG.into();
                    }
                }
                if !chords.is_empty() {
                    bp.harmony.progressions.insert(SECOND_PROG.into(), chords);
                }
                let names: Vec<&str> = at.iter().map(|&i| bp.form[i].name.as_str()).collect();
                also = format!(" (another passage of it in {})", names.join(", "));
            }
        }
        if role == Role::Break {
            // the drum lanes it plays in place of play the break's own hits (a break under house's kick leaves it be)
            for lane in bp.drums.iter_mut().filter(|d| n.instead.contains(&d.name)) {
                let hit = match lane.role().as_str() {
                    "kick" => p.kick.clone(),
                    "snare" | "clap" => p.snare.clone(),
                    "hat" => p.hat.clone(),
                    _ => None,
                };
                if let Some(path) = hit {
                    lane.sound = Sound::Sample {
                        path,
                        root: 60,
                        start: 0.0,
                        end: None,
                        label: format!("{label} {}", lane.name),
                    };
                }
            }
        }
        let how = match semis {
            0 => String::new(),
            s => format!(
                " {} semitone{} {}",
                s.abs(),
                if s.abs() == 1 { "" } else { "s" },
                if s > 0 { "up" } else { "down" }
            ),
        };
        placed.push((role, format!("'{label}'{how}{treated}{also}")));
    }
    for role in Role::ALL {
        let Some(n) = need(g, role) else { continue };
        if placed.iter().any(|(r, _)| *r == role) {
            continue;
        }
        let name = track_name(role);
        let known = bp.layer_names();
        for s in bp.form.iter_mut() {
            if s.layers.iter().any(|l| l == name) {
                s.layers.retain(|l| l != name);
                for l in &n.instead {
                    if known.contains(l) && !s.layers.contains(l) {
                        s.layers.push(l.clone());
                    }
                }
            }
        }
    }
    if !placed.is_empty() {
        let said: Vec<String> = placed.iter().map(|(r, l)| format!("{} {l}", r.name())).collect();
        bp.lineage.push(format!("sampled: {}", said.join(", ")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(title: &str, tags: &[&str]) -> Found {
        Found {
            id: "freesound:1".into(),
            source: online::Source::Openverse,
            provider: "freesound".into(),
            title: title.into(),
            creator: "x".into(),
            licence: online::Licence::Cc0,
            licence_name: "CC0 1.0".into(),
            licence_url: String::new(),
            page: String::new(),
            audio: String::new(),
            filetype: "mp3".into(),
            seconds: None,
            tags: tags.iter().map(|t| t.to_string()).collect(),
            year: None,
            by_age: false,
        }
    }

    #[test]
    fn a_tempo_is_read_from_a_title_or_from_ccmixters_tags() {
        assert_eq!(said_bpm(&found("Crunchy Drum Break 2 [128bpm]", &[])), Some(128.0));
        assert_eq!(said_bpm(&found("Chill Lofi Epiano Loop 80 BPM.wav", &[])), Some(80.0));
        assert_eq!(said_bpm(&found("Lost Roamin'", &["bpm_080_085"])), Some(82.5));
        assert_eq!(said_bpm(&found("Silly Drum Break 1.wav", &[])), None);
    }

    #[test]
    fn a_short_loop_is_a_whole_number_of_bars_at_the_tempo_it_states_or_near_the_genres() {
        // 15 s said to be 128: 32 beats
        let (bpm, beats) = loop_tempo(15.0, Some(128.0), (80.0, 96.0)).unwrap();
        assert_eq!(beats, 32.0);
        assert!((bpm - 128.0).abs() < 0.1, "{bpm}");
        // 3.18 s with nothing said, for a genre at 80 to 96: 4 beats at 75.6 is nearer than 8 at 151 halved
        let (bpm, beats) = loop_tempo(3.176, None, (80.0, 96.0)).unwrap();
        assert!((bpm - 75.57).abs() < 0.1 && beats == 4.0, "{bpm} {beats}");
    }

    struct Shelf(Vec<Prepared>);

    impl crate::compose::Library for Shelf {
        fn one_shot(&self, _: &str, _: &[String], _: &GenreProfile, _: &mut Rng) -> Option<Sound> {
            None
        }
        fn find_loop(
            &self,
            _: &crate::genres::LoopSpec,
            _: &GenreProfile,
            _: f64,
            _: &Key,
            _: &mut Rng,
        ) -> Option<(Sound, i32)> {
            None
        }
        fn prepared(&self, role: Role, g: &GenreProfile, asked: Option<&str>, _: &mut Rng) -> Option<Prepared> {
            self.0.iter().find(|p| p.role == role && p.fits(lead(&g.name), asked)).cloned()
        }
    }

    fn sampling_genre() -> GenreProfile {
        crate::genres::get("trip_hop").unwrap()
    }

    fn shelf() -> Shelf {
        let p = |role: Role, path: &str| Prepared {
            path: path.into(),
            role,
            genre: "trip_hop".into(),
            bpm: 150.0,
            beats: 8.0,
            key: None,
            minor: false,
            kick: None,
            snare: None,
            hat: None,
            chords: Vec::new(),
            z: Some(2.0),
            source: "freesound:1".into(),
            asked: None,
            second: None,
        };
        Shelf(vec![
            Prepared {
                kick: Some("/b/kick.wav".into()),
                snare: Some("/b/snare.wav".into()),
                ..p(Role::Break, "/b/break.wav")
            },
            Prepared {
                bpm: 176.0,
                key: Some(2),
                minor: true,
                chords: vec!["i".into(), "VI".into()],
                ..p(Role::Phrase, "/p/phrase.wav")
            },
            p(Role::Texture, "/t/texture.wav"),
        ])
    }

    #[test]
    fn a_song_is_built_on_its_phrase_and_plays_its_break_and_the_breaks_own_hits() {
        let g = sampling_genre();
        let opts = crate::compose::Options::default();
        let bp = crate::compose::generate(&g, 3, &opts, Some(&shelf()));
        // the phrase's key, its tempo halved into trip-hop's, and its chords as the song counts its bars: its two bars
        // at 176 are one at 88, which plays the first one's chord
        assert_eq!((bp.key.tonic, bp.key.mode), (2, Mode::Minor));
        assert_eq!(bp.bpm, 88.0);
        assert_eq!(bp.harmony.progressions["A"], ["i"]);
        assert_eq!(bp.harmony.bars_per_chord, 1);
        let names: Vec<&str> = bp.loops.iter().map(|l| l.name.as_str()).collect();
        assert!(names.contains(&BREAK) && names.contains(&PHRASE) && names.contains(&TEXTURE), "{names:?}");
        // a break of 8 beats at 150 plays as 4 at 75 under a song at 88
        let brk = bp.loops.iter().find(|l| l.name == BREAK).unwrap();
        assert!(matches!(brk.sound, Sound::Loop { bpm, beats, tonal: false, .. } if bpm == 75.0 && beats == 4.0));
        let kick = bp.drums.iter().find(|d| d.name == "kick").unwrap();
        assert!(matches!(&kick.sound, Sound::Sample { path, .. } if path == "/b/kick.wav"), "{:?}", kick.sound);
        assert!(bp.form.iter().any(|s| s.layers.contains(&BREAK.to_string())));
        assert!(bp.lineage.iter().any(|l| l.starts_with("sampled: phrase")), "{:?}", bp.lineage);
    }

    #[test]
    fn a_phrases_chords_last_as_long_as_its_bars_do_in_the_song() {
        let g = sampling_genre();
        let at = |bpm: f64| {
            let mut s = shelf();
            for p in s.0.iter_mut().filter(|p| p.role == Role::Phrase) {
                p.bpm = bpm;
            }
            let bp = crate::compose::generate(&g, 3, &crate::compose::Options::default(), Some(&s));
            let phrase = bp.loops.iter().find(|l| l.name == PHRASE).unwrap();
            let Sound::Loop { beats, .. } = phrase.sound else { panic!("a loop") };
            (bp.bpm, beats, bp.harmony.progressions["A"].clone(), bp.harmony.bars_per_chord)
        };
        // two bars at its own tempo: a chord a bar
        assert_eq!(at(88.0), (88.0, 8.0, vec!["i".to_string(), "VI".to_string()], 1));
        // two bars at 44 are four at 88: each chord lasts two
        assert_eq!(at(44.0), (88.0, 16.0, vec!["i".to_string(), "VI".to_string()], 2));
    }

    #[test]
    fn a_phrases_other_passage_plays_where_the_song_lifts_when_it_is_the_louder_on_its_own_chords() {
        let g = sampling_genre();
        let song = |second: Option<bool>| {
            let mut s = shelf();
            for p in s.0.iter_mut().filter(|p| p.role == Role::Phrase) {
                // at the song's tempo: a chord a bar
                p.bpm = 88.0;
                p.second = second.map(|louder| Passage {
                    path: "/p/phrase-2.wav".into(),
                    chords: vec!["iv".into(), "v".into()],
                    louder,
                });
            }
            crate::compose::generate(&g, 3, &crate::compose::Options::default(), Some(&s))
        };
        let on = |bp: &Blueprint, layer: &str| -> Vec<SectionKind> {
            bp.form.iter().filter(|s| s.layers.iter().any(|l| l == layer)).map(|s| s.kind).collect()
        };
        let lifts = |k: &SectionKind| matches!(k, SectionKind::Drop | SectionKind::Hook);
        let plain = song(None);
        let playing = on(&plain, PHRASE);
        assert!(playing.iter().any(lifts) && !playing.iter().all(lifts), "{playing:?}");
        assert!(plain.loops.iter().all(|l| l.name != SECOND));
        // the louder passage where the song lifts, the phrase where it played the rest of the time
        let bp = song(Some(true));
        let (first, second) = (on(&bp, PHRASE), on(&bp, SECOND));
        assert_eq!(first.len() + second.len(), playing.len());
        assert!(!second.is_empty() && second.iter().all(lifts) && !first.iter().any(lifts), "{first:?} {second:?}");
        let other = bp.loops.iter().find(|l| l.name == SECOND).expect("the other passage plays");
        let phrase = bp.loops.iter().find(|l| l.name == PHRASE).unwrap();
        let Sound::Loop { path, bpm, beats, .. } = &other.sound else { panic!("a loop") };
        assert_eq!(path, "/p/phrase-2.wav");
        assert!(matches!(phrase.sound, Sound::Loop { bpm: b, beats: n, .. } if b == *bpm && n == *beats));
        assert_eq!((other.transpose, &other.mix.fx), (phrase.transpose, &phrase.mix.fx));
        // on its own chords, and the genre's own B left to the sections on it
        assert_eq!(bp.harmony.progressions[SECOND_PROG], ["iv", "v"]);
        assert!(bp.form.iter().filter(|s| s.layers.iter().any(|l| l == SECOND)).all(|s| s.prog == SECOND_PROG));
        assert_eq!(bp.harmony.progressions.get("B"), plain.harmony.progressions.get("B"));
        assert!(bp.form.iter().filter(|s| s.prog == "B").all(|s| !s.layers.iter().any(|l| l == SECOND)));
        assert!(bp.lineage.iter().any(|l| l.contains("another passage of it in")), "{:?}", bp.lineage);
        // the quieter one where it does not lift
        let quiet = song(Some(false));
        assert!(on(&quiet, SECOND).iter().all(|k| !lifts(k)) && on(&quiet, PHRASE).iter().all(lifts));
        // in a song that never lifts, every other section the phrase plays in
        let mut flat = plain.clone();
        flat.form.iter_mut().for_each(|s| s.kind = SectionKind::Groove);
        let at: Vec<usize> =
            (0..flat.form.len()).filter(|&i| flat.form[i].layers.iter().any(|l| l == PHRASE)).collect();
        assert_eq!(second_sections(&flat, true), at.iter().copied().skip(1).step_by(2).collect::<Vec<_>>());
    }

    #[test]
    fn a_phrase_is_treated_with_a_move_its_genre_makes_and_its_seed_picks() {
        let g = sampling_genre();
        let base = need(&g, Role::Phrase).unwrap().mix.clone();
        let mut made = std::collections::BTreeSet::new();
        let mut played = 0;
        for seed in 0..40 {
            let bp = crate::compose::generate(&g, seed, &crate::compose::Options::default(), Some(&shelf()));
            // a song plays its genre's phrase as often as its chance says
            let Some(phrase) = bp.loops.iter().find(|l| l.name == PHRASE) else { continue };
            played += 1;
            let said = bp.lineage.iter().find(|l| l.starts_with("sampled: phrase")).unwrap();
            let name = MOVES.iter().skip(1).find(|m| said.contains(&format!(", {m}"))).copied().unwrap_or("as it is");
            assert!(g.sampling.as_ref().unwrap().phrase.as_ref().unwrap().treat.contains_key(name), "{said}");
            // the move's effects after the genre's own, and its drive
            let m = treatment(name).unwrap();
            assert_eq!(phrase.mix.fx[..base.fx.len()], base.fx[..], "{name}");
            assert_eq!(phrase.mix.fx[base.fx.len()..], m.fx[..], "{name}");
            assert_eq!(phrase.mix.drive_db, base.drive_db.max(m.drive_db), "{name}");
            made.insert(name);
        }
        assert!(played >= 32, "{played} of 40 songs played their phrase");
        assert!(made.len() >= 3, "a few moves across songs: {made:?}");
    }

    #[test]
    fn a_move_is_made_only_where_the_phrase_plays_for_it_to_be_heard() {
        let lift = |name: &str| SampleNeed { treat: [(name.to_string(), 1.0)].into(), ..soul_need() };
        let mut r = Rng::new(1);
        let verses = [SectionKind::Verse, SectionKind::Groove];
        assert_eq!(treat(&lift("filtered until it lifts"), &verses, &mut r).0, "as it is");
        assert_eq!(treat(&lift("filtered until it lifts"), &[SectionKind::Hook], &mut r).0, "filtered until it lifts");
        let telephone = "a telephone's band in the intro and the breaks";
        assert_eq!(treat(&lift(telephone), &verses, &mut r).0, "as it is");
        assert_eq!(treat(&lift(telephone), &[SectionKind::Break], &mut r).0, telephone);
        assert_eq!(treat(&lift("darkened"), &verses, &mut r).0, "darkened", "it works all through");
        assert_eq!(treat(&lift("no such move"), &verses, &mut r).0, "as it is");
        // every genre's moves are moves claudius makes
        for g in crate::genres::all().unwrap() {
            for role in Role::ALL {
                for name in need(&g, role).map(|n| n.treat.keys().collect::<Vec<_>>()).unwrap_or_default() {
                    assert!(MOVES.contains(&name.as_str()), "{}: {name}", g.name);
                }
            }
        }
    }

    fn soul_need() -> SampleNeed {
        need(&sampling_genre(), Role::Phrase).unwrap().clone()
    }

    #[test]
    fn a_passage_the_recording_plays_again_repeats() {
        let pc = |p: usize| {
            let mut c = [0f32; 12];
            (c[p], c[(p + 7) % 12]) = (0.7, 0.3);
            c
        };
        // A, F, A, F, then C and G once
        let ch = [pc(9), pc(5), pc(9), pc(5), pc(0), pc(7)];
        assert!((repeats(&ch, 0) - 1.0).abs() < 1e-9 && (repeats(&ch, 2) - 1.0).abs() < 1e-9);
        assert!(repeats(&ch, 4) < 0.0, "{}", repeats(&ch, 4));
        assert_eq!(repeats(&ch[..2], 0), 0.0, "nothing either side");
    }

    /// sixteen bars at 120 BPM, a kick on every beat, and from each bar on the chord `chord` says for it, at its level:
    /// A minor's notes or F major's
    fn recording(chord: impl Fn(usize) -> (bool, f64)) -> Stereo {
        let n = (32.0 * SRF) as usize;
        let mut x = Stereo::zeros(n);
        for i in 0..n {
            let t = i as f64 / SRF;
            let (minor, amp) = chord((t / 2.0) as usize);
            let notes = if minor { [220.0, 261.63, 329.63] } else { [174.61, 220.0, 261.63] };
            let held: f64 = notes.iter().map(|f| (std::f64::consts::TAU * f * t).sin()).sum::<f64>() * amp;
            let tb = t % 0.5;
            let kick = (std::f64::consts::TAU * 55.0 * tb).sin() * (-tb / 0.08).exp() * 0.5;
            (x.l[i], x.r[i]) = ((held + kick) as f32, (held + kick) as f32);
        }
        x
    }

    #[test]
    fn a_phrase_and_another_passage_are_cut_from_two_sections_of_a_recording_or_on_other_chords() {
        let n = need(&sampling_genre(), Role::Phrase).unwrap().clone();
        // A minor for eight bars, then F major, louder
        let halves = recording(|bar| if bar < 8 { (true, 0.08) } else { (false, 0.14) });
        let c = cut_phrase(&halves, &n, None, Path::new("demo:phrase")).unwrap();
        let p = &c.prepared;
        let s = p.second.as_ref().expect("another passage");
        // the louder half's is the phrase, and the other half's the other passage, on other chords
        assert!(!s.louder && p.chords != s.chords, "{:?} {:?}", p.chords, s.chords);
        let paths: Vec<String> = c.files.iter().map(|f| f.path.to_string_lossy().into_owned()).collect();
        assert_eq!(paths, [p.path.clone(), s.path.clone()]);
        let power = |x: &Stereo| x.mono().iter().map(|v| (v * v) as f64).sum::<f64>();
        assert!(power(&c.files[0].audio) > power(&c.files[1].audio));
        assert_eq!(c.files[0].audio.len(), c.files[1].audio.len());
        // four bars of each all through, heard as one section: the other passage is on the other chord
        let turns = recording(|bar| (bar % 8 < 4, 0.1));
        let c = cut_phrase(&turns, &n, None, Path::new("demo:phrase")).unwrap();
        let s = c.prepared.second.as_ref().expect("another passage");
        assert_ne!(c.prepared.chords, s.chords);
        // one chord all through: nothing else to play
        let one = cut_phrase(&recording(|_| (true, 0.1)), &n, None, Path::new("demo:phrase")).unwrap();
        assert_eq!(one.prepared.second, None);
        assert_eq!(one.files.len(), 1);
    }

    #[test]
    fn a_gathering_told_to_stop_asks_no_library() {
        let root = std::env::temp_dir().join(format!("claudius-stopped-{}", std::process::id()));
        let ix = Index::at(&root);
        let mut said = Vec::new();
        let ask = Ask { seed: Some(3), search: None, most: 2 };
        let got =
            gather(&sampling_genre(), &ask, None, &Policy::default(), &ix, &|| true, &mut |s| said.push(s.to_string()))
                .unwrap();
        assert!(got.added.is_empty() && got.notes.is_empty() && said.is_empty(), "{:?} {said:?}", got.notes);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_fusion_gathers_and_plays_the_samples_of_the_genre_that_leads_it() {
        let g = crate::genres::get("trip_hop+idm").unwrap();
        assert_eq!(lead(&g.name), "trip_hop");
        let bp = crate::compose::generate(&g, 3, &crate::compose::Options::default(), Some(&shelf()));
        assert!(bp.loops.iter().any(|l| l.name == PHRASE), "trip-hop's phrase plays in trip_hop+idm");
    }

    #[test]
    fn a_dark_genre_plays_a_major_phrase_in_its_relative_minor() {
        let g = sampling_genre();
        let mut s = shelf();
        for p in s.0.iter_mut().filter(|p| p.role == Role::Phrase) {
            p.key = Some(9);
            p.minor = false;
            p.chords = vec!["I".into(), "IV".into()];
            // at the song's tempo, a chord a bar
            p.bpm = 88.0;
        }
        let bp = crate::compose::generate(&g, 3, &crate::compose::Options::default(), Some(&s));
        // A major's I and IV are III and VI of F sharp minor
        assert_eq!((bp.key.tonic, bp.key.mode), (6, Mode::Minor));
        assert_eq!(bp.harmony.progressions["A"], ["III", "VI"]);
    }

    #[test]
    fn a_phrase_plays_sped_up_or_slowed_down_by_whole_semitones_into_the_genres_tempo() {
        let mut r = Rng::new(1);
        // a soul record at 98 in house, from 120 to 126: four semitones up, at 123.47, as a sampler plays it
        assert_eq!(speed_for(98.0, (120.0, 126.0), (-2, 5), &mut r), Some((4, 123.47)));
        // at its own pitch when its tempo, or half of it, fits
        assert_eq!(speed_for(176.0, (78.0, 94.0), (-5, 1), &mut r), Some((0, 88.0)));
        // out of reach
        assert_eq!(speed_for(98.0, (120.0, 126.0), (-2, 2), &mut r), None);
    }

    #[test]
    fn where_nothing_stretches_only_what_plays_in_tune_near_the_genres_tempo_plays() {
        let house = crate::genres::get("house").unwrap();
        let at = |role: Role, bpm: f64| Prepared { role, bpm, ..soul("house", None) };
        // a phrase at 98 sped up four semitones; one at 80 no speed brings into house's tempo
        assert!(plays(&at(Role::Phrase, 98.0), &house, false) && !plays(&at(Role::Phrase, 80.0), &house, false));
        // a break at 61 doubled is near enough; one at 92 would play four semitones off
        assert!(plays(&at(Role::Break, 61.0), &house, false) && !plays(&at(Role::Break, 92.0), &house, false));
        assert!(plays(&at(Role::Texture, 92.0), &house, false), "a texture has no tempo");
        // where claudius stretches, every one plays
        assert!(plays(&at(Role::Phrase, 80.0), &house, true) && plays(&at(Role::Break, 92.0), &house, true));
    }

    #[test]
    fn one_recording_gathered_twice_is_cut_twice_each_cut_in_its_own_folder() {
        let get = |name: &str| crate::genres::get(name).unwrap();
        let (house, nu_disco) = (get("house"), get("nu_disco"));
        assert_ne!(whose(&house, None), whose(&nu_disco, None), "a genre's own");
        assert_ne!(whose(&house, None), whose(&house, Some("70s soul")), "the words' apart from the genre's");
        assert_eq!(whose(&house, Some("70s soul")), whose(&nu_disco, Some("70s soul")), "words are for any genre");
        assert_ne!(whose(&house, Some("house")), whose(&house, None), "even words that name a genre");
        assert_eq!(whose(&get("trip_hop+idm"), None), "trip_hop", "a fusion's is its lead's");
        // words a folder's name would make one are still two
        assert_ne!(whose(&house, Some("r&b")), whose(&house, Some("r b")));
        assert_ne!(whose(&house, Some("música")), whose(&house, Some("musica")));
        assert_ne!(whose(&house, Some("шансон")), whose(&house, Some("фолк")), "words with no letter a name keeps");
        assert!(whose(&house, Some("70s soul")).starts_with("asked-70s-soul-"), "and still say what they were");
    }

    #[test]
    fn a_song_is_sampled_as_asked_else_as_its_genre_is() {
        let (trip_hop, house) = (sampling_genre(), crate::genres::get("house").unwrap());
        assert!(sampled(&trip_hop, None, None) && !sampled(&house, None, None));
        assert!(sampled(&house, None, Some("disco")) && !sampled(&house, Some(false), Some("disco")));
        assert!(!sampled(&trip_hop, Some(false), None) && !sampled(&house, None, Some("   ")));
        assert_eq!(words("  70s   Soul  ").as_deref(), Some("70s soul"));
        // a trip-hop song asked not to be: the layers it plays instead, with samples at hand
        let opts = crate::compose::Options { sampled: Some(false), ..Default::default() };
        let bp = crate::compose::generate(&trip_hop, 3, &opts, Some(&shelf()));
        assert!(bp.loops.iter().all(|l| ![BREAK, PHRASE, TEXTURE].contains(&l.name.as_str())));
        let names = bp.layer_names();
        assert!(bp.form.iter().all(|s| s.layers.iter().all(|l| names.contains(l))));
    }

    /// a soul record at 98 BPM in C minor, prepared for `genre` (or for the words `asked`)
    fn soul(genre: &str, asked: Option<&str>) -> Prepared {
        Prepared {
            genre: genre.into(),
            bpm: 98.0,
            key: Some(0),
            minor: true,
            chords: vec!["i".into(), "iv".into()],
            asked: asked.map(str::to_string),
            ..shelf().0[1].clone()
        }
    }

    #[test]
    fn a_house_song_asked_for_samples_is_built_on_a_phrase_sped_up_in_place_of_its_chords() {
        let g = crate::genres::get("house").unwrap();
        let s = Shelf(vec![soul("house", None)]);
        let plain = crate::compose::generate(&g, 3, &crate::compose::Options::default(), Some(&s));
        assert!(plain.loops.iter().all(|l| l.name != PHRASE), "house is not sampled unless asked");
        let opts = crate::compose::Options { sampled: Some(true), ..Default::default() };
        let bp = crate::compose::generate(&g, 3, &opts, Some(&s));
        // four semitones up: E minor at 123.47
        assert_eq!((bp.bpm, bp.key.tonic, bp.key.mode), (123.47, 4, Mode::Minor));
        let phrase = bp.loops.iter().find(|l| l.name == PHRASE).expect("the phrase plays");
        assert_eq!(phrase.transpose, 4);
        // where house played its chords, the phrase plays, on its own chords
        let with = |bp: &Blueprint, layer: &str| -> Vec<String> {
            bp.form.iter().filter(|s| s.layers.iter().any(|l| l == layer)).map(|s| s.name.clone()).collect()
        };
        assert!(!with(&plain, "chords").is_empty());
        assert_eq!(with(&bp, PHRASE), with(&plain, "chords"));
        assert!(with(&bp, "chords").is_empty());
        assert_eq!(bp.harmony.progressions["A"], ["i", "iv"]);
        assert!(bp.lineage.iter().any(|l| l.contains("4 semitones up")), "{:?}", bp.lineage);
    }

    #[test]
    fn a_song_asked_for_words_of_its_own_plays_what_was_found_for_them_in_any_genre() {
        let g = crate::genres::get("house").unwrap();
        let s = Shelf(vec![soul("trip_hop", Some("70s soul"))]);
        let asked = |search: Option<&str>, sampled: Option<bool>| {
            let opts =
                crate::compose::Options { sampled, sample_search: search.map(str::to_string), ..Default::default() };
            crate::compose::generate(&g, 3, &opts, Some(&s)).loops.iter().any(|l| l.name == PHRASE)
        };
        assert!(asked(Some("  70s Soul"), None), "its words, as a gathering keeps them");
        assert!(!asked(None, Some(true)), "house's own records: none at hand");
        assert!(!asked(Some("disco"), None) && !asked(Some("70s soul"), Some(false)));
    }

    #[test]
    fn a_break_under_a_genres_own_kick_leaves_the_kick_be() {
        let g = crate::genres::get("house").unwrap();
        let brk = Prepared { genre: "house".into(), bpm: 124.0, ..shelf().0[0].clone() };
        let opts = crate::compose::Options { sampled: Some(true), ..Default::default() };
        let bp = (0..20)
            .map(|seed| crate::compose::generate(&g, seed, &opts, Some(&Shelf(vec![brk.clone()]))))
            .find(|bp| bp.loops.iter().any(|l| l.name == BREAK))
            .expect("house plays a break now and then");
        let kick = bp.drums.iter().find(|d| d.name == "kick").unwrap();
        assert!(!matches!(&kick.sound, Sound::Sample { path, .. } if path == "/b/kick.wav"), "{:?}", kick.sound);
        // it plays where the top loop and the shaker did
        assert!(bp.form.iter().any(|s| s.layers.iter().any(|l| l == BREAK)));
        assert!(bp.form.iter().all(|s| !s.layers.iter().any(|l| l == "top" || l == "shaker")));
    }

    #[test]
    fn without_samples_the_layers_the_genre_names_play_in_their_place() {
        let g = sampling_genre();
        let bp = crate::compose::generate(&g, 3, &crate::compose::Options::default(), None);
        assert!(bp.loops.iter().all(|l| ![BREAK, PHRASE, TEXTURE].contains(&l.name.as_str())));
        let names = bp.layer_names();
        for s in &bp.form {
            assert!(s.layers.iter().all(|l| names.contains(l)), "{}: {:?}", s.name, s.layers);
        }
        // the sections that played the break play the drum lanes
        assert!(bp.form.iter().any(|s| s.layers.contains(&"kick".to_string())));
        assert!(bp.form.iter().any(|s| s.layers.contains(&"keys".to_string())));
    }

    #[test]
    fn a_break_is_cut_to_whole_bars_with_its_own_hits() {
        // two bars at 100 BPM: kicks on 1 and 3, snares on 2 and 4, hats on the eighths
        let beat = 0.6;
        let n = (8.0 * beat * SRF) as usize;
        let mut x = Stereo::zeros(n);
        let mut hit = |t: f64, f: f64, decay: f64, noise: bool, amp: f32| {
            let mut seed = 1u32;
            for i in 0..(0.3 * SRF) as usize {
                let k = (t * SRF) as usize + i;
                if k >= n {
                    break;
                }
                let tt = i as f64 / SRF;
                let v = if noise {
                    seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    (seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
                } else {
                    (std::f64::consts::TAU * f * tt).sin() as f32
                };
                let s = v * amp * (-tt / decay).exp() as f32;
                x.l[k] += s;
                x.r[k] += s;
            }
        };
        for b in 0..8 {
            let t = b as f64 * beat;
            if b % 2 == 0 {
                hit(t, 55.0, 0.12, false, 0.9);
            } else {
                // a snare: a body near 190 Hz under a burst of noise
                hit(t, 190.0, 0.06, false, 0.6);
                hit(t, 0.0, 0.1, true, 0.6);
            }
            hit(t, 0.0, 0.015, true, 0.25);
            hit(t + beat / 2.0, 0.0, 0.015, true, 0.25);
        }
        let f = found("Synthetic break", &[]);
        let g = crate::genres::get("trip_hop").unwrap();
        let c = cut_break(&x, &f, &g, Path::new("demo:break")).unwrap();
        let p = &c.prepared;
        assert!((p.bpm - 100.0).abs() < 0.5 && p.beats == 8.0, "{} {}", p.bpm, p.beats);
        assert!(p.kick.is_some() && p.snare.is_some(), "{p:?}");
        // nothing is written: the loop first, then each hit, at the paths the sample names
        let paths: Vec<String> = c.files.iter().map(|f| f.path.to_string_lossy().into_owned()).collect();
        assert_eq!(paths[0], p.path);
        assert!(paths.contains(p.kick.as_ref().unwrap()) && paths.contains(p.snare.as_ref().unwrap()));
        assert!(!Path::new(&p.path).exists());
        assert_eq!((c.files[0].float, c.files[1].float), (false, true), "a hit keeps its floats");
    }

    #[test]
    fn a_tempo_folds_into_the_genres_range_and_a_loop_toward_the_song() {
        assert_eq!(fit_tempo(176.0, (80.0, 96.0)), Some(88.0));
        assert_eq!(fit_tempo(90.0, (80.0, 96.0)), Some(90.0));
        assert_eq!(fit_tempo(120.0, (80.0, 96.0)), None);
        assert_eq!(fold(151.0, 8.0, 80.0), (75.5, 4.0));
        assert_eq!(fold(90.0, 8.0, 88.0), (90.0, 8.0));
    }
}
