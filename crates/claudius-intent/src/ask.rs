//! Requests in plain words, read into what claudius would do for them: "increase the overdrive on the guitar to 95%",
//! "undo the last five changes", "go back to before we added the whistles", "swap all the toms for snares", "cut the
//! guitar from 1:02". What a request asks (its `Kind`) is told by the examples like it (`ASKS`, the requests the evals
//! and a model's later training share): by meaning once the embedding model is there, and by the words each kind is
//! asked in before. Its numbers, times, tracks, sounds and effects are read from it by rule, and what it asks is worked
//! out on the song as it is: each reading a `Proposal`, in words with the values it changes from and to, and the
//! commands that do it, which the person confirms before anything changes.

use claudius_api::{Command, Done, SongView, State, TrackView};
use serde::{Deserialize, Serialize};

use crate::find::{Item, find_among};
use crate::index::Index;
use crate::model::Model;

/// What a request asks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Undo,
    Redo,
    /// back to before a change made to the song
    Back,
    Level,
    Drive,
    Reverb,
    Delay,
    Pan,
    Tempo,
    Key,
    /// another sound for a track
    Swap,
    /// a track taken out from a time to the end
    Cut,
    Mute,
    Unmute,
    Solo,
    Remove,
}

/// A request as the examples hold it (`data/asks.jsonl`): the words, what they ask, and what the evals expect of them
/// on their song. A `set` keeps one out of the examples, for the evals alone: "held" the requests held out from the
/// first, "later" those written after the readings were last tuned, and scored without tuning them to it.
#[derive(Clone, Debug, Deserialize)]
pub struct Example {
    pub ask: String,
    pub kind: Kind,
    #[serde(default)]
    pub set: Option<String>,
    #[serde(flatten)]
    pub expect: serde_json::Map<String, serde_json::Value>,
}

/// the requests, one a line
pub const ASKS: &str = include_str!("../data/asks.jsonl");

/// every example request
pub fn examples() -> Vec<Example> {
    ASKS.lines().filter(|l| !l.trim().is_empty()).filter_map(|l| serde_json::from_str(l).ok()).collect()
}

/// up or down
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Way {
    Up,
    Down,
}

/// What a request was read as, on its song: the kind, and what it named.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Reading {
    pub kind: Option<Kind>,
    /// the tracks it is about, by their names in the song
    pub tracks: Vec<String>,
    /// the value asked for, in the kind's own unit: a level's dB, an effect's percent, a tempo's BPM
    pub to: Option<f64>,
    /// by how much, in that unit, up when above zero
    pub by: Option<f64>,
    pub way: Option<Way>,
    /// how many changes to undo or redo
    pub count: Option<usize>,
    /// where in the song, in seconds
    pub from: Option<f64>,
    /// the sound a swap is to, or the track whose sound it is
    pub sound: Option<String>,
    /// a key asked for: "A minor"
    pub key: Option<String>,
    /// a pan asked for, -1 (left) to 1 (right)
    pub pan: Option<f64>,
    /// of a going back, the change it goes back to before: what undo calls it
    pub about: Option<String>,
    /// read by the small model on the device (`needle`), where the words alone read nothing
    #[serde(default)]
    pub by_model: bool,
}

/// A reading of a request: what claudius would do, in words, and the commands that do it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Proposal {
    pub said: String,
    /// what it changed, a second line under it (a going back's change, in its words)
    #[serde(default)]
    pub detail: String,
    pub commands: Vec<Command>,
    pub reading: Reading,
    /// how sure the reading is, 0 to 1
    pub score: f32,
}

/// The example requests, with what they mean, ready to tell a request's kind by.
pub struct Asker {
    kinds: Vec<Kind>,
    meanings: Option<Index<usize>>,
}

/// how near an example's meaning must be to count, and from which it counts in full
const NEAR: (f32, f32) = (0.45, 0.85);

impl Asker {
    /// The examples the evals do not hold out, and with `model` what each means.
    pub fn new(model: Option<&Model>) -> Asker {
        let tuned: Vec<Example> = examples().into_iter().filter(|e| e.set.is_none()).collect();
        let meanings = model.map(|m| {
            let mut ix = Index::new(m.dim());
            for (i, e) in tuned.iter().enumerate() {
                if let Some(v) = m.embed(&plain(&e.ask, None)) {
                    ix.add(i, &v);
                }
            }
            ix
        });
        Asker { kinds: tuned.iter().map(|e| e.kind).collect(), meanings }
    }

    /// whether the examples' meanings are read, as well as their words
    pub fn by_meaning(&self) -> bool {
        self.meanings.is_some()
    }

    /// The kinds `query` may ask on `song`, the likeliest first, each with how likely: by the words each kind is asked
    /// in, and by the meanings of the examples nearest it. A word that tells a kind surely leads, the examples' meanings
    /// breaking ties; where no word does, the examples decide.
    pub fn kinds(&self, query: &str, song: Option<&SongView>, model: Option<&Model>) -> Vec<(Kind, f32)> {
        let q = query.to_lowercase();
        let mut cued: Vec<(Kind, f32)> = Vec::new();
        // a request to go back to before a change names the change, in words of other kinds ("before the tempo
        // change"): what it names is less likely what it asks
        let raw = cues(&q);
        let back = raw.iter().any(|(k, _)| *k == Kind::Back);
        for (kind, s) in raw {
            let s = if back && kind != Kind::Back { s * 0.5 } else { s };
            best(&mut cued, kind, s);
        }
        let mut near: Vec<(Kind, f32)> = Vec::new();
        if let (Some(ix), Some(v)) = (&self.meanings, model.and_then(|m| m.embed(&plain(&q, song)))) {
            // the meaning is read without the tracks: one that names a track is less likely of a kind that acts on none
            let on_a_track = song.is_some_and(|s| !named_tracks(&q, s).is_empty());
            for (i, cos) in ix.nearest(&v, 5) {
                let kind = self.kinds[*i];
                let s = ((cos - NEAR.0) / (NEAR.1 - NEAR.0)).clamp(0.0, 1.0);
                let songwide = matches!(kind, Kind::Undo | Kind::Redo | Kind::Back | Kind::Tempo | Kind::Key);
                best(&mut near, kind, if on_a_track && songwide { s * 0.5 } else { s });
            }
        }
        let sure = cued.iter().any(|(_, s)| *s >= 0.75);
        let of = |list: &[(Kind, f32)], k: Kind| list.iter().find(|(x, _)| *x == k).map_or(0.0, |(_, s)| *s);
        let mut scores: Vec<(Kind, f32)> = Vec::new();
        for kind in cued.iter().chain(&near).map(|(k, _)| *k) {
            let (c, n) = (of(&cued, kind), of(&near, kind));
            best(&mut scores, kind, if sure { c + 0.25 * n } else { c.max(n) });
        }
        scores.retain(|(_, s)| *s > 0.0);
        scores.sort_by(|a, b| b.1.total_cmp(&a.1));
        scores
    }
}

