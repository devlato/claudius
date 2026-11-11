//! Evals of reading requests in plain words (`ask`): every example request (`data/asks.jsonl`) on one song, whose
//! first proposal must read the request as its example says: its kind, its tracks, its numbers, its time, its sound,
//! the change to go back to before. The examples the readings learn from and those held out of them are scored apart,
//! by words alone and, with the embedding model (`CLAUDIUS_INTENT_MODEL`, as `meaning.rs` takes it), by meaning too.

use claudius_api::{Done, SectionView, SongView, State, TrackView};
use claudius_intent::Model;
use claudius_intent::ask::{Asker, Example, Proposal, Way, examples, propose};
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
