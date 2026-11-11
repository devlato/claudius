//! Evals of reading requests in plain words (`ask`): every example request (`data/asks.jsonl`) on one song, whose
//! first proposal must read the request as its example says: its kind, its tracks, its numbers, its time, its sound,
//! the change to go back to before. The examples the readings learn from and those held out of them are scored apart,
//! by words alone and, with the embedding model (`CLAUDIUS_INTENT_MODEL`, as `meaning.rs` takes it), by meaning too.

use claudius_api::{Done, SectionView, SongView, State, TrackView};
use claudius_intent::Model;
use claudius_intent::ask::{Asker, Example, Kind, Proposal, Way, examples, propose};
use claudius_intent::needle::{Adapters, Needle};
use claudius_intent::teach::{self, Call, TOOLS, lessons, said};
use serde_json::Value;

/// a track of the song: its name, kind, role, sound, level, drive, reverb, delay and pan
fn track(name: &str, kind: &str, role: &str, sound: &str, mix: [f32; 5]) -> TrackView {
    let [level_db, drive_db, reverb, delay, pan] = mix;
    TrackView {
        name: name.into(),
        kind: kind.into(),
        role: role.into(),
        sound: sound.into(),
        level_db,
        drive_db,
        reverb,
        delay,
        pan,
        ..TrackView::default()
    }
}

/// the song every request is read on: house in E minor at 120 BPM, a bar two seconds, its drums, a bass, guitars, a
/// piano, a vocal take, a whistled lead, a string pad, an organ and a choir muted, and what was done to it
fn song() -> State {
    let tracks = vec![
        track("kick", "drum", "kick", "drum voice 'kick'", [0.0, 0.0, 0.05, 0.0, 0.0]),
        track("snare", "drum", "snare", "drum voice 'snare'", [-2.0, 0.0, 0.2, 0.0, 0.0]),
        track("hat", "drum", "hat", "drum voice 'hat'", [-4.0, 0.0, 0.1, 0.0, 0.2]),
        track("tom_hi", "drum", "tom", "drum voice 'tom'", [-3.0, 0.0, 0.1, 0.0, -0.2]),
        track("tom_lo", "drum", "tom", "drum voice 'tom'", [-3.0, 0.0, 0.1, 0.0, 0.2]),
        track("bass", "part", "bass", "synth patch 'saw_bass'", [-3.0, 10.8, 0.0, 0.0, 0.0]),
        track("guitar", "part", "chords", "instrument 'freepats:dist_guitar'", [-5.0, 7.2, 0.15, 0.15, 0.0]),
        track("piano", "part", "keys", "synth patch 'keys'", [-6.0, 0.0, 0.25, 0.0, 0.1]),
        track("vocals", "take", "vocal", "a take, recorded", [-2.0, 0.0, 0.3, 0.1, 0.3]),
        track("lead", "part", "lead", "synth patch 'whistle'", [-5.0, 0.0, 0.2, 0.2, 0.0]),
        track("pad", "part", "pad", "instrument 'vsco2:strings'", [-8.0, 0.0, 0.4, 0.0, 0.0]),
        // two tracks muted, to bring back
        TrackView {
            muted: true,
            ..track("organ", "part", "chords", "synth patch 'rock_organ'", [-6.0, 0.0, 0.2, 0.0, 0.0])
        },
        TrackView {
            muted: true,
            ..track("choir", "part", "fx", "synth patch 'vowel_pad'", [-9.0, 0.0, 0.5, 0.0, 0.0])
        },
    ];
    let names: Vec<String> = tracks.iter().map(|t| t.name.clone()).collect();
    let section = |name: &str, kind: &str, start_bar: u32, bars: u32| SectionView {
        name: name.into(),
        kind: kind.into(),
        start_bar,
        bars,
        layers: names.clone(),
        ..SectionView::default()
    };
    let sections = vec![
        section("intro", "intro", 0, 8),
        section("verse", "verse", 8, 16),
        section("hook", "hook", 24, 16),
        section("break", "break", 40, 8),
        section("drop", "drop", 48, 16),
        section("outro", "outro", 64, 8),
    ];
    let done = |id: &str, label: &str, said: &str| Done {
        id: id.into(),
        kind: "edit".into(),
        label: label.into(),
        said: said.into(),
        at: 0,
        can_go_back: true,
    };
    State {
        song: Some(SongView {
            title: "Night Bus".into(),
            genre: "house".into(),
            bpm: 120.0,
            key: "E minor".into(),
            bars: 72,
            seconds: 144.0,
            tracks,
            sections,
            ..SongView::default()
        }),
        history: vec![
            done("6@x", "tempo 124", "bpm 120 to 124, faster"),
            done("5@x", "add track whistle (lead)", "added the whistle track (lead), playing the synth whistle"),
            done("4@x", "mix bass: level -3", "bass: mix level db 0 to -3, quieter"),
            done(
                "3@x",
                "add track strings (pad)",
                "added the strings track (pad), playing the instrument vsco2:strings",
            ),
            done("2@x", "new song: Night Bus — house at 120 BPM in A minor (seed 3)", "a new song: Night Bus house"),
        ],
        // enough to undo and redo as many as the examples ask
        can_undo: true,
        can_redo: true,
        undo_steps: 12,
        redo_steps: 5,
        ..State::default()
    }
}