/// `kind`'s best score in `list`, as `s` raises it
fn best(list: &mut Vec<(Kind, f32)>, kind: Kind, s: f32) {
    match list.iter_mut().find(|(k, _)| *k == kind) {
        Some((_, b)) => *b = b.max(s),
        None => list.push((kind, s)),
    }
}

/// a request as its meaning is read: without its numbers, and without the words that name a track (of the song's, or
/// the usual ones), which tell what it acts on, not what it asks
fn plain(q: &str, song: Option<&SongView>) -> String {
    let named: Vec<String> = song.map(|s| s.tracks.iter().flat_map(track_words).collect()).unwrap_or_default();
    q.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && !w.chars().any(|c| c.is_ascii_digit()))
        .filter(|w| {
            let one = singular(w);
            !TRACK_WORDS.contains(&one.as_str()) && !named.contains(&one)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// the words tracks are usually called by, which the examples' meanings are read without
const TRACK_WORDS: &[&str] = &[
    "kick", "snare", "hat", "hihat", "tom", "clap", "cymbal", "crash", "ride", "shaker", "bass", "bassline", "sub",
    "guitar", "piano", "key", "keys", "rhode", "vocal", "vox", "voice", "lead", "melody", "pad", "string", "organ",
    "choir", "whistle", "synth", "drum", "brass", "horn", "flute", "violin", "cello", "sax", "arp", "chord", "track",
];

/// the words each kind is asked in, and how surely they tell it
fn cues(q: &str) -> Vec<(Kind, f32)> {
    let has = |w: &str| contains_word(q, w);
    let any = |ws: &[&str]| ws.iter().any(|w| has(w));
    let mut out = Vec::new();
    let mut cue = |kind: Kind, words: &[&str], sure: f32| {
        if any(words) {
            out.push((kind, sure));
        }
    };
    cue(Kind::Back, &["before", "rewind", "restore", "everything since", "everything after"], 0.9);
    // an undo of an undo is a redo
    let redo =
        ["redo", "bring back what", "put back what", "put that change back", "undo my undo", "undo the undo", "undid"];
    if !redo.iter().any(|w| has(w)) {
        let undo = [
            "undo",
            "take back",
            "take that back",
            "revert the last",
            "roll back",
            "step back",
            "scrap that",
            "scrap the last",
            "ctrl z",
            "ctrl+z",
            "cmd z",
            "cmd+z",
            "control z",
        ];
        cue(Kind::Undo, &undo, 0.8);
        cue(Kind::Undo, &["revert", "revert that"], 0.6);
        cue(Kind::Undo, &["go back", "cancel my"], 0.5);
    }
    cue(Kind::Redo, &redo, 0.95);
    // a time and a word for leaving off: a track cut from there
    if q.split_whitespace().any(|w| {
        w.split_once(':').is_some_and(|(m, s)| {
            m.parse::<u32>().is_ok() && s.trim_matches(|c: char| !c.is_ascii_digit()).parse::<u32>().is_ok()
        })
    }) {
        cue(Kind::Cut, &["cut", "lose", "drop", "stop", "kill", "take", "end", "no", "after", "from", "out"], 0.85);
    }
    cue(
        Kind::Drive,
        &["overdrive", "overload", "drive", "distortion", "distort", "distorted", "saturate", "saturation"],
        0.9,
    );
    let dirt =
        ["grit", "gritty", "grittier", "dirt", "dirty", "dirtier", "crunch", "crunchy", "crunchier", "fuzz", "fuzzy"];
    cue(Kind::Drive, &dirt, 0.7);
    cue(Kind::Reverb, &["reverb", "verb", "hall", "ambience", "wetter", "wet", "drier", "dry up"], 0.9);
    cue(Kind::Reverb, &["roomy", "drench", "drenched", "washed out", "drowning", "drowned"], 0.7);
    cue(Kind::Reverb, &["dry"], 0.6);
    cue(Kind::Reverb, &["space", "room", "bigger"], 0.5);
    cue(Kind::Delay, &["delay", "echo", "echoes"], 0.95);
    cue(Kind::Pan, &["pan", "left", "right", "center", "centre", "middle", "side"], 0.7);
    cue(Kind::Tempo, &["tempo", "bpm", "faster", "slower", "speed", "quicker", "slow it", "too slow", "too fast"], 0.9);
    cue(Kind::Tempo, &["slow"], 0.6);
    // "make it 128": the song made a number, which is a tempo
    if numbers(q).iter().any(|n| n.unit == Unit::None && (40.0..=250.0).contains(&n.value)) {
        cue(Kind::Tempo, &["make it", "set it", "take it", "put it"], 0.6);
    }
    cue(Kind::Key, &["key", "transpose", "minor", "major"], 0.8);
    cue(Kind::Swap, &["swap", "replace", "instead", "sound for", "sound to", "another sound", "new sound"], 0.85);
    cue(Kind::Swap, &["to be a", "to be an"], 0.6);
    // "change the lead to a pluck", "try the lead as a pluck", "put a reese on the bass": a track to a sound
    cue(Kind::Swap, &["to a", "to an", "as a", "as an", "put a", "put an"], 0.5);
    cue(Kind::Swap, &["different sound", "use a", "make the", "play the"], 0.4);
    cue(Kind::Cut, &["cut", "stop", "drop out", "take out from", "from", "after", "kill", "lose", "end the"], 0.4);
    cue(Kind::Mute, &["mute", "silence", "turn off", "shut"], 0.9);
    cue(Kind::Mute, &["kill", "for now", "for a bit", "off"], 0.6);
    cue(Kind::Unmute, &["unmute", "back in", "back on"], 0.95);
    cue(Kind::Unmute, &["bring back", "turn back on"], 0.7);
    // "bring the organ back"
    if has("back") {
        cue(Kind::Unmute, &["bring"], 0.7);
    }
    cue(Kind::Solo, &["solo", "only the", "just the", "on its own"], 0.9);
    cue(Kind::Solo, &["only"], 0.6);
    cue(Kind::Solo, &["just"], 0.5);
    cue(Kind::Remove, &["remove", "delete", "get rid of", "completely", "entirely", "at all", "out of the song"], 0.85);
    cue(Kind::Remove, &["throw out", "throw away", "scrap", "ditch"], 0.7);
    cue(Kind::Level, &["louder", "quieter", "volume", "level", "turn up", "turn down", "db", "hear more"], 0.75);
    cue(Kind::Level, &["loud", "quiet", "buried", "too much"], 0.6);
    cue(Kind::Level, &["hear", "boost", "raise", "lower", "pull", "bring", "turn the", "up", "down"], 0.4);
    // more or less of a track, and no more said: of its level
    cue(Kind::Level, &["more", "less"], 0.3);
    out
}

/// a request with its numbers apart from their units: "2db" as "2 db", "120bpm" as "120 bpm"
fn spaced(q: &str) -> String {
    let mut out = String::with_capacity(q.len() + 4);
    let mut last = ' ';
    for c in q.chars() {
        if last.is_ascii_digit() && c.is_alphabetic() {
            out.push(' ');
        }
        out.push(c);
        last = c;
    }
    out
}

/// whether `w` is in `q` as a word or words of its own
fn contains_word(q: &str, w: &str) -> bool {
    split_word(q, w).is_some()
}

/// What `query` asks, on the song in `state`: each reading of it as a proposal, the likeliest first; none when it asks
/// nothing claudius can do, or names nothing in the song it would act on.
pub fn propose(query: &str, state: &State, asker: &Asker, model: Option<&Model>) -> Vec<Proposal> {
    let q = spaced(query.trim()).to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<Proposal> = Vec::new();
    for (kind, sure) in asker.kinds(&q, state.song.as_ref(), model).into_iter().take(3) {
        for mut p in read(kind, &q, state, model) {
            p.score *= sure;
            if !out.iter().any(|o| o.commands == p.commands) {
                out.push(p);
            }
        }
    }
    out.sort_by(|a, b| b.score.total_cmp(&a.score));
    // readings far less likely than the best are no candidates beside it: as likely ones are ("the tom", of two)
    let best = out.first().map_or(0.0, |p| p.score);
    out.retain(|p| p.score >= best * 0.5);
    out.truncate(4);
    out
}

/// the proposals a request of `kind` makes on the song, each sure as its reading is complete
fn read(kind: Kind, q: &str, state: &State, model: Option<&Model>) -> Vec<Proposal> {
    let mut r = Reading { kind: Some(kind), ..Reading::default() };
    // undo and redo name nothing a reading could miss, so only words of theirs ask them: what the words mean alone
    // ("stop", "the changes made") does not
    let cued = |k: Kind| cues(q).iter().any(|(c, _)| *c == k);
    match kind {
        Kind::Undo | Kind::Redo if !cued(kind) => Vec::new(),
        Kind::Undo | Kind::Redo => {
            r.count = Some(count(q).unwrap_or(1));
            steps(kind, r, state).into_iter().collect()
        }
        // back to a change only when asked in words of going back: "new song" names a change too
        Kind::Back if !BACK.iter().any(|w| contains_word(q, w)) => Vec::new(),
        Kind::Back => back(q, r, state, model),
        Kind::Tempo => state.song.as_ref().map(|s| tempo(q, r, s)).into_iter().flatten().collect(),
        Kind::Key => state.song.as_ref().map(|s| key(q, r, s)).into_iter().flatten().collect(),
        _ => {
            let Some(song) = state.song.as_ref() else { return Vec::new() };
            on_tracks(kind, q, r, song)
        }
    }
}

/// the words a request to go back to before a change is asked in
const BACK: &[&str] = &["before", "back", "rewind", "restore", "revert", "reset", "since", "earlier", "how it was"];

/// undo or redo, as many times as asked or as there are to undo or redo; none when there are none
fn steps(kind: Kind, mut r: Reading, state: &State) -> Option<Proposal> {
    let asked = r.count.unwrap_or(1).max(1);
    let (verb, command, can, there) = match kind {
        Kind::Undo => ("Undo", Command::Undo, state.can_undo, state.undo_steps),
        _ => ("Redo", Command::Redo, state.can_redo, state.redo_steps),
    };
    // an older claudius says only whether there are any
    let there = if there == 0 && can { asked } else { there };
    let n = asked.min(there);
    if n == 0 {
        return None;
    }
    let labels: Vec<&str> = state
        .history
        .iter()
        .filter(|d| kind == Kind::Undo && d.kind == "edit" && d.can_go_back)
        .take(n)
        .map(|d| d.label.as_str())
        .collect();
    let (said, detail) = match n {
        1 => (format!("{verb} the last change"), labels.first().map(|l| l.to_string()).unwrap_or_default()),
        n => (format!("{verb} the last {n} changes"), labels.join(", ")),
    };
    let detail = match (n < asked, detail.is_empty()) {
        (false, _) => detail,
        (true, true) => format!("all there are, of the {asked} asked"),
        (true, false) => format!("all there are, of the {asked} asked: {detail}"),
    };
    r.count = Some(n);
    Some(Proposal { said, detail, commands: vec![command; n], reading: r, score: 1.0 })
}

/// back to before the changes the request describes, one proposal each
fn back(q: &str, r: Reading, state: &State, model: Option<&Model>) -> Vec<Proposal> {
    let reachable: Vec<&Done> = state.history.iter().filter(|d| d.can_go_back).collect();
    let items: Vec<Item> = reachable.iter().map(|d| Item { id: &d.id, name: &d.label, words: &d.said }).collect();
    find_among(&as_done(q), &items, model, 3)
        .into_iter()
        .map(|f| Proposal {
            said: format!("Go back to before {}", f.name),
            detail: f.place.clone(),
            commands: vec![Command::GoBack { to: f.id }],
            reading: Reading { about: Some(f.name), ..r.clone() },
            score: f.score.min(1.0),
        })
        .collect()
}

/// a request to go back, with the words the history says its changes in for those people say them in: "sped it up"
/// is "faster", "brought in" is "added"
fn as_done(q: &str) -> String {
    const SAID: &[(&str, &str)] = &[
        ("sped", "faster"),
        ("speed", "faster"),
        ("quicker", "faster"),
        ("slowed", "slower"),
        ("turned up", "louder"),
        ("turned down", "quieter"),
        ("came in", "added"),
        ("went in", "added"),
        ("brought in", "added"),
        ("put in", "added"),
        ("took out", "removed"),
        ("got rid of", "removed"),
    ];
    let mut out = q.to_string();
    for (said, done) in SAID {
        if contains_word(q, said) && !contains_word(q, done) {
            out.push(' ');
            out.push_str(done);
        }
    }
    out
}

fn tempo(q: &str, mut r: Reading, song: &SongView) -> Vec<Proposal> {
    let now = song.bpm;
    let n = numbers(q);
    let by = n.iter().find(|x| x.by).map(|x| x.value * sign(q, x.value));
    let to = n.iter().find(|x| !x.by && (30.0..=300.0).contains(&x.value)).map(|x| x.value);
    r.way = way(q);
    let new = match (to, by, r.way) {
        (Some(t), _, _) => t,
        (_, Some(b), _) => now + b,
        (_, _, Some(w)) => now + step(w) * (now * 0.05).max(4.0).round(),
        _ => return Vec::new(),
    };
    (r.to, r.by) = (to, by);
    if (new - now).abs() < 0.5 || !(20.0..=400.0).contains(&new) {
        return Vec::new();
    }
    let said = format!("Change the tempo from {} to {} BPM", trim(now), trim(new.round()));
    vec![Proposal {
        said,
        detail: String::new(),
        commands: vec![Command::SetTempo { bpm: new.round() }],
        reading: r,
        score: 1.0,
    }]
}

fn key(q: &str, mut r: Reading, song: &SongView) -> Vec<Proposal> {
    let Some(asked) = key_named(q) else { return Vec::new() };
    if asked.eq_ignore_ascii_case(&song.key) {
        return Vec::new();
    }
    r.key = Some(asked.clone());
    let said = format!("Change the key from {} to {asked}", song.key);
    vec![Proposal {
        said,
        detail: String::new(),
        commands: vec![Command::SetKey { key: asked }],
        reading: r,
        score: 1.0,
    }]
}

/// the kinds that act on tracks: each track (or group) named, the request's values on it
fn on_tracks(kind: Kind, q: &str, mut r: Reading, song: &SongView) -> Vec<Proposal> {
    // a swap names its tracks before the sound they are to go to ("swap the toms for snares"), but for "use a pluck
    // for the lead"
    let (about, target) = match kind {
        Kind::Swap => split_target(q, song),
        _ => (q.to_string(), None),
    };
    let groups = named_tracks(&about, song);
    // a swap to no sound named is to the next one, which only words asking for another sound ask
    let other_sound = || cues(q).iter().any(|(k, _)| *k == Kind::Swap);
    if groups.is_empty() || (kind == Kind::Swap && target.is_none() && !other_sound()) {
        return Vec::new();
    }
    r.way = way(q);
    let mut out = Vec::new();
    for mut tracks in groups {
        // muting what is muted already, or bringing back what plays, is no part of it
        if let Kind::Mute | Kind::Unmute = kind {
            tracks.retain(|t| t.muted != (kind == Kind::Mute));
        }
        if tracks.is_empty() {
            continue;
        }
        let mut r = r.clone();
        r.tracks = tracks.iter().map(|t| t.name.clone()).collect();
        if let Some(p) = on(kind, q, r, &tracks, target.as_deref(), song) {
            out.push(p);
        }
    }
    out
}

/// what `c` makes of each of `tracks` that it acts on: the commands and a line for each; none when it acts on none
fn each<T>(tracks: &[&TrackView], c: impl Fn(&TrackView) -> Option<(T, String)>) -> Option<(Vec<T>, Vec<String>)> {
    let done: Vec<(T, String)> = tracks.iter().filter_map(|t| c(t)).collect();
    (!done.is_empty()).then(|| done.into_iter().unzip())
}

/// the one line of a reading on one track, or what it does to several
fn said(lines: &[String], several: String) -> String {
    if lines.len() == 1 { lines[0].clone() } else { several }
}

/// what the request does to these tracks
fn on(
    kind: Kind,
    q: &str,
    mut r: Reading,
    tracks: &[&TrackView],
    target: Option<&str>,
    song: &SongView,
) -> Option<Proposal> {
    let names = list(tracks.iter().map(|t| t.name.as_str()));
    let n = numbers(q);
    let by = n.iter().find(|x| x.by).map(|x| x.value * sign(q, x.value));
    let down = matches!(r.way, Some(Way::Down)) || by.is_some_and(|b| b < 0.0);
    let (commands, lines, said): (Vec<Command>, Vec<String>, String) = match kind {
        Kind::Level => {
            let to = n.iter().find(|x| x.unit == Unit::Db && !x.by).map(|x| x.value);
            (r.to, r.by) = (to, by);
            let (to, way) = (to, r.way);
            let (c, lines) = each(tracks, |t| {
                let v = match (to, by, way) {
                    (Some(to), _, _) => to as f32,
                    (_, Some(b), _) => t.level_db + b as f32,
                    (_, _, Some(w)) => t.level_db + 3.0 * step(w) as f32,
                    _ => return None,
                }
                .clamp(-60.0, 12.0);
                let up = if v > t.level_db { "up" } else { "down" };
                let line = format!("Turn {} {up} from {} dB to {} dB", t.name, db(t.level_db), db(v));
                Some((mix(&t.name, |m| m.level_db = Some(v)), line))
            })?;
            let several = format!("Turn {names} {}", if down { "down" } else { "up" });
            let said = said(&lines, several);
            (c, lines, said)
        }
        Kind::Drive | Kind::Reverb | Kind::Delay => {
            let (name, of, full): (&str, fn(&TrackView) -> f32, f32) = match kind {
                Kind::Drive => ("drive", |t| t.drive_db, 36.0),
                Kind::Reverb => ("reverb", |t| t.reverb, 1.0),
                _ => ("delay", |t| t.delay, 1.0),
            };
            // taken off ("remove the delay", "take the reverb off"), set to a percent, moved by one, or more or less
            let off = [
                "remove",
                "off",
                "without",
                "kill",
                "lose",
                "ditch",
                "no reverb",
                "no delay",
                "no distortion",
                "no more",
            ]
            .iter()
            .any(|w| contains_word(q, w));
            let to =
                n.iter().find(|x| !x.by && x.unit != Unit::Db && (0.0..=100.0).contains(&x.value)).map(|x| x.value);
            let to = if off { Some(0.0) } else { to };
            (r.to, r.by) = (to, by);
            if to.is_none() && by.is_none() {
                r.way = r.way.or(Some(Way::Up));
            }
            let way = r.way;
            let (c, lines) = each(tracks, |t| {
                let cur = (of(t) / full * 100.0).round();
                let new = match (to, by, way) {
                    (Some(to), _, _) => to as f32,
                    (_, Some(b), _) => cur + b as f32,
                    (_, _, Some(w)) => cur + 25.0 * step(w) as f32,
                    _ => return None,
                }
                .clamp(0.0, 100.0);
                if (new - cur).abs() < 0.5 {
                    return None;
                }
                let verb = if new > cur { "Increase" } else { "Decrease" };
                let line = format!("{verb} {}'s {name} from {cur}% to {new}%", t.name);
                let v = new / 100.0 * full;
                let c = match kind {
                    Kind::Drive => mix(&t.name, |m| m.drive_db = Some(v)),
                    Kind::Reverb => mix(&t.name, |m| m.reverb = Some(v)),
                    _ => mix(&t.name, |m| m.delay = Some(v)),
                };
                Some((c, line))
            })?;
            let several = format!("{} {name} on {names}", if down || off { "Less" } else { "More" });
            let said = said(&lines, several);
            (c, lines, said)
        }
        Kind::Pan => {
            let amount = match q {
                q if q.contains("hard") => 1.0,
                q if q.contains("a bit") || q.contains("slightly") => 0.25,
                _ => 0.5,
            };
            let amount = n.iter().find(|x| x.unit == Unit::Percent).map_or(amount, |x| x.value / 100.0);
            let side = if contains_word(q, "left") {
                -amount
            } else if contains_word(q, "right") {
                amount
            } else if ["center", "centre", "middle"].iter().any(|w| q.contains(w)) {
                0.0
            } else {
                return None;
            } as f32;
            r.pan = Some(f64::from(side));
            let (c, lines) = each(tracks, |t| {
                let line = format!("Pan {} from {} to {}", t.name, pan(t.pan), pan(side));
                ((t.pan - side).abs() > 0.01).then(|| (mix(&t.name, |m| m.pan = Some(side)), line))
            })?;
            let said = said(&lines, format!("Pan {names} to {}", pan(side)));
            (c, lines, said)
        }
        Kind::Swap => {
            let to = target.map(|s| sound_for(s, song));
            r.sound = target.map(str::to_string);
            let (c, lines) = each(tracks, |t| {
                let playing = t.playing.clone().unwrap_or_else(|| t.sound.clone());
                let line = match &to {
                    Some(to) => format!("Swap {}'s sound from {playing} to {to}", t.name),
                    None => format!("Swap {}'s sound from {playing} to the next one", t.name),
                };
                Some((Command::SwapSound { track: Some(t.name.clone()), to: to.clone(), only: None }, line))
            })?;
            let said = said(&lines, format!("Swap {names} to {}", target.unwrap_or("their next sounds")));
            (c, lines, said)
        }
        Kind::Cut => {
            let from = n.iter().find(|x| x.unit == Unit::Time).map(|x| x.value)?;
            r.from = Some(from);
            let first = section_from(from, song)?;
            let start = clock(seconds_at(song.sections[first].start_bar, song.bpm));
            let section = &song.sections[first].name;
            let (c, lines) = each(tracks, |t| {
                let off: Vec<Command> = (first..song.sections.len())
                    .filter(|i| song.sections[*i].layers.contains(&t.name))
                    .map(|i| Command::ToggleLayer { section: i, layer: t.name.clone() })
                    .collect();
                let line = format!("Take {} out from {start} (the {section}) to the end", t.name);
                (!off.is_empty()).then_some((off, line))
            })?;
            let said = said(&lines, format!("Take {names} out from {start} (the {section}) to the end"));
            (c.into_iter().flatten().collect(), lines, said)
        }
        Kind::Mute | Kind::Unmute => {
            let mute = kind == Kind::Mute;
            let verb = if mute { "Mute" } else { "Unmute" };
            let (c, lines) = each(tracks, |t| {
                (t.muted != mute).then(|| (mix(&t.name, |m| m.muted = Some(mute)), format!("{verb} {}", t.name)))
            })?;
            (c, lines, format!("{verb} {names}"))
        }
        Kind::Solo => {
            let (c, lines) =
                each(tracks, |t| Some((mix(&t.name, |m| m.solo = Some(true)), format!("Solo {}", t.name))))?;
            (c, lines, format!("Solo {names}"))
        }
        Kind::Remove => {
            let (c, lines) =
                each(tracks, |t| Some((Command::RemoveTrack { name: t.name.clone() }, format!("Remove {}", t.name))))?;
            (c, lines, format!("Remove {names} from the song"))
        }
        _ => return None,
    };
    let detail = if lines.len() > 1 { lines.join("; ") } else { String::new() };
    Some(Proposal { said, detail, commands, reading: r, score: 1.0 })
}

/// a `set_mix` of one track, with only what `set` gives it
fn mix(track: &str, set: impl FnOnce(&mut MixSet)) -> Command {
    let mut m = MixSet::default();
    set(&mut m);
    Command::SetMix {
        track: track.to_string(),
        level_db: m.level_db,
        pan: m.pan,
        reverb: m.reverb,
        delay: m.delay,
        duck: None,
        drive_db: m.drive_db,
        muted: m.muted,
        solo: m.solo,
        bus: None,
    }
}

#[derive(Default)]
struct MixSet {
    level_db: Option<f32>,
    pan: Option<f32>,
    reverb: Option<f32>,
    delay: Option<f32>,
    drive_db: Option<f32>,
    muted: Option<bool>,
    solo: Option<bool>,
}

/// "the drums", "the toms", "the guitar": the tracks a request names, each group a reading of its own. A group word
/// (drums) or a plural (toms) is all the tracks it names together; a word in the name of several tracks is a reading
/// for each; tracks named together ("the bass and the drums") are one reading. "Everything" is every track, but those
/// named after "except" or "but".
fn named_tracks<'a>(q: &str, song: &'a SongView) -> Vec<Vec<&'a TrackView>> {
    let (q, except) =
        match ["except", "apart from", "other than", "but the", "but"].iter().find_map(|w| split_word(q, w)) {
            Some((before, after)) => (before, Some(after)),
            None => (q, None),
        };
    let words = words_of(q);
    let mut groups = tracks_named(&words, song);
    let joined = [" and ", " & ", ", ", " plus "].iter().any(|j| q.contains(j));
    // the drums, as a group, unless a track is named itself ("cut the guitar where the drums start") and not with it
    let drums: Vec<&TrackView> = song.tracks.iter().filter(|t| t.kind == "drum").collect();
    let group = words.iter().any(|w| ["drums", "drum", "kit", "beat", "percussion"].contains(&w.as_str()));
    if (groups.is_empty() || joined) && group && !drums.is_empty() {
        groups.retain(|g| !g.iter().all(|t| t.kind == "drum"));
        groups.push(drums);
    }
    if joined && groups.len() > 1 {
        let mut all: Vec<&TrackView> = Vec::new();
        for t in groups.into_iter().flatten() {
            if !all.iter().any(|a| a.name == t.name) {
                all.push(t);
            }
        }
        groups = vec![all];
    }
    let every = ["everything", "every track", "all tracks", "all the tracks", "all of them", "the whole mix"];
    if groups.is_empty() && every.iter().any(|w| contains_word(q, w)) {
        groups.push(song.tracks.iter().collect());
    }
    let left_out: Vec<&TrackView> =
        except.map(|e| named_tracks(e, song).into_iter().flatten().collect()).unwrap_or_default();
    for g in &mut groups {
        g.retain(|t| !left_out.iter().any(|o| o.name == t.name));
    }
    groups.retain(|g| !g.is_empty());
    groups
}

