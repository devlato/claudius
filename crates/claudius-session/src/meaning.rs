//! The embedding model the command palette finds actions by meaning with (`claudius_intent`), held once for the process
//! with the actions and the example requests embedded. It is read from claudius's cache, and when it is not there yet,
//! downloaded there once in the background (its pinned revision on Hugging Face, each file checked against its hash)
//! and kept as claudius's own smaller file. Until it is there, actions are found by name, and requests read by their
//! words.

use std::sync::{Arc, LazyLock, RwLock};

use claudius_intent::model::POTION;
use claudius_intent::{Asker, Finder, Model, catalogue::ENTRIES};

/// what claudius's own file of the model is named after (`<name>.clde`)
pub const NAME: &str = POTION.name;

/// The model once it is there, the palette's actions, found by meaning once it is, and requests, read by meaning too
/// once it is.
#[derive(Clone)]
pub struct Held {
    pub model: Option<Arc<Model>>,
    pub finder: Arc<Finder>,
    pub asker: Arc<Asker>,
}

/// the model, and the palette's actions and example requests embedded with it once, as it came
static HELD: RwLock<Option<Held>> = RwLock::new(None);

/// the actions and the requests as they are found and read without the model
static BY_WORDS: LazyLock<Held> = LazyLock::new(|| Held {
    model: None,
    finder: Arc::new(Finder::new(ENTRIES, None)),
    asker: Arc::new(Asker::new(None)),
});

/// what finds actions and reads requests: by meaning once the model is there, by their words until it is
pub fn held() -> Held {
    HELD.read().ok().and_then(|h| h.clone()).unwrap_or_else(|| BY_WORDS.clone())
}

/// whether the model is there
pub fn ready() -> bool {
    HELD.read().is_ok_and(|h| h.is_some())
}

fn hold(model: Model) {
    let (finder, asker) = (Finder::new(ENTRIES, Some(&model)), Asker::new(Some(&model)));
    if let Ok(mut h) = HELD.write() {
        *h = Some(Held { model: Some(Arc::new(model)), finder: Arc::new(finder), asker: Arc::new(asker) });
    }
}

/// The model from claudius's own file of it (`Model::to_bytes`), as a page that fetched it hands it over.
pub fn provide(bytes: &[u8]) -> Result<(), String> {
    hold(Model::from_bytes(bytes)?);
    Ok(())
}

/// Reads the model from claudius's cache, or downloads it there, in the background; once a process. The palette asks
/// for it, which lets the request model download too, once the readings first read nothing of a request.
#[cfg(not(target_arch = "wasm32"))]
pub fn fetch_in_background() {
    static STARTED: std::sync::Once = std::sync::Once::new();
    model::ALLOWED.store(true, std::sync::atomic::Ordering::Relaxed);
    STARTED.call_once(|| {
        std::thread::spawn(|| {
            if let Err(why) = load_or_fetch() {
                eprintln!("claudius: the palette finds actions by name only for now: {why}");
            }
        });
    });
}

/// the model from the cache, or downloaded and kept there
#[cfg(not(target_arch = "wasm32"))]
fn load_or_fetch() -> Result<(), String> {
    use sha2::{Digest, Sha256};
    let dir = claudius::config::cache_dir("models");
    let own = dir.join(format!("{NAME}.clde"));
    if let Ok(bytes) = std::fs::read(&own)
        && provide(&bytes).is_ok()
    {
        return Ok(());
    }
    let downloads = dir.join(NAME);
    std::fs::create_dir_all(&downloads).map_err(|e| format!("cannot make {}: {e}", downloads.display()))?;
    for &(file, size, sha) in POTION.files {
        let dest = downloads.join(file);
        claudius_packs::download_file(&POTION.url(file), &dest, size, false).map_err(|e| format!("{file}: {e:#}"))?;
        let bytes = std::fs::read(&dest).map_err(|e| format!("{file}: {e}"))?;
        let got: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
        if got != sha {
            let _ = std::fs::remove_file(&dest);
            return Err(format!("{file} is not the file claudius was checked with (SHA-256 {got})"));
        }
    }
    let read = |file: &str| std::fs::read(downloads.join(file)).map_err(|e| format!("{file}: {e}"));
    let vocab = String::from_utf8(read("vocab.txt")?).map_err(|_| "vocab.txt is not text".to_string())?;
    let model = Model::from_model2vec(&read("model.safetensors")?, &vocab)?;
    std::fs::write(&own, model.to_bytes()).map_err(|e| format!("cannot write {}: {e}", own.display()))?;
    let _ = std::fs::remove_dir_all(&downloads);
    hold(model);
    Ok(())
}