/// why a proposal is not what its example expects, or nothing
fn wrong(p: Option<&Proposal>, e: &Example) -> Option<String> {
    let Some(p) = p else { return Some("no proposal".into()) };
    let r = &p.reading;
    let number = |k: &str| e.expect.get(k).and_then(Value::as_f64);
    let text = |k: &str| e.expect.get(k).and_then(Value::as_str);
    let mut why = Vec::new();
    if r.kind != Some(e.kind) {
        why.push(format!("read as {:?}", r.kind));
    }
    if let Some(t) = text("track")
        && r.tracks != [t]
    {
        why.push(format!("tracks {:?}", r.tracks));
    }
    if let Some(Value::Array(ts)) = e.expect.get("tracks") {
        let mut want: Vec<&str> = ts.iter().filter_map(Value::as_str).collect();
        let mut got: Vec<&str> = r.tracks.iter().map(String::as_str).collect();
        want.sort();
        got.sort();
        if want != got {
            why.push(format!("tracks {got:?}"));
        }
    }
    let near = |a: Option<f64>, b: f64| a.is_some_and(|a| (a - b).abs() < 0.01);
    if let Some(n) = number("count")
        && r.count != Some(n as usize)
    {
        why.push(format!("count {:?}", r.count));
    }
    match (e.expect.get("to"), number("to")) {
        (Some(Value::String(k)), _) if r.key.as_deref() != Some(k.as_str()) => why.push(format!("key {:?}", r.key)),
        (_, Some(to)) if !near(r.to, to) => why.push(format!("to {:?}", r.to)),
        _ => {}
    }
    if let Some(by) = number("by")
        && !near(r.by, by)
    {
        why.push(format!("by {:?}", r.by));
    }
    let way = text("way").map(|w| if w == "up" { Way::Up } else { Way::Down });
    if way.is_some() && r.way != way {
        why.push(format!("way {:?}", r.way));
    }
    if let Some(from) = number("from")
        && !near(r.from, from)
    {
        why.push(format!("from {:?}", r.from));
    }
    if let Some(s) = text("sound")
        && r.sound.as_deref() != Some(s)
    {
        why.push(format!("sound {:?}", r.sound));
    }
    if let Some(a) = text("about")
        && r.about.as_deref() != Some(a)
    {
        why.push(format!("about {:?}", r.about));
    }
    match text("side") {
        Some("left") if !r.pan.is_some_and(|p| p < 0.0) => why.push(format!("pan {:?}", r.pan)),
        Some("right") if !r.pan.is_some_and(|p| p > 0.0) => why.push(format!("pan {:?}", r.pan)),
        Some("center") if !near(r.pan, 0.0) => why.push(format!("pan {:?}", r.pan)),
        _ => {}
    }
    (!why.is_empty()).then(|| format!("{} ({})", why.join(", "), p.said))
}

/// how many of the examples of `set` (or of those the readings learn from) read right, of how many, each one missed
/// said on stderr
fn score(state: &State, asker: &Asker, model: Option<&Model>, set: Option<&str>) -> (usize, usize) {
    let mut right = 0;
    let asks: Vec<Example> = examples().into_iter().filter(|e| e.set.as_deref() == set).collect();
    for e in &asks {
        let proposals = propose(&e.ask, state, asker, model);
        match wrong(proposals.first(), e) {
            None => right += 1,
            Some(why) => eprintln!("'{}': {why}", e.ask),
        }
    }
    (right, asks.len())
}