/// a request's words, lowercase
fn words_of(q: &str) -> Vec<String> {
    q.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_lowercase).collect()
}

/// `q` before and after the word `w`, where it is a word of its own
fn split_word<'q>(q: &'q str, w: &str) -> Option<(&'q str, &'q str)> {
    q.match_indices(w).find_map(|(i, _)| {
        let before = q[..i].chars().next_back().is_none_or(|c| !c.is_alphanumeric());
        let after = q[i + w.len()..].chars().next().is_none_or(|c| !c.is_alphanumeric());
        (before && after).then(|| (&q[..i], &q[i + w.len()..]))
    })
}

/// the tracks each word of a request names
fn tracks_named<'a>(words: &[String], song: &'a SongView) -> Vec<Vec<&'a TrackView>> {
    let mut groups: Vec<Vec<&TrackView>> = Vec::new();
    for w in words {
        let one = singular(w);
        if one.len() < 3 || IGNORED.contains(&one.as_str()) {
            continue;
        }
        let hits: Vec<&TrackView> = song.tracks.iter().filter(|t| track_words(t).contains(&one)).collect();
        if hits.is_empty() || groups.iter().any(|g| g.iter().any(|t| hits.iter().any(|h| h.name == t.name))) {
            continue;
        }
        let plural = one != *w;
        if plural || hits.len() == 1 {
            groups.push(hits);
        } else {
            groups.extend(hits.into_iter().map(|t| vec![t]));
        }
    }
    groups
}

