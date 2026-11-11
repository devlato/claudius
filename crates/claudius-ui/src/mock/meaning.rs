//! What the palette's words find in the demo, on the studio's own thread rather than the engine's: a search takes
//! microseconds, where the engine may be rendering a song for seconds. In the app, the session holds the model
//! (`claudius_session::meaning`, which downloads it once); in a page, the page fetches the site's copy (`studio/models/`,
//! which `cargo xtask site --sounds` makes) once and holds it. Until it is there, actions are found by name and requests
//! read by their words, and the words asked about last are answered again once it has come.

use std::sync::Arc;

use claudius_api::State;
use claudius_intent::{Answer, Asker, Finder, Model, answer};
use eframe::egui;

/// for how long after the last words the demo looks for the model to come, to answer them again (seconds)
const WAITS: f64 = 20.0;

/// the words the palette asked about last: how many actions, when, and whether they were found by meaning
#[derive(Default)]
pub struct Finding {
    last: Option<(String, usize, f64, bool)>,
}

/// the model once it is there, and what finds actions and reads requests with it, or without it until it is
#[derive(Clone)]
struct Held {
    model: Option<Arc<Model>>,
    finder: Arc<Finder>,
    asker: Arc<Asker>,
}

impl Finding {
    /// What the words find on the song of `state`, by name and by their words, and by meaning once the model is there
    /// (it is asked for the first time): the actions, the changes in its history the song can go back to before, and
    /// what they ask of the song.
    pub fn find(&mut self, query: &str, limit: usize, state: &State, now: f64, ctx: Option<&egui::Context>) -> Answer {
        let held = held(ctx);
        self.last = Some((query.to_string(), limit, now, held.model.is_some()));
        let found = answer(query, limit, state, &held.finder, &held.asker, held.model.as_deref());
        // the app has the small model for what the readings read nothing of; a page has none yet
        #[cfg(not(target_arch = "wasm32"))]
        let found = {
            let mut found = found;
            claudius_session::meaning::ask_model(&mut found, state, &held.asker);
            found
        };
        found
    }

    /// the words asked about last again, once the model has come since they were found by name
    pub fn again(&mut self, state: &State, now: f64, ctx: Option<&egui::Context>) -> Option<Answer> {
        let (query, limit, at, false) = self.last.clone()? else { return None };
        if !ready() {
            if now - at < WAITS
                && let Some(ctx) = ctx
            {
                ctx.request_repaint_after(std::time::Duration::from_millis(250));
            }
            return None;
        }
        Some(self.find(&query, limit, state, now, ctx))
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn held(_: Option<&egui::Context>) -> Held {
    claudius_session::meaning::fetch_in_background();
    let h = claudius_session::meaning::held();
    Held { model: h.model, finder: h.finder, asker: h.asker }
}

#[cfg(not(target_arch = "wasm32"))]
fn ready() -> bool {
    claudius_session::meaning::ready()
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static HELD: std::cell::RefCell<Option<Held>> = const { std::cell::RefCell::new(None) };
    static BY_WORDS: Held = Held {
        model: None,
        finder: Arc::new(Finder::new(claudius_intent::catalogue::ENTRIES, None)),
        asker: Arc::new(Asker::new(None)),
    };
    static ASKED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(target_arch = "wasm32")]
fn held(ctx: Option<&egui::Context>) -> Held {
    use claudius_intent::catalogue::ENTRIES;
    if !ASKED.replace(true) {
        let url = format!("models/{}.clde", claudius_intent::model::POTION.name);
        let ctx = ctx.cloned();
        ehttp::fetch(ehttp::Request::get(&url), move |res| {
            let model = res
                .and_then(|r| if r.ok { Ok(r.bytes) } else { Err(format!("{} {}", r.status, r.status_text)) })
                .and_then(|bytes| Model::from_bytes(&bytes));
            match model {
                Ok(model) => {
                    let (finder, asker) = (Finder::new(ENTRIES, Some(&model)), Asker::new(Some(&model)));
                    HELD.set(Some(Held {
                        model: Some(Arc::new(model)),
                        finder: Arc::new(finder),
                        asker: Arc::new(asker),
                    }));
                    if let Some(ctx) = ctx {
                        ctx.request_repaint();
                    }
                }
                Err(why) => log::warn!("the palette finds actions by name: {url}: {why}"),
            }
        });
    }
    HELD.with_borrow(|h| h.clone()).unwrap_or_else(|| BY_WORDS.with(Held::clone))
}

#[cfg(target_arch = "wasm32")]
fn ready() -> bool {
    HELD.with_borrow(|h| h.is_some())
}