fn model() -> Option<Model> {
    let path = std::env::var_os("CLAUDIUS_INTENT_MODEL").map(std::path::PathBuf::from)?;
    Some(if path.is_file() {
        Model::from_bytes(&std::fs::read(&path).ok()?).ok()?
    } else {
        Model::from_model2vec(
            &std::fs::read(path.join("model.safetensors")).ok()?,
            &std::fs::read_to_string(path.join("vocab.txt")).ok()?,
        )
        .ok()?
    })
}

/// the share of each set read right, at least: those the readings learn from, those held out, those written later
const BY_WORDS: [usize; 3] = [85, 70, 60];
const BY_MEANING: [usize; 3] = [90, 80, 70];

/// how many of each set read right, of how many, said and held to `least` percent
fn sets(state: &State, asker: &Asker, model: Option<&Model>, by: &str, least: [usize; 3]) {
    let started = std::time::Instant::now();
    let scores = [None, Some("held"), Some("later")].map(|set| score(state, asker, model, set));
    let all: usize = scores.iter().map(|(_, n)| n).sum();
    let each = started.elapsed() / all as u32;
    let [(tuned, t), (held, h), (later, l)] = scores;
    let said = format!("{tuned} of {t} read right; held out {held} of {h}; written later {later} of {l}");
    eprintln!("by {by}: {said}; a request in {each:?}");
    let ok = scores.iter().zip(least).all(|((right, n), least)| right * 100 >= n * least);
    assert!(ok, "{said}, short of {least:?} percent");
}

#[test]
fn requests_in_plain_words_are_read_by_their_words_alone() {
    sets(&song(), &Asker::new(None), None, "words", BY_WORDS);
}

#[test]
fn requests_in_plain_words_are_read_by_meaning_too() {
    let Some(model) = model() else {
        eprintln!("CLAUDIUS_INTENT_MODEL names no model: skipped");
        return;
    };
    sets(&song(), &Asker::new(Some(&model)), Some(&model), "meaning", BY_MEANING);
}

/// what no reading can do on a song: these fall to the actions, and the search of them
const NOTHING: &[&str] = &[
    "what's the weather like",
    "hello",
    "save",
    "open a project",
    "export the song as a wav",
    "write me a poem",
    "make it sound better",
    "show me the mixer",
    "how do i record my voice",
    "play",
    "new song",
    "what key is it in",
];

#[test]
fn a_request_to_undo_more_than_there_is_undoes_what_there_is_and_none_proposes_nothing() {
    let asker = Asker::new(None);
    let state = State { undo_steps: 2, redo_steps: 0, can_undo: true, can_redo: false, ..song() };
    let undo = propose("undo the last 5 changes", &state, &asker, None);
    let first = undo.first().expect("a proposal");
    assert_eq!((first.said.as_str(), first.commands.len()), ("Undo the last 2 changes", 2));
    assert!(first.detail.starts_with("all there are, of the 5 asked: tempo 124"), "{}", first.detail);
    assert!(propose("redo that", &state, &asker, None).is_empty(), "nothing to redo");
    // an older claudius says only whether there is anything to undo
    let older = State { undo_steps: 0, can_undo: true, ..song() };
    assert_eq!(propose("undo the last 3", &older, &asker, None)[0].commands.len(), 3);
}

#[test]
fn what_claudius_cannot_do_on_the_song_proposes_nothing() {
    let state = song();
    let model = model();
    let asker = Asker::new(model.as_ref());
    let proposed: Vec<String> = NOTHING
        .iter()
        .filter_map(|q| propose(q, &state, &asker, model.as_ref()).first().map(|p| format!("'{q}': {}", p.said)))
        .collect();
    assert!(proposed.is_empty(), "proposed: {proposed:#?}");
}

