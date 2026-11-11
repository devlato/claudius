//! The actions a few words find, the best first. By name, as typing a few letters of one finds it: its name starting
//! with them, then each word typed starting a word of its name, of the words it goes by or of where it lives, then the
//! actions holding them. And by meaning, as the embedding model reads the words, against each action's name and against
//! its name with the words it goes by: "make it louder" finds the mixer and "sing over it" the Record window, though
//! they share no word with either's name.

use claudius_api::State;
use serde::{Deserialize, Serialize};

use crate::ask::{Asker, Proposal, propose};
use crate::catalogue::Entry;
use crate::index::Index;
use crate::model::Model;

/// how an action was found
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum By {
    Name,
    Meaning,
}

/// An action the words found: what the studio needs to show it and act on it, and how well it was found (0..1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Found {
    pub id: String,
    pub name: String,
    pub place: String,
    pub keys: String,
    pub score: f32,
    pub by: By,
}

/// What words found, as the engine answers the studio: the words, the actions they found, the best first, the
/// changes made to the song they found, which the song can go back to before (`done`: each change's id, what undo calls
/// it as its `name`, and what it changed as its `place`), what they ask of the song read as what claudius would do for
/// it, for the person to confirm (`proposed`, `ask`), and whether they were found by meaning as well as by name (by
/// name only until the embedding model is there).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Answer {
    pub query: String,
    pub found: Vec<Found>,
    #[serde(default)]
    pub done: Vec<Found>,
    #[serde(default)]
    pub proposed: Vec<Proposal>,
    pub by_meaning: bool,
    /// the small model on the device is still reading the words, which the readings read nothing of: ask again
    #[serde(default)]
    pub reading: bool,
}

/// What `query` finds on the song of `state`, as the palette shows it: at most `limit` of the actions, the changes
/// made to the song it could go back to before, and what it asks of the song.
pub fn answer(
    query: &str,
    limit: usize,
    state: &State,
    finder: &Finder,
    asker: &Asker,
    model: Option<&Model>,
) -> Answer {
    let reachable: Vec<Item> = state
        .history
        .iter()
        .filter(|d| d.can_go_back)
        .map(|d| Item { id: &d.id, name: &d.label, words: &d.said })
        .collect();
    Answer {
        query: query.to_string(),
        found: finder.find(query, model, limit),
        done: find_among(query, &reachable, model, DONE),
        proposed: propose(query, state, asker, model),
        by_meaning: finder.by_meaning(),
        reading: false,
    }
}

/// A change made to the song, as words may find it again: its id, what it is called and what it did.
pub struct Item<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub words: &'a str,
}

/// how many changes made to the song an answer offers at most
pub const DONE: usize = 5;

/// the words a request to go back to before a change has, which tell nothing of which change it was ("go back to
/// before we added the whistles")
const BACK: &[&str] = &[
    "go",
    "going",
    "back",
    "before",
    "undo",
    "revert",
    "return",
    "rewind",
    "when",
    "where",
    "did",
    "had",
    "was",
    "were",
    "again",
    "earlier",
    "point",
    "state",
    "version",
    "time",
    "ago",
    "came",
    "come",
    "got",
    "get",
    "went",
    "still",
    "there",
    "made",
    "everything",
    "since",
];

/// Of `items`, those `query` finds, the best first, at most `limit`, each as an answer offers it: by name and by
/// meaning, as the actions are found, once the words that only ask to go back are taken out of it. Nothing when no
/// word is left ("go back" alone is Undo, an action).
pub fn find_among(query: &str, items: &[Item], model: Option<&Model>, limit: usize) -> Vec<Found> {
    let q = query.trim().to_lowercase();
    let told: Vec<&str> = telling(&q).into_iter().filter(|w| !BACK.contains(w)).collect();
    if told.is_empty() {
        return Vec::new();
    }
    let alike = |t: &str, w: &str| t == w || (t.len().min(w.len()) >= 5 && (w.starts_with(t) || t.starts_with(w)));
    let meant = model.and_then(|m| m.embed(&told.join(" ")));
    let mut scored: Vec<(usize, f32, By)> = items
        .iter()
        .enumerate()
        .filter_map(|(i, it)| {
            let text = format!("{} {}", it.name, it.words).to_lowercase();
            let words: Vec<&str> = text.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
            let matched = told.iter().filter(|t| words.iter().any(|w| alike(t, w))).count();
            let name = if matched == 0 { 0.0 } else { 0.5 + 0.4 * matched as f32 / told.len() as f32 };
            // the nearest of its name, each thing it did, and all of it: a long description dilutes each thing in it
            let meaning = match (&meant, model) {
                (Some(v), Some(m)) => {
                    let all = format!("{}: {}", it.name, it.words);
                    let parts = [it.name, all.as_str()].into_iter().chain(it.words.split("; "));
                    let cos = parts
                        .filter_map(|p| m.embed(p))
                        .map(|w| v.iter().zip(&w).map(|(a, b)| a * b).sum::<f32>())
                        .fold(0.0f32, f32::max);
                    MEANT * ((cos - MEANING.0) / (MEANING.1 - MEANING.0)).clamp(0.0, 1.0)
                }
                _ => 0.0,
            };
            let (strong, by) = if meaning > name { (meaning, By::Meaning) } else { (name, By::Name) };
            let score = strong + 0.1 * meaning.min(name);
            (score > 0.0).then_some((i, score, by))
        })
        .collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let found = |(i, score, by): (usize, f32, By)| Found {
        id: items[i].id.into(),
        name: items[i].name.into(),
        place: items[i].words.into(),
        keys: String::new(),
        score,
        by,
    };
    scored.into_iter().take(limit).map(found).collect()
}