/// words of a request that name no track though a track's words may hold them
const IGNORED: &[&str] =
    &["track", "the", "and", "song", "sound", "part", "synth", "patch", "instrument", "take", "drum", "lane"];

/// what a track is called by: its name and its parts, its role, its sound's name, and the words for its role
fn track_words(t: &TrackView) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    let mut add = |s: &str| {
        for w in s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| w.len() >= 2) {
            words.push(singular(w));
        }
    };
    add(&t.name);
    add(&t.role);
    // the sound's name, quoted in its words ("synth patch 'whistle'", "instrument 'vsco2:strings'")
    if let Some(name) = t.sound.split('\'').nth(1) {
        add(name);
    }
    let role = t.role.to_lowercase();
    let also: &[&str] = match role.as_str() {
        "vocal" | "vocals" | "voice" => &["vocal", "vox", "voice", "singing", "singer"],
        "lead" => &["lead", "melody", "hook"],
        "keys" => &["keys", "piano", "rhodes"],
        "bass" => &["bass", "bassline", "sub"],
        "hat" | "hats" => &["hat", "hihat", "hi"],
        "kick" => &["kick"],
        "snare" => &["snare"],
        "pad" => &["pad"],
        _ => &[],
    };
    words.extend(also.iter().map(|w| w.to_string()));
    words
}