/// The palette shows what words ask before the actions they find, Enter doing the first: an action's name, or a word
/// it goes by, asks nothing of the song (but undo and redo, which do as the action does).
#[test]
fn an_actions_name_or_the_words_it_goes_by_ask_nothing_of_the_song() {
    let state = song();
    let model = model();
    let asker = Asker::new(model.as_ref());
    let mut asked = Vec::new();
    for e in claudius_intent::catalogue::ENTRIES {
        let words = e.words.split(", ").map(str::trim).filter(|w| !w.is_empty());
        for q in std::iter::once(e.name).chain(words) {
            let first = propose(q, &state, &asker, model.as_ref()).into_iter().next();
            let same = |p: &Proposal| match e.id {
                "undo" => p.commands.iter().all(|c| *c == claudius_api::Command::Undo),
                "redo" => p.commands.iter().all(|c| *c == claudius_api::Command::Redo),
                _ => false,
            };
            if let Some(p) = first.filter(|p| !same(p)) {
                asked.push(format!("{} ('{q}'): {}", e.id, p.said));
            }
        }
    }
    assert!(asked.is_empty(), "{asked:#?}");
}

/// the kind of request a tool's call asks
fn kind_of(tool: &str) -> Option<Kind> {
    Some(match tool {
        "undo" => Kind::Undo,
        "redo" => Kind::Redo,
        "go_back" => Kind::Back,
        "set_volume" => Kind::Level,
        "set_drive" => Kind::Drive,
        "set_reverb" => Kind::Reverb,
        "set_delay" => Kind::Delay,
        "pan" => Kind::Pan,
        "set_tempo" => Kind::Tempo,
        "set_key" => Kind::Key,
        "swap_sound" => Kind::Swap,
        "cut_track" => Kind::Cut,
        "mute" => Kind::Mute,
        "unmute" => Kind::Unmute,
        "solo" => Kind::Solo,
        "remove_track" => Kind::Remove,
        _ => return None,
    })
}

/// The song the lessons are read on: the evals' song with a clap too, long enough for every time a lesson names, each
/// track's mix where no value a lesson asks for already is, and steps enough to undo and redo.
fn lesson_song() -> State {
    let mut s = song();
    let song = s.song.as_mut().unwrap();
    song.tracks.insert(5, track("clap", "drum", "clap", "drum voice 'clap'", [0.0; 5]));
    for t in &mut song.tracks {
        (t.level_db, t.drive_db, t.reverb, t.delay, t.pan, t.muted) = (-1.5, 9.5, 0.33, 0.33, 0.3, false);
    }
    let names: Vec<String> = song.tracks.iter().map(|t| t.name.clone()).collect();
    for section in &mut song.sections {
        section.layers = names.clone();
    }
    song.sections.last_mut().unwrap().bars = 72;
    (song.bpm, song.key, song.bars, song.seconds) = (121.0, "F minor".into(), 136, 270.0);
    (s.undo_steps, s.redo_steps) = (12, 12);
    s
}

/// A model's call is said again in the words the readings read, for the song to ground: every lesson's call, so said,
/// reads as the tool it calls.
#[test]
fn every_lesson_said_again_reads_as_the_tool_it_calls() {
    let tools: Value = serde_json::from_str(TOOLS).unwrap();
    let tools = tools.as_array().unwrap();
    let asker = Asker::new(None);
    let state = lesson_song();
    let muted = {
        let mut m = lesson_song();
        m.song.as_mut().unwrap().tracks.iter_mut().for_each(|t| t.muted = true);
        m
    };
    let all = lessons(7, 6);
    assert!(all.len() > 400, "{} lessons", all.len());
    let mut wrong = Vec::new();
    for l in &all {
        let (_, answer) = l.rendered();
        let Some(call) = l.answers.first() else {
            assert_eq!(teach::read(&l.query, &answer), None, "{}", l.query);
            continue;
        };
        let tool = tools.iter().find(|t| t["name"] == call.name.as_str()).unwrap_or_else(|| panic!("{}", call.name));
        for key in call.arguments.keys() {
            assert!(tool["parameters"]["properties"].get(key).is_some(), "{}: {} takes no {key}", l.query, call.name);
        }
        if teach::read(&l.query, &answer).as_ref() != Some(call) {
            wrong.push(format!("'{}': its own answer does not read, the reasoning '{}'", l.query, l.reasoning));
            continue;
        }
        let Some(words) = said(call) else {
            wrong.push(format!("'{}': {:?} says nothing", l.query, call));
            continue;
        };
        let on = if call.name == "unmute" { &muted } else { &state };
        let read = propose(&words, on, &asker, None).into_iter().next().and_then(|p| p.reading.kind);
        if read != kind_of(&call.name) {
            wrong.push(format!("'{}' said as '{words}': read as {read:?}", l.query));
        }
    }
    assert!(wrong.is_empty(), "{} of {}: {wrong:#?}", wrong.len(), all.len());
}