/// the cosine from which an action's meaning is near enough the words' to show it, and from which it counts in full:
/// the embedding model gives unrelated phrases 0.1 to 0.3, and a paraphrase 0.6 and up
const MEANING: (f32, f32) = (0.35, 0.8);

/// what a meaning found in full scores: under each word typed starting a word of an action (0.9), so that typing a
/// few letters of a name still finds it first
const MEANT: f32 = 0.85;

/// The catalogue's actions with what each means, ready to be found.
pub struct Finder {
    entries: &'static [Entry],
    /// the vectors of each action's name and of its name with its words, each keyed by the action's place in `entries`
    meanings: Option<Index<usize>>,
}

impl Finder {
    /// The actions of `entries`, and with `model` what they mean; without it they are found by name only.
    pub fn new(entries: &'static [Entry], model: Option<&Model>) -> Finder {
        let meanings = model.map(|m| {
            let mut ix = Index::new(m.dim());
            for (i, e) in entries.iter().enumerate() {
                for text in [e.name.to_string(), format!("{}: {}", e.name, e.words)] {
                    if let Some(v) = m.embed(&text) {
                        ix.add(i, &v);
                    }
                }
            }
            ix
        });
        Finder { entries, meanings }
    }

    /// whether the actions are found by meaning as well as by name
    pub fn by_meaning(&self) -> bool {
        self.meanings.is_some()
    }

    /// The actions `query` finds, the best first, at most `limit`; all of them, in their order, when nothing is typed.
    /// `model` reads the query's meaning, as it read the actions' (`new`).
    pub fn find(&self, query: &str, model: Option<&Model>, limit: usize) -> Vec<Found> {
        let q = query.trim().to_lowercase();
        let found = |i: usize, score: f32, by: By| {
            let e = &self.entries[i];
            Found { id: e.id.into(), name: e.name.into(), place: e.place.into(), keys: e.keys.into(), score, by }
        };
        if q.is_empty() {
            return (0..self.entries.len()).map(|i| found(i, 0.0, By::Name)).take(limit).collect();
        }
        let mut best: Vec<(f32, By)> = self.entries.iter().map(|e| (by_name(&q, e), By::Name)).collect();
        // what the words mean is what is left once the words any request has are taken out ("my", "the", "song")
        let telling = telling(&q).join(" ");
        let meant = if telling.is_empty() { q.clone() } else { telling };
        if let (Some(ix), Some(v)) = (&self.meanings, model.and_then(|m| m.embed(&meant))) {
            // each action's nearest meaning (its name's, or its name with its words'), where both are near
            let mut meaning = vec![0.0f32; self.entries.len()];
            for (i, cos) in ix.nearest(&v, ix.len()) {
                let score = MEANT * ((cos - MEANING.0) / (MEANING.1 - MEANING.0)).clamp(0.0, 1.0);
                meaning[*i] = meaning[*i].max(score);
            }
            // the stronger reading, and a tenth of the weaker: words and meaning that agree win a tie of words
            for (b, m) in best.iter_mut().zip(meaning) {
                let (strong, by) = if m > b.0 { (m, By::Meaning) } else { (b.0, b.1) };
                *b = (strong + 0.1 * m.min(b.0), by);
            }
        }
        let mut order: Vec<usize> = (0..self.entries.len()).filter(|i| best[*i].0 > 0.0).collect();
        order.sort_by(|a, b| best[*b].0.total_cmp(&best[*a].0).then(a.cmp(b)));
        order.into_iter().take(limit).map(|i| found(i, best[i].0, best[i].1)).collect()
    }
}

/// the words any request might hold, which tell nothing of which action it asks for
const UNTELLING: &[&str] = &[
    "a", "an", "the", "my", "me", "i", "it", "its", "this", "that", "these", "those", "to", "of", "on", "in", "at",
    "for", "with", "some", "please", "can", "could", "you", "we", "us", "let", "lets", "do", "does", "is", "are", "be",
    "how", "what", "want", "would", "like", "and", "or", "from", "into", "just", "now", "all", "song", "track", "tune",
    "music", "s", "t",
];

/// the words of `q` that tell which action it asks for
fn telling(q: &str) -> Vec<&str> {
    q.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty() && !UNTELLING.contains(w)).collect()
}