/// a word without its plural "s": "hats" "hat", "toms" "tom", "whistles" "whistle"
fn singular(w: &str) -> String {
    let w = w.to_lowercase();
    match w.strip_suffix('s') {
        Some(one) if one.len() >= 3 && !one.ends_with('s') => one.to_string(),
        _ => w,
    }
}

/// a swap's tracks and the sound they are to go to: the words before "for", "with", "to", "into" or "on" name the
/// tracks, those after the sound; "use a pluck for the lead" and "make the guitar a supersaw" the other way about
fn split_target(q: &str, song: &SongView) -> (String, Option<String>) {
    for marker in [" for ", " with ", " into ", " to ", " on ", " instead of ", " as "] {
        if let Some((a, b)) = q.split_once(marker) {
            let (a_tracks, b_tracks) = (named_tracks(a, song), named_tracks(b, song));
            let sound_in = |s: &str| sound_word(s, song);
            if ["use ", "put ", "try ", "give "].iter().any(|w| a.contains(w))
                && a_tracks.is_empty()
                && !b_tracks.is_empty()
            {
                return (b.to_string(), sound_in(a));
            }
            if !a_tracks.is_empty() {
                return (a.to_string(), sound_in(b));
            }
        }
    }
    // "make the guitar a supersaw": the track, then the sound
    if let Some(rest) = q.strip_prefix("make the ").or_else(|| q.strip_prefix("make ")) {
        let words: Vec<&str> = rest.split_whitespace().collect();
        if let Some(i) = words.iter().position(|w| *w == "a" || *w == "an") {
            return (words[..i].join(" "), sound_word(&words[i + 1..].join(" "), song));
        }
    }
    (q.to_string(), None)
}