/// the tracks a reading named, as a person would say them: the drums, the toms, everything but those it left out
fn said_tracks(names: &[&str], state: &State) -> String {
    let tracks = &state.song.as_ref().unwrap().tracks;
    let drums: Vec<&str> = tracks.iter().filter(|t| t.kind == "drum").map(|t| t.name.as_str()).collect();
    let mut names: Vec<&str> = names.to_vec();
    names.sort();
    let mut sorted = drums.clone();
    sorted.sort();
    if names == sorted {
        return "the drums".into();
    }
    if names.len() > tracks.len() / 2 {
        let left: Vec<String> =
            tracks.iter().filter(|t| !names.contains(&t.name.as_str())).map(|t| format!("the {}", t.name)).collect();
        return format!("everything except {}", left.join(" and "));
    }
    let mut said: Vec<String> = Vec::new();
    if names.contains(&"tom_hi") && names.contains(&"tom_lo") {
        said.push("the toms".into());
        names.retain(|n| !n.starts_with("tom_"));
    }
    said.extend(names.iter().map(|n| format!("the {n}")));
    said.join(" and ")
}

/// The most the readings can make of a model: every example's call, as a model that read it perfectly would write it,
/// said again and read on the song, reads as the example expects.
#[test]
fn a_perfect_models_call_for_every_example_reads_as_the_example_expects() {
    let state = song();
    let asker = Asker::new(None);
    let mut missed = Vec::new();
    for e in examples() {
        let x = &e.expect;
        let num = |k: &str| x.get(k).and_then(Value::as_f64);
        let names: Vec<&str> = match (x.get("track").and_then(Value::as_str), x.get("tracks")) {
            (Some(t), _) => vec![t],
            (_, Some(Value::Array(ts))) => ts.iter().filter_map(Value::as_str).collect(),
            _ => vec![],
        };
        let mut a = serde_json::Map::new();
        if !names.is_empty() {
            a.insert("tracks".into(), said_tracks(&names, &state).into());
        }
        let up = x.get("way").and_then(Value::as_str) == Some("up");
        let (name, pairs): (&str, Vec<(&str, Value)>) = match e.kind {
            Kind::Undo => ("undo", vec![("count", num("count").unwrap_or(1.0).into())]),
            Kind::Redo => ("redo", vec![("count", num("count").unwrap_or(1.0).into())]),
            Kind::Back => ("go_back", vec![("change", x["about"].clone())]),
            Kind::Level => (
                "set_volume",
                vec![
                    ("to_db", x.get("to").cloned().unwrap_or(Value::Null)),
                    ("by_db", x.get("by").cloned().unwrap_or(Value::Null)),
                    (
                        "way",
                        if x.get("way").is_some() {
                            (if up { "louder" } else { "quieter" }).into()
                        } else {
                            Value::Null
                        },
                    ),
                ],
            ),
            Kind::Drive | Kind::Reverb | Kind::Delay => (
                match e.kind {
                    Kind::Drive => "set_drive",
                    Kind::Reverb => "set_reverb",
                    _ => "set_delay",
                },
                vec![
                    ("to_percent", x.get("to").cloned().unwrap_or(Value::Null)),
                    (
                        "way",
                        if x.get("way").is_some() { (if up { "more" } else { "less" }).into() } else { Value::Null },
                    ),
                ],
            ),
            Kind::Pan => ("pan", vec![("side", x["side"].clone())]),
            Kind::Tempo => (
                "set_tempo",
                vec![
                    ("bpm", x.get("to").cloned().unwrap_or(Value::Null)),
                    ("by_bpm", x.get("by").cloned().unwrap_or(Value::Null)),
                    (
                        "way",
                        if x.get("way").is_some() {
                            (if up { "faster" } else { "slower" }).into()
                        } else {
                            Value::Null
                        },
                    ),
                ],
            ),
            Kind::Key => ("set_key", vec![("key", x["to"].clone())]),
            Kind::Swap => ("swap_sound", vec![("sound", x.get("sound").cloned().unwrap_or(Value::Null))]),
            Kind::Cut => {
                let s = num("from").unwrap_or(0.0) as u32;
                ("cut_track", vec![("from", format!("{}:{:02}", s / 60, s % 60).into())])
            }
            Kind::Mute => ("mute", vec![]),
            Kind::Unmute => ("unmute", vec![]),
            Kind::Solo => ("solo", vec![]),
            Kind::Remove => ("remove_track", vec![]),
        };
        for (k, v) in pairs {
            if !v.is_null() {
                a.insert(k.into(), v);
            }
        }
        let call = Call { name: name.into(), arguments: a };
        let words = said(&call).unwrap_or_default();
        if let Some(why) = wrong(propose(&words, &state, &asker, None).first(), &e) {
            missed.push(format!("'{}' said as '{words}': {why}", e.ask));
        }
    }
    assert!(missed.is_empty(), "{} missed: {missed:#?}", missed.len());
}

