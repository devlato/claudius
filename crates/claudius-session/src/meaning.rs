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

/// Reads the model from claudius's cache, or downloads it there, in the background; once a process.
#[cfg(not(target_arch = "wasm32"))]
pub fn fetch_in_background() {
    static STARTED: std::sync::Once = std::sync::Once::new();
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
    for (file, size, sha) in POTION.files {
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