/// a sound a swap may go to, named in `s`: another track's ("the snare"), a sound the song's tracks may swap to, or a
/// preset or drum sound claudius knows
fn sound_word(s: &str, song: &SongView) -> Option<String> {
    let plain: String = s
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && !["a", "an", "the", "some"].contains(w))
        .collect::<Vec<_>>()
        .join(" ");
    if plain.is_empty() {
        return None;
    }
    // a sound of two words first ("warm pad"), then of one
    let words: Vec<&str> = plain.split(' ').collect();
    let mut candidates: Vec<String> = words.windows(2).map(|w| w.join("_")).collect();
    candidates.extend(words.iter().map(|w| singular(w)));
    let known: Vec<String> = song
        .tracks
        .iter()
        .flat_map(|t| t.swaps.iter().map(|c| c.name.to_lowercase()).chain(std::iter::once(t.name.to_lowercase())))
        .chain(SOUNDS.iter().map(|s| s.to_string()))
        .collect();
    candidates.into_iter().find(|c| known.iter().any(|k| k == c || k.ends_with(&format!(":{c}"))))
}

/// the sounds claudius has whatever the song: its synth presets and drum sounds
const SOUNDS: &[&str] = &[
    "sub_bass",
    "808",
    "reese",
    "acid",
    "saw_bass",
    "pluck_bass",
    "organ_bass",
    "wobble",
    "log_drum",
    "pluck",
    "supersaw",
    "pad",
    "warm_pad",
    "stab",
    "epiano",
    "keys",
    "bell",
    "organ",
    "strings",
    "square_lead",
    "chip",
    "fm_bass",
    "rubber_bass",
    "moog_bass",
    "dist_bass",
    "deep_bass",
    "saw_lead",
    "pwm_lead",
    "whistle",
    "soft_lead",
    "glide_lead",
    "brass_lead",
    "fm_lead",
    "vox_lead",
    "solina",
    "glass_pad",
    "pwm_pad",
    "air_pad",
    "vowel_pad",
    "swell_pad",
    "tine_piano",
    "clav",
    "house_piano",
    "brass_stab",
    "rock_organ",
    "fm_marimba",
    "fm_kalimba",
    "harp_pluck",
    "mallet",
    "kick",
    "snare",
    "clap",
    "hat",
    "shaker",
    "tom",
    "rim",
    "cowbell",
    "crash",
    "ride",
    "conga",
];