/// The taught model, run as the palette runs it (`needle`), on every example, none of which it was taught on: its
/// call, said again and read on the song, reads as the example expects. Needs the model (`CLAUDIUS_NEEDLE_MODEL`, a
/// `needle3.cact`) and the adapters (`CLAUDIUS_NEEDLE_ADAPTERS`), and says how the model alone, the readings alone,
/// and the two together read the examples: the readings first, the model where they read nothing.
#[test]
fn the_taught_model_reads_the_examples_it_was_not_taught() {
    let file = |var: &str| std::fs::read(std::env::var_os(var)?).ok();
    let (Some(model), Some(adapters)) = (file("CLAUDIUS_NEEDLE_MODEL"), file("CLAUDIUS_NEEDLE_ADAPTERS")) else {
        eprintln!("CLAUDIUS_NEEDLE_MODEL and CLAUDIUS_NEEDLE_ADAPTERS name no model and adapters: skipped");
        return;
    };
    let adapters = Adapters::from_safetensors(adapters).unwrap();
    let mut needle = Needle::from_bytes(model, adapters.depth).unwrap();
    needle.adapt(adapters).unwrap();
    let (state, asker) = (song(), Asker::new(None));
    let only = std::env::var("CLAUDIUS_NEEDLE_SET").ok();
    let all: Vec<Example> = examples().into_iter().filter(|e| only.is_none() || e.set == only).collect();
    let (mut alone, mut readings, mut together, mut misled) = (0, 0, 0, 0);
    let (mut refused, mut refused_right, mut misled_unchecked, mut rescued_unchecked) = (0, 0, 0, 0);
    let started = std::time::Instant::now();
    for e in &all {
        let text = needle.complete(&e.ask, TOOLS, 120);
        let call = teach::read(&e.ask, &text);
        let read = call.as_ref().and_then(said).map(|w| propose(&w, &state, &asker, None)).unwrap_or_default();
        let ruled = propose(&e.ask, &state, &asker, None);
        let (m, r) = (wrong(read.first(), e), wrong(ruled.first(), e));
        if m.is_some() {
            eprintln!("'{}': {} <- {text:?}", e.ask, m.as_deref().unwrap_or_default());
        } else if ruled.is_empty() {
            eprintln!("'{}': the model alone reads it, as '{}'", e.ask, read[0].said);
        }
        if call.is_none()
            && let Some(unchecked) = unchecked(&text)
        {
            refused += 1;
            let p = said(&unchecked).map(|w| propose(&w, &state, &asker, None)).unwrap_or_default();
            let right = wrong(p.first(), e).is_none();
            eprintln!("'{}': refused, {} <- {text:?}", e.ask, if right { "right" } else { "wrong" });
            refused_right += usize::from(right);
            misled_unchecked += usize::from(ruled.is_empty() && !p.is_empty() && !right);
            rescued_unchecked += usize::from(ruled.is_empty() && right);
        }
        alone += usize::from(m.is_none());
        readings += usize::from(r.is_none());
        together += usize::from(if ruled.is_empty() { m.is_none() } else { r.is_none() });
        misled += usize::from(ruled.is_empty() && !read.is_empty() && m.is_some());
    }
    let each = started.elapsed() / all.len() as u32;
    eprintln!(
        "of {}: the model alone reads {alone}, the readings {readings}, the readings and the model together {together}, \
         and the model offers {misled} wrong where the readings offer nothing; its checks refuse {refused} calls, \
         {refused_right} of them right: without them, together {} and {} wrong; a request in {each:?}",
        all.len(),
        together + rescued_unchecked,
        misled + misled_unchecked,
    );
    assert!(together >= readings, "the model makes the readings worse");
}