/// What the small model on the device read of a request: what it calls, or that it calls nothing; that it is
/// reading it still; or that there is no model to read it (it has not downloaded, or this build has none).
#[derive(Clone, Debug, PartialEq)]
pub enum Read {
    Done(Option<claudius_intent::teach::Call>),
    Reading,
    NoModel,
}

/// Needle 3 with claudius's adapters (`claudius_intent::needle`), on a thread of its own: it downloads once (35 MB,
/// its pinned revision, checked against its hash) into claudius's cache, then reads the latest request it is asked
/// about, one at a time, keeping what it read.
#[cfg(not(target_arch = "wasm32"))]
mod model {
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Condvar, Mutex};

    use claudius_intent::needle::{ADAPTERS, Adapters, DEPTH, NEEDLE3, Needle, fingerprint};
    use claudius_intent::teach::{Call, TOOLS, read};

    use super::Read;

    #[derive(Default)]
    struct Shared {
        /// the model could not be had (it did not download, or would not load), and is not tried again
        gone: bool,
        /// the latest request asked about, which it reads next, once it is there the first time
        wanted: Option<String>,
        /// what it read, request by request
        read: HashMap<String, Option<Call>>,
    }

    static SHARED: Mutex<Option<Shared>> = Mutex::new(None);
    static WAKE: Condvar = Condvar::new();

    /// whether the palette has been asked (`fetch_in_background`), without which the model is never downloaded
    pub static ALLOWED: AtomicBool = AtomicBool::new(false);

    /// What the model read of `query`, asking it to when it has not; it downloads the first time it is asked.
    pub fn read_of(query: &str) -> Read {
        if !ALLOWED.load(Ordering::Relaxed) {
            return Read::NoModel;
        }
        let mut guard = SHARED.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_none() {
            *guard = Some(Shared::default());
            std::thread::spawn(run);
        }
        let shared = guard.as_mut().expect("just made");
        if let Some(call) = shared.read.get(query) {
            return Read::Done(call.clone());
        }
        if shared.gone {
            return Read::NoModel;
        }
        shared.wanted = Some(query.to_string());
        WAKE.notify_one();
        Read::Reading
    }

    fn run() {
        let mut needle = match load() {
            Ok(n) => n,
            Err(why) => {
                eprintln!("claudius: requests the readings cannot read stay unread for now: {why}");
                if let Some(s) = SHARED.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
                    s.gone = true;
                }
                return;
            }
        };
        // the tools are read once, before the first request is
        needle.complete("hello", TOOLS, 1);
        loop {
            let query = {
                let mut guard = SHARED.lock().unwrap_or_else(|e| e.into_inner());
                loop {
                    // the palette asks again about the request being read until it has been: one read is enough
                    if let Some(s) = guard.as_mut()
                        && let Some(q) = s.wanted.take()
                        && !s.read.contains_key(&q)
                    {
                        break q;
                    }
                    guard = WAKE.wait(guard).unwrap_or_else(|e| e.into_inner());
                }
            };
            let call = read(&query, &needle.complete(&query, TOOLS, 120));
            if let Some(s) = SHARED.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
                if s.read.len() > 512 {
                    s.read.clear();
                }
                s.read.insert(query, call);
            }
        }
    }

    /// the model from the cache, or downloaded and kept there, with the adapters
    fn load() -> Result<Needle, String> {
        use sha2::{Digest, Sha256};
        let &(file, size, sha) = NEEDLE3.files.first().ok_or("no file")?;
        let dest = claudius::config::cache_dir("models").join(file);
        let good = |bytes: &[u8]| {
            bytes.len() as u64 == size
                && Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect::<String>() == sha
        };
        let bytes = match std::fs::read(&dest) {
            Ok(b) if good(&b) => b,
            _ => {
                if let Some(dir) = dest.parent() {
                    std::fs::create_dir_all(dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
                }
                claudius_packs::download_file(&NEEDLE3.url(file), &dest, size, false)
                    .map_err(|e| format!("{file}: {e:#}"))?;
                let b = std::fs::read(&dest).map_err(|e| format!("{file}: {e}"))?;
                if !good(&b) {
                    let _ = std::fs::remove_file(&dest);
                    return Err(format!("{file} is not the file claudius was checked with"));
                }
                b
            }
        };
        let adapters = Adapters::from_safetensors(ADAPTERS.to_vec())?;
        if adapters.tools != fingerprint(TOOLS) {
            return Err("the adapters were taught with other tools".into());
        }
        let mut needle = Needle::from_bytes(bytes, DEPTH)?;
        needle.adapt(adapters)?;
        Ok(needle)
    }
}