/// what a swap's `to` is for a sound named `s`: another track's own sound when `s` names one ("the snare"), else `s`
fn sound_for(s: &str, song: &SongView) -> String {
    match song.tracks.iter().find(|t| t.name.eq_ignore_ascii_case(s)) {
        Some(t) => t.playing.clone().unwrap_or_else(|| t.name.clone()),
        None => s.to_string(),
    }
}

/// the section a time falls nearest the start of: the one it is in, or the next when it is nearer that one's start
fn section_from(secs: f64, song: &SongView) -> Option<usize> {
    let starts: Vec<f64> = song.sections.iter().map(|s| seconds_at(s.start_bar, song.bpm)).collect();
    let i = starts.iter().rposition(|s| *s <= secs + 0.01)?;
    let next = starts.get(i + 1).copied();
    Some(match next {
        Some(n) if (n - secs) < (secs - starts[i]) => i + 1,
        _ => i,
    })
}

fn seconds_at(bar: u32, bpm: f64) -> f64 {
    f64::from(bar) * 4.0 * 60.0 / bpm.max(1.0)
}

fn clock(secs: f64) -> String {
    let s = secs.round() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Unit {
    None,
    Db,
    Percent,
    Bpm,
    Time,
}

/// a number of the request, its unit, and whether it is a change by it ("by 3 dB", "up 4")
#[derive(Clone, Copy, Debug)]
struct Number {
    value: f64,
    unit: Unit,
    by: bool,
}

/// the numbers of a request, in digits or in words, with their units: "95%", "-6 db", "128 bpm", "1:02", "by 3",
/// "twenty five", and a time as it is said ("one minute two seconds", "2 minutes and 10 seconds")
fn numbers(q: &str) -> Vec<Number> {
    let q = q.replace("per cent", "percent").to_lowercase();
    let tokens: Vec<&str> = q.split_whitespace().map(|t| t.trim_matches(|c: char| ",.!?".contains(c))).collect();
    let word = |i: usize| tokens.get(i).copied().unwrap_or("");
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let (t, before) = (tokens[i], if i > 0 { tokens[i - 1] } else { "" });
        if let Some((m, s)) = t.split_once(':')
            && let (Ok(m), Ok(s)) = (m.parse::<f64>(), s.parse::<f64>())
        {
            out.push(Number { value: m * 60.0 + s, unit: Unit::Time, by: false });
            i += 1;
            continue;
        }
        let (digits, rest) =
            t.split_at(t.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+')).unwrap_or(t.len()));
        let Some((value, used)) = digits.parse::<f64>().ok().map(|v| (v, 1)).or_else(|| said_number(&tokens[i..]))
        else {
            i += 1;
            continue;
        };
        let next = word(i + used);
        if rest.is_empty() && ["minute", "minutes", "min", "mins"].contains(&next) {
            // and the seconds after the minutes, if they are said ("and", or a "to" a speech model hears, between)
            let from = i + used + 1 + usize::from(["and", "to"].contains(&word(i + used + 1)));
            let secs = tokens
                .get(from..)
                .and_then(number_at)
                .filter(|&(_, n)| ["second", "seconds", "sec", "secs"].contains(&word(from + n)));
            let (s, end) = secs.map_or((0.0, i + used + 1), |(s, n)| (s, from + n + 1));
            out.push(Number { value: value * 60.0 + s, unit: Unit::Time, by: false });
            i = end;
            continue;
        }
        let by = before == "by" || (["up", "down"].contains(&before) && next != "to");
        let unit = match (rest, next) {
            ("%", _) | (_, "percent") | (_, "%") => Unit::Percent,
            ("db", _) | (_, "db") | (_, "decibels") | (_, "decibel") => Unit::Db,
            ("bpm", _) | (_, "bpm") => Unit::Bpm,
            ("s", _) | (_, "second") | (_, "seconds") | (_, "secs") => Unit::Time,
            _ => Unit::None,
        };
        // decibels with a way and nothing setting them ("lower the lead 3 db") are by how much; "to -6 db" is where
        let set = ["to", "at", "is", "be", "=", "like", "around", "about"].contains(&before);
        let by = by || (unit == Unit::Db && !set && way(&q).is_some());
        out.push(Number { value, unit, by });
        i += used;
    }
    out
}

/// the number the words start with, in digits or in words, and how many words it takes
fn number_at(words: &[&str]) -> Option<(f64, usize)> {
    words.first()?.parse::<f64>().ok().map(|v| (v, 1)).or_else(|| said_number(words))
}

/// a number in words, as requests say them, up to 99, and how many words it takes: "seven", "a couple", "twenty",
/// "twenty five", "twenty-five"
fn said_number(words: &[&str]) -> Option<(f64, usize)> {
    let first = *words.first()?;
    if let Some((tens, ones)) = first.split_once('-') {
        return Some((tens_of(tens)? + word_number(ones).filter(|&n| n < 10.0)?, 1));
    }
    match (tens_of(first), words.get(1).and_then(|w| word_number(w)).filter(|&n| n < 10.0)) {
        (Some(tens), Some(ones)) => Some((tens + ones, 2)),
        (Some(tens), None) => Some((tens, 1)),
        (None, _) => word_number(first).map(|n| (n, 1)),
    }
}

/// a number of tens in words
fn tens_of(w: &str) -> Option<f64> {
    let n = match w {
        "twenty" => 20,
        "thirty" => 30,
        "forty" => 40,
        "fifty" => 50,
        "sixty" => 60,
        "seventy" => 70,
        "eighty" => 80,
        "ninety" => 90,
        _ => return None,
    };
    Some(f64::from(n))
}

/// a number below twenty in words, as requests say them
fn word_number(w: &str) -> Option<f64> {
    let n = match w {
        "one" | "once" => 1,
        "two" | "twice" | "couple" => 2,
        "three" | "thrice" | "few" => 3,
        "four" => 4,
        "five" => 5,
        "six" => 6,
        "seven" => 7,
        "eight" => 8,
        "nine" => 9,
        "ten" => 10,
        "eleven" => 11,
        "twelve" => 12,
        "thirteen" => 13,
        "fourteen" => 14,
        "fifteen" => 15,
        "sixteen" => 16,
        "seventeen" => 17,
        "eighteen" => 18,
        "nineteen" => 19,
        _ => return None,
    };
    Some(f64::from(n))
}

/// how many changes a request to undo or redo asks for: "the last 5", "three changes", "twice", "a couple", "x3"
fn count(q: &str) -> Option<usize> {
    let times =
        q.split_whitespace().find_map(|w| w.strip_prefix('x').or_else(|| w.strip_suffix('x'))?.parse::<usize>().ok());
    let n = numbers(q)
        .iter()
        .find(|n| n.unit == Unit::None && n.value >= 1.0 && n.value <= 200.0)
        .map(|n| n.value as usize);
    times.filter(|t| (1..=200).contains(t)).or(n)
}

/// up or down, by the words of the request: a phrase that says it whatever its words say ("dry up", "too loud"), "too"
/// much of something less of it and too little more, else the words for more against those for less
fn way(q: &str) -> Option<Way> {
    const SURELY_UP: &[&str] =
        &["speed up", "too slow", "too quiet", "barely hear", "hear more", "buried", "can't hear"];
    const SURELY_DOWN: &[&str] = &["dry up", "clean up", "too loud", "too fast", "drowning", "a bit less"];
    if SURELY_DOWN.iter().any(|w| contains_word(q, w)) {
        return Some(Way::Down);
    }
    if SURELY_UP.iter().any(|w| contains_word(q, w)) {
        return Some(Way::Up);
    }
    // too much of something or too little: "too dry", "too roomy", and as it is: "the bass is a bit loud"
    const LACKING: &[&str] =
        &["quiet", "soft", "slow", "dry", "clean", "thin", "weak", "small", "low", "dull", "far", "distant"];
    const TOO_MUCH: &[&str] = &[
        "loud",
        "much",
        "wet",
        "roomy",
        "fuzzy",
        "distorted",
        "harsh",
        "boomy",
        "muddy",
        "busy",
        "fast",
        "heavy",
        "dirty",
        "noisy",
        "big",
        "bright",
        "washed",
    ];
    let words: Vec<&str> = q.split(|c: char| !c.is_alphanumeric() && c != '\'').filter(|w| !w.is_empty()).collect();
    if let Some(i) = words.iter().position(|w| *w == "too")
        && let Some(next) = words.get(i + 1)
    {
        return Some(if LACKING.contains(next) { Way::Up } else { Way::Down });
    }
    // what it is ("the bass is loud") asks for less of it, what it is to be ("make the bass sound distorted") more
    let wished = words.iter().any(|w| ["make", "want", "should", "could", "need", "needs"].contains(w));
    let (lacking, too_much) = if wished { (Way::Down, Way::Up) } else { (Way::Up, Way::Down) };
    let is = |w: &str| ["is", "are", "sounds", "sound", "feels", "seems", "gets"].contains(&w) || w.ends_with("'s");
    for i in words.iter().enumerate().filter(|(_, w)| is(w)).map(|(i, _)| i) {
        let after = &words[i + 1..(i + 4).min(words.len())];
        if after.iter().any(|w| LACKING.contains(w)) {
            return Some(lacking);
        }
        if after.iter().any(|w| TOO_MUCH.contains(w)) {
            return Some(too_much);
        }
    }
    const UP: &[&str] = &[
        "increase", "more", "up", "raise", "boost", "louder", "higher", "bigger", "faster", "quicker", "crank", "push",
        "add", "dirtier", "grittier", "wetter",
    ];
    const DOWN: &[&str] = &[
        "decrease", "less", "down", "lower", "reduce", "quieter", "smaller", "slower", "slow", "pull", "drier", "drop",
        "back", "tame", "cut",
    ];
    let up = UP.iter().filter(|w| contains_word(q, w)).count();
    let down = DOWN.iter().filter(|w| contains_word(q, w)).count();
    match up.cmp(&down) {
        std::cmp::Ordering::Greater => Some(Way::Up),
        std::cmp::Ordering::Less => Some(Way::Down),
        std::cmp::Ordering::Equal => None,
    }
}

fn step(w: Way) -> f64 {
    match w {
        Way::Up => 1.0,
        Way::Down => -1.0,
    }
}

/// a change by a number, up or down by the words around it: "lower by 3" is -3, "raise by 2" 2
fn sign(q: &str, value: f64) -> f64 {
    if value < 0.0 {
        return 1.0;
    }
    match way(q) {
        Some(Way::Down) => -1.0,
        _ => 1.0,
    }
}

/// a key named in a request: "A minor", "C major", "E flat major", "F sharp minor", "G" (major)
fn key_named(q: &str) -> Option<String> {
    let words: Vec<&str> = q.split(|c: char| !(c.is_alphanumeric() || c == '#')).filter(|w| !w.is_empty()).collect();
    const MODES: &[&str] = &["major", "minor", "dorian", "phrygian", "lydian", "mixolydian", "locrian"];
    for (i, w) in words.iter().enumerate() {
        let (letter, rest) = w.split_at(1.min(w.len()));
        if !matches!(letter, "a" | "b" | "c" | "d" | "e" | "f" | "g")
            || !(rest.is_empty() || rest == "#" || rest == "b")
        {
            continue;
        }
        // "a" alone is an article unless a mode or an accidental follows
        let mut j = i + 1;
        let mut note = letter.to_uppercase();
        match (rest, words.get(j).copied()) {
            ("#", _) => note.push('#'),
            ("b", _) => note.push('b'),
            (_, Some("sharp")) => {
                note.push('#');
                j += 1;
            }
            (_, Some("flat")) => {
                note.push('b');
                j += 1;
            }
            _ => {}
        }
        let mode = words.get(j).filter(|m| MODES.contains(m)).copied();
        if mode.is_none() && *w == "a" && note == "A" {
            continue;
        }
        if mode.is_none() && i + 1 != words.len() && !q.contains("key") {
            continue;
        }
        return Some(format!("{note} {}", mode.unwrap_or("major")));
    }
    None
}

fn trim(v: f64) -> String {
    let s = format!("{v:.1}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn db(v: f32) -> String {
    trim(f64::from(v))
}

fn pan(v: f32) -> String {
    match v {
        v if v.abs() < 0.01 => "the center".to_string(),
        v if v < 0.0 => format!("{}% left", (-v * 100.0).round()),
        v => format!("{}% right", (v * 100.0).round()),
    }
}

/// "kick, snare and hat"
fn list<'a>(names: impl Iterator<Item = &'a str>) -> String {
    let names: Vec<&str> = names.collect();
    match names.as_slice() {
        [] => String::new(),
        [one] => one.to_string(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}