/// the first call in a completion, unchecked
fn unchecked(text: &str) -> Option<Call> {
    let payload = needle_infer::v3_engine::extract_tool_call(text)?;
    serde_json::from_str::<Vec<Call>>(&payload).ok()?.into_iter().next()
}

/// Requests that ask for no change to the song, none of them taught: what the rules read nothing for, the model is
/// asked about, and it should offer nothing either.
const NO_CHANGE: &[&str] = &[
    "what's the weather like",
    "who wrote this song",
    "tell me a joke",
    "is this song any good",
    "thanks, that's great",
    "what key is this in",
    "how loud is the kick right now",
    "what tempo is it at",
    "how many tracks are there",
    "what does reverb actually do",
    "who are you",
    "write me some lyrics about rain",
    "make me a sandwich",
    "good morning",
    "the drums sound amazing",
    "i love this part",
    "where are my files saved",
    "what genre is this",
    "can you hear me",
    "what time is it",
    "is the mix balanced",
    "which track is the loudest",
    "describe the song to me",
    "what instruments are playing",
    "why does it sound so muddy",
    "show me the mixer",
    "open the settings",
    "share this with a friend",
    "close the window",
    "help",
];

#[test]
fn the_taught_model_offers_no_change_for_a_request_that_asks_for_none() {
    let file = |var: &str| std::fs::read(std::env::var_os(var)?).ok();
    let (Some(model), Some(adapters)) = (file("CLAUDIUS_NEEDLE_MODEL"), file("CLAUDIUS_NEEDLE_ADAPTERS")) else {
        eprintln!("CLAUDIUS_NEEDLE_MODEL and CLAUDIUS_NEEDLE_ADAPTERS name no model and adapters: skipped");
        return;
    };
    let adapters = Adapters::from_safetensors(adapters).unwrap();
    let mut needle = Needle::from_bytes(model, adapters.depth).unwrap();
    needle.adapt(adapters).unwrap();
    let (state, asker) = (song(), Asker::new(None));
    let (mut offered, mut unchecked_offered) = (Vec::new(), 0);
    for ask in NO_CHANGE {
        if !propose(ask, &state, &asker, None).is_empty() {
            continue;
        }
        let text = needle.complete(ask, TOOLS, 120);
        let offers = |call: Option<Call>| {
            call.as_ref().and_then(said).map(|w| propose(&w, &state, &asker, None)).unwrap_or_default()
        };
        unchecked_offered += usize::from(!offers(unchecked(&text)).is_empty());
        if let Some(p) = offers(teach::read(ask, &text)).first() {
            eprintln!("'{ask}': {} <- {text:?}", p.said);
            offered.push(*ask);
        }
    }
    eprintln!(
        "of {}: the model offers a change for {}, and would for {unchecked_offered} unchecked",
        NO_CHANGE.len(),
        offered.len()
    );
}

#[test]
fn every_example_is_one_kind_the_readings_know_and_says_what_it_expects() {
    let all = examples();
    assert_eq!(
        all.len(),
        claudius_intent::ask::ASKS.lines().filter(|l| !l.trim().is_empty()).count(),
        "a line that does not read"
    );
    for e in &all {
        assert!(
            !e.expect.is_empty()
                || matches!(e.kind, claudius_intent::ask::Kind::Undo | claudius_intent::ask::Kind::Redo),
            "{}",
            e.ask
        );
        assert!(matches!(e.set.as_deref(), None | Some("held" | "later")), "{}: set {:?}", e.ask, e.set);
    }
    assert!(all.iter().filter(|e| e.set.is_some()).count() * 4 >= all.len(), "a quarter kept out of the examples");
}