/// How well `q` (lowercased) finds `e` by name: its name starting with it 1, each word of it starting a word of the
/// action's name, words or place 0.9, the action holding it 0.75; else, by the share of the words that tell which
/// action is asked for (`telling`) that are words of the action, or share their first five letters or more with one
/// and go on from it ("vocal" and "vocals", "recording" and "record", not "down" and "download"), 0.5 to 0.7; else 0.
fn by_name(q: &str, e: &Entry) -> f32 {
    let name = e.name.to_lowercase();
    let all = format!("{name} {} {}", e.words.to_lowercase(), e.place.to_lowercase());
    let words: Vec<&str> = all.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    if name.starts_with(q) {
        return 1.0;
    }
    if q.split_whitespace().all(|t| words.iter().any(|w| w.starts_with(t))) {
        return 0.9;
    }
    if all.contains(q) {
        return 0.75;
    }
    let told = telling(q);
    let alike = |t: &str, w: &str| t == w || (t.len().min(w.len()) >= 5 && (w.starts_with(t) || t.starts_with(w)));
    let matched = told.iter().filter(|t| words.iter().any(|w| alike(t, w))).count();
    if told.is_empty() || matched == 0 {
        return 0.0;
    }
    0.5 + 0.2 * matched as f32 / told.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalogue::ENTRIES;
    use crate::model::tests::safetensors;

    /// a model of two meanings, loudness and the voice, whose words "boost" and "croon" are in no action's name or words
    fn model() -> Model {
        let words = [
            ("louder", [1.0, 0.0]),
            ("volume", [1.0, 0.0]),
            ("levels", [0.9, 0.1]),
            ("boost", [1.0, 0.05]),
            ("sing", [0.0, 1.0]),
            ("vocals", [0.1, 0.9]),
            ("microphone", [0.0, 1.0]),
            ("croon", [0.05, 1.0]),
        ];
        let vocab = words.iter().map(|(w, _)| *w).collect::<Vec<_>>().join("\n");
        let floats: Vec<f32> = words.iter().flat_map(|(_, v)| *v).collect();
        Model::from_model2vec(&safetensors(words.len(), 2, &floats), &vocab).unwrap()
    }

    #[test]
    fn a_name_typed_comes_first_then_the_words_an_action_goes_by() {
        let f = Finder::new(ENTRIES, None);
        let names = |q: &str| f.find(q, None, 10).into_iter().map(|x| x.name).collect::<Vec<_>>();
        assert_eq!(names("").len(), ENTRIES.len().min(10), "everything, in order, before anything is typed");
        assert_eq!(names("mix")[0], "Mixer");
        assert_eq!(names("loop")[0], "Loop");
        assert_eq!(names("volume")[0], "Mixer", "found by a word it goes by");
        assert_eq!(names("whiteboard")[0], "Signal flow, large");
        assert_eq!(names("sample vocals")[0], "Add a recording or sample one...");
        assert!(names("zzzz").is_empty());
        assert!(f.find("mix", None, 10).iter().all(|x| x.by == By::Name));
        assert!(!f.by_meaning());
    }

    #[test]
    fn words_in_no_actions_name_find_the_action_they_mean() {
        let m = model();
        let f = Finder::new(ENTRIES, Some(&m));
        assert!(f.by_meaning());
        let boost = f.find("boost", Some(&m), 5);
        assert_eq!((boost[0].name.as_str(), boost[0].by), ("Mixer", By::Meaning), "{boost:?}");
        let croon = f.find("croon", Some(&m), 5);
        assert_eq!(croon[0].name, "Record a take...", "{croon:?}");
        // a few letters of a name still find it first, whatever the meaning
        assert_eq!(f.find("rec", Some(&m), 5)[0].name, "Record a take...");
        assert_eq!(f.find("mix", Some(&m), 5)[0].name, "Mixer");
        assert!(f.find("quantum", Some(&m), 5).is_empty(), "a word the model does not know finds nothing by meaning");
    }

    #[test]
    fn a_change_made_is_found_by_its_words_once_those_asking_to_go_back_are_taken_out() {
        let items = [
            Item { id: "4@a", name: "vocals louder", words: "vocals: mix level db -6 to -3" },
            Item { id: "3@a", name: "tempo 128", words: "bpm 124 to 128" },
            Item {
                id: "2@a",
                name: "track whistle",
                words: "added the whistle track (lead), playing the synth whistle",
            },
            Item { id: "1@a", name: "new house song", words: "a new song: house at 124 BPM" },
        ];
        let ids =
            |q: &str, m: Option<&Model>| find_among(q, &items, m, 5).into_iter().map(|f| f.id).collect::<Vec<_>>();
        assert_eq!(ids("go back to before we added the whistles", None)[0], "2@a");
        assert_eq!(ids("before the tempo change", None)[0], "3@a");
        assert!(ids("go back", None).is_empty(), "going back alone is Undo, an action");
        assert!(ids("quantum", None).is_empty());
        // words in no change's, found by what they mean
        let m = model();
        let boost = find_among("back to before the boost", &items, Some(&m), 5);
        assert_eq!((boost[0].id.as_str(), boost[0].by), ("4@a", By::Meaning), "{boost:?}");
        assert_eq!(boost[0].place, "vocals: mix level db -6 to -3", "what it changed, to show");
    }
}