/// What the small model read of a request the readings read nothing of, as `answer`'s proposals, each marked as the
/// model's; or that it is reading it still, for the palette to ask again. A word alone is an action's name, not a
/// request, and is left to the actions.
pub fn ask_model(answer: &mut claudius_intent::Answer, state: &claudius_api::State, asker: &Asker) {
    let q = answer.query.trim().to_string();
    if !answer.proposed.is_empty() || q.split_whitespace().count() < 2 {
        return;
    }
    match model_read(&q) {
        Read::Done(Some(call)) => {
            let Some(words) = claudius_intent::teach::said(&call) else { return };
            answer.proposed = claudius_intent::ask::propose(&words, state, asker, None)
                .into_iter()
                .map(|mut p| {
                    p.reading.by_model = true;
                    p
                })
                .collect();
        }
        Read::Reading => answer.reading = true,
        Read::Done(None) | Read::NoModel => {}
    }
}

/// What the small model on the device read of `query`, asking it to when it has not (see `Read`).
pub fn model_read(query: &str) -> Read {
    #[cfg(not(target_arch = "wasm32"))]
    return model::read_of(query);
    #[cfg(target_arch = "wasm32")]
    {
        let _ = query;
        Read::NoModel
    }
}

#[cfg(test)]
mod tests {
    use claudius_api::{Done, State};

    use crate::host::Host;

    #[test]
    fn until_the_model_is_there_actions_are_found_by_name_and_a_broken_one_is_refused() {
        let none = State::default();
        let answer = Host::find("mixer", 3, &none);
        assert_eq!(
            (answer.query.as_str(), answer.found[0].id.as_str(), answer.by_meaning),
            ("mixer", "tab:mixer", false)
        );
        assert_eq!(
            Host::find("", 40, &none).found.len(),
            claudius_intent::catalogue::ENTRIES.len(),
            "nothing typed: all of them"
        );
        assert!(super::provide(b"CLDE but not a model").is_err());
        assert!(!Host::find("mixer", 3, &none).by_meaning);
    }

    #[test]
    fn the_changes_the_song_can_go_back_to_before_are_found_too() {
        let done = |id: &str, label: &str, said: &str, can_go_back| Done {
            id: id.into(),
            kind: "edit".into(),
            label: label.into(),
            said: said.into(),
            at: 0,
            can_go_back,
        };
        let history = vec![
            done("3@a", "tempo 128", "bpm 124 to 128", true),
            done("2@a", "track whistle", "added the whistle track (lead), playing the synth whistle", true),
            done("1@a", "track whistles", "added the whistles track", false),
        ];
        let state = State { history, ..State::default() };
        let answer = Host::find("go back to before we added the whistles", 12, &state);
        let ids: Vec<&str> = answer.done.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, ["2@a"], "only what go_back can reach: {:?}", answer.done);
        assert_eq!(answer.done[0].name, "track whistle");
        let back: Vec<&str> = answer.proposed.iter().map(|p| p.said.as_str()).collect();
        assert_eq!(back, ["Go back to before track whistle"], "and asked to: a proposal to: {:?}", answer.proposed);
    }
}
